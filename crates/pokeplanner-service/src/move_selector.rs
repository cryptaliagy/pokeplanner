use std::collections::HashSet;

use pokeplanner_core::{DetailedLearnsetEntry, MoveRole, Pokemon, PokemonType, RecommendedMove};

use crate::type_chart::TypeChart;

/// Result of move selection for a single pokemon.
#[derive(Debug, Clone)]
pub struct MoveRecommendation {
    /// The recommended moves (up to 4).
    pub moves: Vec<RecommendedMove>,
    /// Types covered by the recommended coverage moves.
    pub coverage_types: Vec<PokemonType>,
}

/// Selects optimal moves for a pokemon based on its stats, types, learnset, and weaknesses.
///
/// The algorithm:
/// 1. Filters to damaging moves matching the pokemon's dominant offensive stat
/// 2. Excludes recoil moves (`drain < 0`) and guaranteed self-debuff moves
/// 3. Deduplicates by move name (same move learned via multiple methods)
/// 4. Picks 2 STAB moves (one per type for dual-types when possible)
/// 5. Picks 2 coverage moves using greedy set-cover over weaknesses, with mirror-match fallback
pub struct MoveSelector<'a> {
    type_chart: &'a TypeChart,
}

impl<'a> MoveSelector<'a> {
    pub fn new(type_chart: &'a TypeChart) -> Self {
        Self { type_chart }
    }

    /// Select up to 4 optimal moves for the given pokemon.
    ///
    /// `weaknesses` should be the combined 2x and 4x weakness types for the pokemon.
    pub fn select_moves(
        &self,
        pokemon: &Pokemon,
        learnset: &[DetailedLearnsetEntry],
        weaknesses: &[PokemonType],
    ) -> MoveRecommendation {
        let preferred_class = if pokemon.stats.attack >= pokemon.stats.special_attack {
            "physical"
        } else {
            "special"
        };

        // Filter and deduplicate eligible moves
        let eligible = self.filter_eligible(learnset, preferred_class);

        if eligible.is_empty() {
            return MoveRecommendation {
                moves: Vec::new(),
                coverage_types: Vec::new(),
            };
        }

        // Split into STAB and non-STAB pools
        let (stab_pool, coverage_pool): (Vec<_>, Vec<_>) = eligible
            .into_iter()
            .partition(|e| pokemon.types.contains(&e.move_details.move_type));

        // Select STAB moves (up to 2)
        let stab_moves = self.select_stab(&pokemon.types, &stab_pool);

        // Select coverage moves for remaining slots
        let remaining_slots = 4 - stab_moves.len();
        let (coverage_moves, coverage_types) =
            self.select_coverage(&coverage_pool, weaknesses, &pokemon.types, remaining_slots);

        let mut moves = Vec::with_capacity(4);
        for entry in &stab_moves {
            moves.push(RecommendedMove {
                move_name: entry.move_details.name.clone(),
                move_type: entry.move_details.move_type,
                power: entry.move_details.power.unwrap_or(0),
                damage_class: entry.move_details.damage_class.clone(),
                role: MoveRole::Stab,
            });
        }
        for (entry, role) in &coverage_moves {
            moves.push(RecommendedMove {
                move_name: entry.move_details.name.clone(),
                move_type: entry.move_details.move_type,
                power: entry.move_details.power.unwrap_or(0),
                damage_class: entry.move_details.damage_class.clone(),
                role: role.clone(),
            });
        }

        MoveRecommendation {
            moves,
            coverage_types,
        }
    }

    /// Filter learnset to eligible damaging moves of the correct class, deduped by name.
    fn filter_eligible(
        &self,
        learnset: &[DetailedLearnsetEntry],
        preferred_class: &str,
    ) -> Vec<DetailedLearnsetEntry> {
        let mut seen = HashSet::new();
        let mut eligible = Vec::new();

        for entry in learnset {
            let m = &entry.move_details;

            // Must be a damaging move with positive power
            if m.power.is_none_or(|p| p == 0) {
                continue;
            }
            // Must match preferred damage class
            if m.damage_class != preferred_class {
                continue;
            }
            // No recoil (drain < 0)
            if m.drain < 0 {
                continue;
            }
            // No guaranteed self-debuffs
            if !m.self_stat_changes.is_empty() {
                continue;
            }
            // Deduplicate by name
            if !seen.insert(m.name.clone()) {
                continue;
            }

            eligible.push(entry.clone());
        }

        eligible
    }

    /// Select up to 2 STAB moves. For dual-types, prefer one of each type.
    fn select_stab(
        &self,
        pokemon_types: &[PokemonType],
        stab_pool: &[DetailedLearnsetEntry],
    ) -> Vec<DetailedLearnsetEntry> {
        if pokemon_types.len() >= 2 {
            // Dual type: try to get best of each
            let mut by_type: Vec<Vec<&DetailedLearnsetEntry>> =
                pokemon_types.iter().map(|_| Vec::new()).collect();
            for entry in stab_pool {
                for (i, &t) in pokemon_types.iter().enumerate() {
                    if entry.move_details.move_type == t {
                        by_type[i].push(entry);
                    }
                }
            }
            // Sort each bucket by power descending
            for bucket in &mut by_type {
                bucket.sort_by(|a, b| {
                    b.move_details
                        .power
                        .unwrap_or(0)
                        .cmp(&a.move_details.power.unwrap_or(0))
                });
            }

            let has_type0 = !by_type[0].is_empty();
            let has_type1 = !by_type[1].is_empty();

            match (has_type0, has_type1) {
                (true, true) => {
                    // One from each type
                    vec![by_type[0][0].clone(), by_type[1][0].clone()]
                }
                (true, false) => {
                    // Both from type 0
                    by_type[0].iter().take(2).map(|e| (*e).clone()).collect()
                }
                (false, true) => {
                    // Both from type 1
                    by_type[1].iter().take(2).map(|e| (*e).clone()).collect()
                }
                (false, false) => Vec::new(),
            }
        } else {
            // Mono type: top 2 by power
            let mut sorted: Vec<_> = stab_pool.to_vec();
            sorted.sort_by(|a, b| {
                b.move_details
                    .power
                    .unwrap_or(0)
                    .cmp(&a.move_details.power.unwrap_or(0))
            });
            sorted.into_iter().take(2).collect()
        }
    }

    /// Select coverage moves using greedy set-cover over weaknesses, with mirror-match fallback.
    fn select_coverage(
        &self,
        coverage_pool: &[DetailedLearnsetEntry],
        weaknesses: &[PokemonType],
        pokemon_types: &[PokemonType],
        max_slots: usize,
    ) -> (Vec<(DetailedLearnsetEntry, MoveRole)>, Vec<PokemonType>) {
        if max_slots == 0 || coverage_pool.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let mut result: Vec<(DetailedLearnsetEntry, MoveRole)> = Vec::new();
        let mut all_coverage_types = Vec::new();
        let mut uncovered: HashSet<PokemonType> = weaknesses.iter().copied().collect();
        let mut used_moves: HashSet<String> = HashSet::new();

        for _ in 0..max_slots {
            if !uncovered.is_empty() {
                // Pick the move covering the most uncovered weaknesses, break ties by power
                let best = coverage_pool
                    .iter()
                    .filter(|e| !used_moves.contains(&e.move_details.name))
                    .map(|e| {
                        let covered: Vec<PokemonType> = uncovered
                            .iter()
                            .filter(|&&weak_type| {
                                self.type_chart
                                    .effectiveness(e.move_details.move_type, weak_type)
                                    >= 2.0
                            })
                            .copied()
                            .collect();
                        (e, covered)
                    })
                    .filter(|(_, covered)| !covered.is_empty())
                    .max_by(|(a, a_covered), (b, b_covered)| {
                        a_covered.len().cmp(&b_covered.len()).then_with(|| {
                            a.move_details
                                .power
                                .unwrap_or(0)
                                .cmp(&b.move_details.power.unwrap_or(0))
                        })
                    });

                if let Some((entry, covered)) = best {
                    for &t in &covered {
                        uncovered.remove(&t);
                    }
                    used_moves.insert(entry.move_details.name.clone());
                    let role = MoveRole::WeaknessCoverage(covered.clone());
                    all_coverage_types.extend(covered);
                    result.push((entry.clone(), role));
                    continue;
                }
            }

            // Mirror-match fallback: pick a move SE against the pokemon's own type(s)
            let mirror = coverage_pool
                .iter()
                .filter(|e| !used_moves.contains(&e.move_details.name))
                .filter(|e| {
                    pokemon_types.iter().any(|&pt| {
                        self.type_chart.effectiveness(e.move_details.move_type, pt) >= 2.0
                    })
                })
                .max_by_key(|e| e.move_details.power.unwrap_or(0));

            if let Some(entry) = mirror {
                used_moves.insert(entry.move_details.name.clone());
                result.push((entry.clone(), MoveRole::MirrorCoverage));
                continue;
            }

            // Last resort: highest-power remaining move
            let best_remaining = coverage_pool
                .iter()
                .filter(|e| !used_moves.contains(&e.move_details.name))
                .max_by_key(|e| e.move_details.power.unwrap_or(0));

            if let Some(entry) = best_remaining {
                used_moves.insert(entry.move_details.name.clone());
                result.push((entry.clone(), MoveRole::MirrorCoverage));
            } else {
                break; // No more moves available
            }
        }

        (result, all_coverage_types)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokeplanner_core::{BaseStats, LearnMethod, Move, MoveStatChange};
    use PokemonType::*;

    fn make_move_entry(
        name: &str,
        move_type: PokemonType,
        power: u32,
        class: &str,
    ) -> DetailedLearnsetEntry {
        DetailedLearnsetEntry {
            move_details: Move {
                name: name.to_string(),
                move_type,
                power: Some(power),
                accuracy: Some(100),
                pp: Some(10),
                damage_class: class.to_string(),
                priority: 0,
                effect: None,
                drain: 0,
                self_stat_changes: Vec::new(),
            },
            learn_method: LearnMethod::LevelUp,
            level: 1,
        }
    }

    fn make_recoil_entry(
        name: &str,
        move_type: PokemonType,
        power: u32,
        class: &str,
        drain: i32,
    ) -> DetailedLearnsetEntry {
        let mut entry = make_move_entry(name, move_type, power, class);
        entry.move_details.drain = drain;
        entry
    }

    fn make_debuff_entry(
        name: &str,
        move_type: PokemonType,
        power: u32,
        class: &str,
        stat: &str,
        change: i32,
    ) -> DetailedLearnsetEntry {
        let mut entry = make_move_entry(name, move_type, power, class);
        entry.move_details.self_stat_changes = vec![MoveStatChange {
            stat: stat.to_string(),
            change,
        }];
        entry
    }

    fn make_pokemon(
        name: &str,
        types: Vec<PokemonType>,
        attack: u32,
        special_attack: u32,
    ) -> Pokemon {
        Pokemon {
            species_name: name.to_string(),
            form_name: name.to_string(),
            pokedex_number: 1,
            types,
            stats: BaseStats {
                hp: 80,
                attack,
                defense: 80,
                special_attack,
                special_defense: 80,
                speed: 80,
            },
            is_default_form: true,
        }
    }

    fn chart() -> TypeChart {
        TypeChart::fallback()
    }

    #[test]
    fn physical_vs_special_preference() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);

        // High Attack -> physical
        let physical_mon = make_pokemon("machamp", vec![Fighting], 130, 65);
        let learnset = vec![
            make_move_entry("close-combat", Fighting, 120, "physical"),
            make_move_entry("focus-blast", Fighting, 120, "special"),
        ];
        let result = selector.select_moves(&physical_mon, &learnset, &[]);
        assert!(result.moves.iter().all(|m| m.damage_class == "physical"));

        // High SpA -> special
        let special_mon = make_pokemon("alakazam", vec![Psychic], 50, 135);
        let learnset = vec![
            make_move_entry("zen-headbutt", Psychic, 80, "physical"),
            make_move_entry("psychic", Psychic, 90, "special"),
        ];
        let result = selector.select_moves(&special_mon, &learnset, &[]);
        assert!(result.moves.iter().all(|m| m.damage_class == "special"));
    }

    #[test]
    fn recoil_exclusion() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("arcanine", vec![Fire], 110, 80);
        let learnset = vec![
            make_recoil_entry("flare-blitz", Fire, 120, "physical", -33),
            make_move_entry("fire-fang", Fire, 65, "physical"),
            make_move_entry("flame-wheel", Fire, 60, "physical"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        // Flare Blitz excluded despite highest power
        assert!(!result.moves.iter().any(|m| m.move_name == "flare-blitz"));
        assert!(result.moves.iter().any(|m| m.move_name == "fire-fang"));
    }

    #[test]
    fn self_debuff_exclusion() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("charizard", vec![Fire, Flying], 80, 109);
        let learnset = vec![
            make_debuff_entry("overheat", Fire, 130, "special", "special-attack", -2),
            make_move_entry("flamethrower", Fire, 90, "special"),
            make_move_entry("air-slash", Flying, 75, "special"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        assert!(!result.moves.iter().any(|m| m.move_name == "overheat"));
        assert!(result.moves.iter().any(|m| m.move_name == "flamethrower"));
    }

    #[test]
    fn stab_mono_type() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("arcanine", vec![Fire], 110, 80);
        let learnset = vec![
            make_move_entry("fire-fang", Fire, 65, "physical"),
            make_move_entry("flame-wheel", Fire, 60, "physical"),
            make_move_entry("fire-punch", Fire, 75, "physical"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        let stab: Vec<_> = result
            .moves
            .iter()
            .filter(|m| m.role == MoveRole::Stab)
            .collect();
        assert_eq!(stab.len(), 2);
        // Should pick the 2 highest: fire-punch (75) and fire-fang (65)
        assert!(stab.iter().any(|m| m.move_name == "fire-punch"));
        assert!(stab.iter().any(|m| m.move_name == "fire-fang"));
    }

    #[test]
    fn stab_dual_type() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("charizard", vec![Fire, Flying], 80, 109);
        let learnset = vec![
            make_move_entry("flamethrower", Fire, 90, "special"),
            make_move_entry("fire-blast", Fire, 110, "special"),
            make_move_entry("air-slash", Flying, 75, "special"),
            make_move_entry("hurricane", Flying, 110, "special"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        let stab: Vec<_> = result
            .moves
            .iter()
            .filter(|m| m.role == MoveRole::Stab)
            .collect();
        assert_eq!(stab.len(), 2);
        // Should get one Fire (fire-blast, highest) and one Flying (hurricane, highest)
        let types: HashSet<_> = stab.iter().map(|m| m.move_type).collect();
        assert!(types.contains(&Fire));
        assert!(types.contains(&Flying));
    }

    #[test]
    fn stab_dual_type_fallback() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        // Fire/Flying but no Flying special moves available
        let pokemon = make_pokemon("charizard", vec![Fire, Flying], 80, 109);
        let learnset = vec![
            make_move_entry("flamethrower", Fire, 90, "special"),
            make_move_entry("fire-blast", Fire, 110, "special"),
            make_move_entry("heat-wave", Fire, 95, "special"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        let stab: Vec<_> = result
            .moves
            .iter()
            .filter(|m| m.role == MoveRole::Stab)
            .collect();
        assert_eq!(stab.len(), 2);
        // Both should be Fire since no Flying moves exist
        assert!(stab.iter().all(|m| m.move_type == Fire));
        // Should be the 2 highest: fire-blast (110) and heat-wave (95)
        assert!(stab.iter().any(|m| m.move_name == "fire-blast"));
        assert!(stab.iter().any(|m| m.move_name == "heat-wave"));
    }

    #[test]
    fn coverage_weakness_targeting() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        // Fire pokemon is weak to Water, Ground, Rock
        let pokemon = make_pokemon("arcanine", vec![Fire], 110, 80);
        let learnset = vec![
            make_move_entry("fire-fang", Fire, 65, "physical"),
            make_move_entry("fire-punch", Fire, 75, "physical"),
            // Grass covers Water and Ground
            make_move_entry("solar-blade", Grass, 125, "physical"),
            // Fighting covers Rock
            make_move_entry("close-combat-safe", Fighting, 120, "physical"),
        ];
        let weaknesses = vec![Water, Ground, Rock];
        let result = selector.select_moves(&pokemon, &learnset, &weaknesses);
        let coverage: Vec<_> = result
            .moves
            .iter()
            .filter(|m| matches!(m.role, MoveRole::WeaknessCoverage(_)))
            .collect();
        assert!(!coverage.is_empty());
        // At least one should target a weakness type
        for cm in &coverage {
            if let MoveRole::WeaknessCoverage(types) = &cm.role {
                assert!(types.iter().any(|t| weaknesses.contains(t)));
            }
        }
    }

    #[test]
    fn coverage_greedy_set_cover() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        // Grass/Poison pokemon: weak to Fire, Ice, Flying, Psychic
        let pokemon = make_pokemon("venusaur", vec![Grass, Poison], 82, 100);
        let learnset = vec![
            make_move_entry("energy-ball", Grass, 90, "special"),
            make_move_entry("sludge-bomb", Poison, 90, "special"),
            // Ground covers Fire (SE)
            make_move_entry("earth-power", Ground, 90, "special"),
            // Rock covers Fire, Ice, Flying (3 weaknesses!)
            make_move_entry("power-gem", Rock, 80, "special"),
            // Psychic covers... nothing useful for weaknesses
            make_move_entry("psychic-move", Psychic, 90, "special"),
        ];
        let weaknesses = vec![Fire, Ice, Flying, Psychic];
        let result = selector.select_moves(&pokemon, &learnset, &weaknesses);

        // power-gem (Rock) should be picked first because it covers 3 weaknesses (Fire, Ice, Flying)
        let coverage: Vec<_> = result
            .moves
            .iter()
            .filter(|m| matches!(m.role, MoveRole::WeaknessCoverage(_)))
            .collect();
        assert!(coverage.iter().any(|m| m.move_name == "power-gem"));
    }

    #[test]
    fn mirror_match_fallback() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        // Normal pokemon: only weakness is Fighting
        let pokemon = make_pokemon("snorlax", vec![Normal], 110, 65);
        let learnset = vec![
            make_move_entry("body-slam", Normal, 85, "physical"),
            make_move_entry("return", Normal, 102, "physical"),
            // Fighting covers the Fighting weakness? No — Fighting is NVE vs Fighting.
            // But let's give a move that's SE against Normal (Fighting)
            make_move_entry("brick-break", Fighting, 75, "physical"),
            // Also give a Ground move (not SE against Normal or Fighting weakness)
            make_move_entry("earthquake", Ground, 100, "physical"),
        ];
        let weaknesses = vec![Fighting];
        let result = selector.select_moves(&pokemon, &learnset, &weaknesses);
        // Should have brick-break as weakness coverage (Fighting is SE vs... wait,
        // Fighting is NVE against Fighting, not SE. Let me check the type chart.)
        // Actually: what types are SE against Fighting? Flying, Psychic, Fairy.
        // So none of these coverage moves hit Fighting SE.
        // brick-break (Fighting) is SE against Normal — that's mirror coverage!
        // earthquake (Ground) is not SE against Normal.

        // With no moves SE against Fighting weakness, it should fall back to mirror coverage.
        let mirror: Vec<_> = result
            .moves
            .iter()
            .filter(|m| m.role == MoveRole::MirrorCoverage)
            .collect();
        // brick-break is Fighting, which IS SE against Normal (mirror coverage)
        assert!(
            mirror.iter().any(|m| m.move_name == "brick-break"),
            "Expected brick-break as mirror coverage, got: {mirror:?}"
        );
    }

    #[test]
    fn limited_movepool() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("magikarp", vec![Water], 10, 15);
        // Only 2 eligible moves total
        let learnset = vec![
            make_move_entry("tackle", Normal, 40, "special"),
            make_move_entry("water-pulse", Water, 60, "special"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[Ground]);
        // Should return what's available without panic
        assert!(result.moves.len() <= 4);
        assert!(!result.moves.is_empty());
    }

    #[test]
    fn empty_movepool() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("ditto", vec![Normal], 48, 48);
        let learnset: Vec<DetailedLearnsetEntry> = vec![];
        let result = selector.select_moves(&pokemon, &learnset, &[Fighting]);
        assert!(result.moves.is_empty());
    }

    #[test]
    fn deduplication() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("pikachu", vec![Electric], 55, 50);
        let mut entry1 = make_move_entry("thunderbolt", Electric, 90, "physical");
        entry1.learn_method = LearnMethod::LevelUp;
        let mut entry2 = make_move_entry("thunderbolt", Electric, 90, "physical");
        entry2.learn_method = LearnMethod::Machine;
        let learnset = vec![entry1, entry2];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        // Thunderbolt should appear only once
        let tb_count = result
            .moves
            .iter()
            .filter(|m| m.move_name == "thunderbolt")
            .count();
        assert_eq!(tb_count, 1);
    }

    #[test]
    fn equal_attack_stats_prefers_physical() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        // Equal attack and special_attack: should prefer physical (>=)
        let pokemon = make_pokemon("balanced", vec![Normal], 100, 100);
        let learnset = vec![
            make_move_entry("body-slam", Normal, 85, "physical"),
            make_move_entry("hyper-voice", Normal, 90, "special"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        assert!(result.moves.iter().all(|m| m.damage_class == "physical"));
    }

    #[test]
    fn status_moves_excluded() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("pikachu", vec![Electric], 55, 50);
        let mut status_move = make_move_entry("thunder-wave", Electric, 0, "physical");
        status_move.move_details.power = None;
        let learnset = vec![
            status_move,
            make_move_entry("spark", Electric, 65, "physical"),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        assert!(!result.moves.iter().any(|m| m.move_name == "thunder-wave"));
    }

    #[test]
    fn all_moves_recoil_returns_empty() {
        let tc = chart();
        let selector = MoveSelector::new(&tc);
        let pokemon = make_pokemon("talonflame", vec![Fire, Flying], 81, 74);
        let learnset = vec![
            make_recoil_entry("flare-blitz", Fire, 120, "physical", -33),
            make_recoil_entry("brave-bird", Flying, 120, "physical", -33),
        ];
        let result = selector.select_moves(&pokemon, &learnset, &[]);
        assert!(result.moves.is_empty());
    }
}
