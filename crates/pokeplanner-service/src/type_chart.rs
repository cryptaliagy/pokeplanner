use pokeplanner_core::PokemonType;
use pokeplanner_pokeapi::{TypeEffectivenessData, TypeEffectivenessEntry};
use serde::{Deserialize, Serialize};

const NUM_TYPES: usize = 18;

/// 18x18 type effectiveness matrix.
/// `matrix[attacker][defender]` = damage multiplier.
/// Unspecified pairs default to 1.0 (neutral).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeChart {
    matrix: [[f64; NUM_TYPES]; NUM_TYPES],
}

impl TypeChart {
    /// Build from PokeAPI effectiveness data.
    pub fn from_api_data(data: &TypeEffectivenessData) -> Self {
        let mut matrix = [[1.0_f64; NUM_TYPES]; NUM_TYPES];

        for entry in &data.entries {
            let atk = entry.attack_type.index();
            let def = entry.defend_type.index();
            matrix[atk][def] = entry.multiplier;
        }

        Self { matrix }
    }

    /// Hardcoded fallback chart (Gen 6+ with Fairy, unchanged through Gen 9).
    pub fn fallback() -> Self {
        let mut entries = Vec::new();

        // Helper to add entries
        let mut add = |atk: PokemonType, def: PokemonType, mult: f64| {
            entries.push(TypeEffectivenessEntry {
                attack_type: atk,
                defend_type: def,
                multiplier: mult,
            });
        };

        use PokemonType::*;

        // Normal
        add(Normal, Rock, 0.5);
        add(Normal, Ghost, 0.0);
        add(Normal, Steel, 0.5);

        // Fire
        add(Fire, Fire, 0.5);
        add(Fire, Water, 0.5);
        add(Fire, Grass, 2.0);
        add(Fire, Ice, 2.0);
        add(Fire, Bug, 2.0);
        add(Fire, Rock, 0.5);
        add(Fire, Dragon, 0.5);
        add(Fire, Steel, 2.0);

        // Water
        add(Water, Fire, 2.0);
        add(Water, Water, 0.5);
        add(Water, Grass, 0.5);
        add(Water, Ground, 2.0);
        add(Water, Rock, 2.0);
        add(Water, Dragon, 0.5);

        // Electric
        add(Electric, Water, 2.0);
        add(Electric, Electric, 0.5);
        add(Electric, Grass, 0.5);
        add(Electric, Ground, 0.0);
        add(Electric, Flying, 2.0);
        add(Electric, Dragon, 0.5);

        // Grass
        add(Grass, Fire, 0.5);
        add(Grass, Water, 2.0);
        add(Grass, Grass, 0.5);
        add(Grass, Poison, 0.5);
        add(Grass, Ground, 2.0);
        add(Grass, Flying, 0.5);
        add(Grass, Bug, 0.5);
        add(Grass, Rock, 2.0);
        add(Grass, Dragon, 0.5);
        add(Grass, Steel, 0.5);

        // Ice
        add(Ice, Fire, 0.5);
        add(Ice, Water, 0.5);
        add(Ice, Grass, 2.0);
        add(Ice, Ice, 0.5);
        add(Ice, Ground, 2.0);
        add(Ice, Flying, 2.0);
        add(Ice, Dragon, 2.0);
        add(Ice, Steel, 0.5);

        // Fighting
        add(Fighting, Normal, 2.0);
        add(Fighting, Ice, 2.0);
        add(Fighting, Poison, 0.5);
        add(Fighting, Flying, 0.5);
        add(Fighting, Psychic, 0.5);
        add(Fighting, Bug, 0.5);
        add(Fighting, Rock, 2.0);
        add(Fighting, Ghost, 0.0);
        add(Fighting, Dark, 2.0);
        add(Fighting, Steel, 2.0);
        add(Fighting, Fairy, 0.5);

        // Poison
        add(Poison, Grass, 2.0);
        add(Poison, Poison, 0.5);
        add(Poison, Ground, 0.5);
        add(Poison, Rock, 0.5);
        add(Poison, Ghost, 0.5);
        add(Poison, Steel, 0.0);
        add(Poison, Fairy, 2.0);

        // Ground
        add(Ground, Fire, 2.0);
        add(Ground, Electric, 2.0);
        add(Ground, Grass, 0.5);
        add(Ground, Poison, 2.0);
        add(Ground, Flying, 0.0);
        add(Ground, Bug, 0.5);
        add(Ground, Rock, 2.0);
        add(Ground, Steel, 2.0);

        // Flying
        add(Flying, Electric, 0.5);
        add(Flying, Grass, 2.0);
        add(Flying, Fighting, 2.0);
        add(Flying, Bug, 2.0);
        add(Flying, Rock, 0.5);
        add(Flying, Steel, 0.5);

        // Psychic
        add(Psychic, Fighting, 2.0);
        add(Psychic, Poison, 2.0);
        add(Psychic, Psychic, 0.5);
        add(Psychic, Dark, 0.0);
        add(Psychic, Steel, 0.5);

        // Bug
        add(Bug, Fire, 0.5);
        add(Bug, Grass, 2.0);
        add(Bug, Fighting, 0.5);
        add(Bug, Poison, 0.5);
        add(Bug, Flying, 0.5);
        add(Bug, Psychic, 2.0);
        add(Bug, Ghost, 0.5);
        add(Bug, Dark, 2.0);
        add(Bug, Steel, 0.5);
        add(Bug, Fairy, 0.5);

        // Rock
        add(Rock, Fire, 2.0);
        add(Rock, Ice, 2.0);
        add(Rock, Fighting, 0.5);
        add(Rock, Ground, 0.5);
        add(Rock, Flying, 2.0);
        add(Rock, Bug, 2.0);
        add(Rock, Steel, 0.5);

        // Ghost
        add(Ghost, Normal, 0.0);
        add(Ghost, Psychic, 2.0);
        add(Ghost, Ghost, 2.0);
        add(Ghost, Dark, 0.5);

        // Dragon
        add(Dragon, Dragon, 2.0);
        add(Dragon, Steel, 0.5);
        add(Dragon, Fairy, 0.0);

        // Dark
        add(Dark, Fighting, 0.5);
        add(Dark, Psychic, 2.0);
        add(Dark, Ghost, 2.0);
        add(Dark, Dark, 0.5);
        add(Dark, Fairy, 0.5);

        // Steel
        add(Steel, Fire, 0.5);
        add(Steel, Water, 0.5);
        add(Steel, Electric, 0.5);
        add(Steel, Ice, 2.0);
        add(Steel, Rock, 2.0);
        add(Steel, Steel, 0.5);
        add(Steel, Fairy, 2.0);

        // Fairy
        add(Fairy, Fire, 0.5);
        add(Fairy, Poison, 0.5);
        add(Fairy, Fighting, 2.0);
        add(Fairy, Dragon, 2.0);
        add(Fairy, Dark, 2.0);
        add(Fairy, Steel, 0.5);

        Self::from_api_data(&TypeEffectivenessData { entries })
    }

    /// Get the effectiveness multiplier for attacker type vs defender type.
    pub fn effectiveness(&self, attacker: PokemonType, defender: PokemonType) -> f64 {
        self.matrix[attacker.index()][defender.index()]
    }

    /// Calculate the combined effectiveness of an attack type against a dual-type defender.
    /// Multiplies effectiveness against each of the defender's types.
    pub fn effectiveness_against_pokemon(
        &self,
        attack_type: PokemonType,
        defender_types: &[PokemonType],
    ) -> f64 {
        defender_types
            .iter()
            .map(|&def_type| self.effectiveness(attack_type, def_type))
            .product()
    }

    /// Calculate offensive coverage score for a team.
    /// Returns the fraction of the 18 types that at least one team member can hit super-effectively (>= 2.0).
    pub fn team_offensive_coverage(&self, team_types: &[Vec<PokemonType>]) -> f64 {
        let mut covered = [false; NUM_TYPES];

        for pokemon_types in team_types {
            for &atk_type in pokemon_types {
                for &target_type in &PokemonType::ALL {
                    if self.effectiveness(atk_type, target_type) >= 2.0 {
                        covered[target_type.index()] = true;
                    }
                }
            }
        }

        let covered_count = covered.iter().filter(|&&c| c).count();
        covered_count as f64 / NUM_TYPES as f64
    }

    /// Calculate defensive weakness score for a team.
    /// Returns a score from 0.0 to 1.0 where 1.0 means no shared weaknesses.
    /// Penalizes types that are super-effective against 3+ team members.
    pub fn team_defensive_score(&self, team_types: &[Vec<PokemonType>]) -> f64 {
        let mut bad_types = 0;

        for &attack_type in &PokemonType::ALL {
            let mut weak_count = 0;
            for pokemon_types in team_types {
                let multiplier = self.effectiveness_against_pokemon(attack_type, pokemon_types);
                if multiplier >= 2.0 {
                    weak_count += 1;
                }
            }
            if weak_count >= 3 {
                bad_types += 1;
            }
        }

        1.0 - (bad_types as f64 / NUM_TYPES as f64)
    }

    /// Returns the list of types that no team member can hit super-effectively.
    pub fn uncovered_types(&self, team_types: &[Vec<PokemonType>]) -> Vec<PokemonType> {
        PokemonType::ALL
            .iter()
            .filter(|&&target_type| {
                !team_types.iter().any(|pokemon_types| {
                    pokemon_types
                        .iter()
                        .any(|&atk_type| self.effectiveness(atk_type, target_type) >= 2.0)
                })
            })
            .copied()
            .collect()
    }

    // --- Counter-team scoring methods ---

    /// Offensive coverage against a specific enemy team.
    /// Returns the fraction of enemy pokemon that at least one team member can hit SE.
    pub fn offensive_coverage_against(
        &self,
        team_types: &[Vec<PokemonType>],
        enemy_types: &[Vec<PokemonType>],
    ) -> f64 {
        if enemy_types.is_empty() {
            return 0.0;
        }
        let covered = enemy_types
            .iter()
            .filter(|enemy| {
                team_types.iter().any(|my_types| {
                    my_types
                        .iter()
                        .any(|&atk| self.effectiveness_against_pokemon(atk, enemy) >= 2.0)
                })
            })
            .count();
        covered as f64 / enemy_types.len() as f64
    }

    /// Defensive score against a specific enemy team's STAB types.
    /// Returns the fraction of our team that is NOT weak to any enemy STAB type.
    pub fn defensive_score_against(
        &self,
        team_types: &[Vec<PokemonType>],
        enemy_types: &[Vec<PokemonType>],
    ) -> f64 {
        if team_types.is_empty() {
            return 0.0;
        }
        let safe_count = team_types
            .iter()
            .filter(|my_types| {
                // This team member is "safe" if no enemy STAB type hits it SE
                !enemy_types.iter().any(|enemy| {
                    enemy
                        .iter()
                        .any(|&atk| self.effectiveness_against_pokemon(atk, my_types) >= 2.0)
                })
            })
            .count();
        safe_count as f64 / team_types.len() as f64
    }

    /// Returns enemy pokemon that no team member can hit super-effectively.
    pub fn uncovered_enemies(
        &self,
        team_types: &[Vec<PokemonType>],
        enemy_types: &[Vec<PokemonType>],
    ) -> Vec<usize> {
        enemy_types
            .iter()
            .enumerate()
            .filter(|(_, enemy)| {
                !team_types.iter().any(|my_types| {
                    my_types
                        .iter()
                        .any(|&atk| self.effectiveness_against_pokemon(atk, enemy) >= 2.0)
                })
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns types that are super-effective against 3+ team members.
    pub fn shared_weaknesses(&self, team_types: &[Vec<PokemonType>]) -> Vec<PokemonType> {
        PokemonType::ALL
            .iter()
            .filter(|&&attack_type| {
                let weak_count = team_types
                    .iter()
                    .filter(|pokemon_types| {
                        self.effectiveness_against_pokemon(attack_type, pokemon_types) >= 2.0
                    })
                    .count();
                weak_count >= 3
            })
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PokemonType::*;

    fn chart() -> TypeChart {
        TypeChart::fallback()
    }

    #[test]
    fn test_fire_vs_grass() {
        assert_eq!(chart().effectiveness(Fire, Grass), 2.0);
    }

    #[test]
    fn test_fire_vs_water() {
        assert_eq!(chart().effectiveness(Fire, Water), 0.5);
    }

    #[test]
    fn test_normal_vs_ghost() {
        assert_eq!(chart().effectiveness(Normal, Ghost), 0.0);
    }

    #[test]
    fn test_electric_vs_ground() {
        assert_eq!(chart().effectiveness(Electric, Ground), 0.0);
    }

    #[test]
    fn test_neutral_matchup() {
        assert_eq!(chart().effectiveness(Normal, Normal), 1.0);
    }

    #[test]
    fn test_dual_type_4x_weakness() {
        // Grass vs Water/Ground = 2.0 * 2.0 = 4.0
        let multiplier = chart().effectiveness_against_pokemon(Grass, &[Water, Ground]);
        assert_eq!(multiplier, 4.0);
    }

    #[test]
    fn test_dual_type_immunity() {
        // Electric vs Water/Ground: Electric vs Water = 2.0, Electric vs Ground = 0.0 → 0.0
        let multiplier = chart().effectiveness_against_pokemon(Electric, &[Water, Ground]);
        assert_eq!(multiplier, 0.0);
    }

    #[test]
    fn test_offensive_coverage_single_type() {
        let team_types = vec![vec![Fire]];
        let coverage = chart().team_offensive_coverage(&team_types);
        // Fire is super-effective against: Grass, Ice, Bug, Steel = 4/18
        assert!((coverage - 4.0 / 18.0).abs() < 0.001);
    }

    #[test]
    fn test_uncovered_types() {
        let team_types = vec![vec![Normal]];
        let uncovered = chart().uncovered_types(&team_types);
        // Normal has no super-effective matchups at all
        assert_eq!(uncovered.len(), 18);
    }

    #[test]
    fn test_fallback_chart_symmetry_check() {
        let c = chart();
        // Dragon vs Dragon should be 2.0
        assert_eq!(c.effectiveness(Dragon, Dragon), 2.0);
        // Fairy vs Dragon should be 2.0
        assert_eq!(c.effectiveness(Fairy, Dragon), 2.0);
        // Dragon vs Fairy should be 0.0
        assert_eq!(c.effectiveness(Dragon, Fairy), 0.0);
    }

    #[test]
    fn test_team_defensive_score_diverse_team() {
        // A well-typed team should have a good defensive score
        let team_types = vec![
            vec![Water, Ground],  // Swampert
            vec![Steel, Psychic], // Metagross
            vec![Fire, Flying],   // Charizard
            vec![Grass, Poison],  // Venusaur
            vec![Electric],       // Jolteon
            vec![Dark, Ghost],    // Spiritomb (pre-Fairy)
        ];
        let score = chart().team_defensive_score(&team_types);
        // Should be reasonable (>0.5)
        assert!(score > 0.5, "Score was {score}");
    }

    // --- Counter-team tests ---

    #[test]
    fn test_offensive_coverage_against_enemy() {
        let c = chart();
        // My team: Fire, Water
        let my_team = vec![vec![Fire], vec![Water]];
        // Enemy: Grass, Steel
        let enemy = vec![vec![Grass], vec![Steel]];
        // Fire hits Grass SE (2.0) and Steel SE (2.0) → 2/2 = 1.0
        let coverage = c.offensive_coverage_against(&my_team, &enemy);
        assert!((coverage - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_offensive_coverage_against_partial() {
        let c = chart();
        // My team: only Normal
        let my_team = vec![vec![Normal]];
        // Enemy: Ghost, Rock
        let enemy = vec![vec![Ghost], vec![Rock]];
        // Normal can't hit Ghost at all (0.0), can't hit Rock SE (0.5) → 0/2
        let coverage = c.offensive_coverage_against(&my_team, &enemy);
        assert!((coverage - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_defensive_score_against_enemy() {
        let c = chart();
        // My team: Water/Ground (Swampert), Steel/Psychic (Metagross)
        let my_team = vec![vec![Water, Ground], vec![Steel, Psychic]];
        // Enemy has Grass STAB — Grass hits Water/Ground at 4x, Steel/Psychic at 0.5x
        let enemy = vec![vec![Grass]];
        // Swampert is weak (4x), Metagross is safe → 1/2 safe = 0.5
        let score = c.defensive_score_against(&my_team, &enemy);
        assert!((score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_defensive_score_against_no_threats() {
        let c = chart();
        // My team: all Ghost types. Enemy: Normal STAB only.
        let my_team = vec![vec![Ghost], vec![Ghost]];
        let enemy = vec![vec![Normal]];
        // Normal can't hit Ghost at all → both safe → 1.0
        let score = c.defensive_score_against(&my_team, &enemy);
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_uncovered_enemies() {
        let c = chart();
        let my_team = vec![vec![Fire]];
        // Enemy: Water (Fire not SE), Grass (Fire SE), Steel (Fire SE)
        let enemy = vec![vec![Water], vec![Grass], vec![Steel]];
        let uncovered = c.uncovered_enemies(&my_team, &enemy);
        // Only Water is uncovered (index 0)
        assert_eq!(uncovered, vec![0]);
    }
}
