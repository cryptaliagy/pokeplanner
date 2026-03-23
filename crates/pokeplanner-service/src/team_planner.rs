use pokeplanner_core::{Pokemon, PokemonType, TeamMember, TeamPlan, TypeCoverage};

use crate::type_chart::TypeChart;

const BEAM_WIDTH: usize = 50;
const TEAM_SIZE: usize = 6;
const EXACT_THRESHOLD: usize = 25;

// Score weights
const OFFENSIVE_WEIGHT: f64 = 0.4;
const DEFENSIVE_WEIGHT: f64 = 0.3;
const BST_WEIGHT: f64 = 0.3;

// Approximate maximum BST for normalization (Mega Rayquaza: 780)
const MAX_BST: f64 = 780.0;

/// Pre-extracted enemy type data, or None for general planning.
pub struct TeamPlanner<'a> {
    type_chart: &'a TypeChart,
    counter: Option<CounterTarget>,
}

/// Holds the pre-extracted types of the enemy team.
struct CounterTarget {
    enemy_types: Vec<Vec<PokemonType>>,
}

impl<'a> TeamPlanner<'a> {
    pub fn new(type_chart: &'a TypeChart) -> Self {
        Self {
            type_chart,
            counter: None,
        }
    }

    /// Set an enemy team to counter. The planner will optimize against this team
    /// rather than general type coverage.
    pub fn with_counter_team(mut self, enemy: &[Pokemon]) -> Self {
        self.counter = Some(CounterTarget {
            enemy_types: enemy.iter().map(|p| p.types.clone()).collect(),
        });
        self
    }

    /// Plan the best teams from a list of candidate pokemon.
    /// Automatically chooses exact brute-force for small N or beam search for large N.
    pub fn plan_teams(&self, candidates: &[Pokemon], top_k: usize) -> Vec<TeamPlan> {
        if candidates.len() < TEAM_SIZE {
            if candidates.is_empty() {
                return Vec::new();
            }
            let team = candidates.to_vec();
            let plan = self.build_team_plan(&team);
            return vec![plan];
        }

        if candidates.len() <= EXACT_THRESHOLD {
            self.plan_exact(candidates, top_k)
        } else {
            self.plan_beam(candidates, top_k)
        }
    }

    /// Exact brute-force: enumerate all C(N, 6) combinations.
    fn plan_exact(&self, candidates: &[Pokemon], top_k: usize) -> Vec<TeamPlan> {
        let mut best: Vec<(f64, Vec<usize>)> = Vec::new();
        let n = candidates.len();
        let mut indices: [usize; TEAM_SIZE] = std::array::from_fn(|i| i);

        loop {
            let team: Vec<&Pokemon> = indices.iter().map(|&i| &candidates[i]).collect();
            let score = self.score_team_refs(&team);

            if best.len() < top_k {
                best.push((score, indices.to_vec()));
                best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            } else if score > best.last().unwrap().0 {
                best.pop();
                best.push((score, indices.to_vec()));
                best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            }

            if !next_combination(&mut indices, n) {
                break;
            }
        }

        best.into_iter()
            .map(|(_, idx)| {
                let team: Vec<Pokemon> = idx.iter().map(|&i| candidates[i].clone()).collect();
                self.build_team_plan(&team)
            })
            .collect()
    }

    /// Beam search: greedy expansion with beam width.
    fn plan_beam(&self, candidates: &[Pokemon], top_k: usize) -> Vec<TeamPlan> {
        let mut beam: Vec<(f64, Vec<usize>)> = vec![(0.0, Vec::new())];

        for _slot in 0..TEAM_SIZE {
            let mut next_beam: Vec<(f64, Vec<usize>)> = Vec::new();

            for (_, partial_indices) in &beam {
                let start_after = partial_indices.last().copied().map(|i| i + 1).unwrap_or(0);

                for i in start_after..candidates.len() {
                    let mut new_indices = partial_indices.clone();
                    new_indices.push(i);

                    let team: Vec<&Pokemon> =
                        new_indices.iter().map(|&idx| &candidates[idx]).collect();
                    let score = self.score_team_refs(&team);

                    next_beam.push((score, new_indices));
                }
            }

            next_beam.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            next_beam.truncate(BEAM_WIDTH);
            beam = next_beam;

            if beam.is_empty() {
                break;
            }
        }

        beam.truncate(top_k);
        beam.into_iter()
            .map(|(_, indices)| {
                let team: Vec<Pokemon> = indices.iter().map(|&i| candidates[i].clone()).collect();
                self.build_team_plan(&team)
            })
            .collect()
    }

    fn score_team_refs(&self, team: &[&Pokemon]) -> f64 {
        let team_types: Vec<Vec<PokemonType>> = team.iter().map(|p| p.types.clone()).collect();

        let (offensive, defensive) = match &self.counter {
            Some(ct) => (
                self.type_chart
                    .offensive_coverage_against(&team_types, &ct.enemy_types),
                self.type_chart
                    .defensive_score_against(&team_types, &ct.enemy_types),
            ),
            None => (
                self.type_chart.team_offensive_coverage(&team_types),
                self.type_chart.team_defensive_score(&team_types),
            ),
        };

        let total_bst: f64 = team.iter().map(|p| p.bst() as f64).sum();
        let bst_normalized = total_bst / (team.len() as f64 * MAX_BST);

        OFFENSIVE_WEIGHT * offensive + DEFENSIVE_WEIGHT * defensive + BST_WEIGHT * bst_normalized
    }

    fn build_team_plan(&self, team: &[Pokemon]) -> TeamPlan {
        let team_types: Vec<Vec<PokemonType>> = team.iter().map(|p| p.types.clone()).collect();

        let (offensive_score, defensive_score) = match &self.counter {
            Some(ct) => (
                self.type_chart
                    .offensive_coverage_against(&team_types, &ct.enemy_types),
                self.type_chart
                    .defensive_score_against(&team_types, &ct.enemy_types),
            ),
            None => (
                self.type_chart.team_offensive_coverage(&team_types),
                self.type_chart.team_defensive_score(&team_types),
            ),
        };

        let total_bst: u32 = team.iter().map(|p| p.bst()).sum();
        let bst_normalized = total_bst as f64 / (team.len() as f64 * MAX_BST);

        let score = OFFENSIVE_WEIGHT * offensive_score
            + DEFENSIVE_WEIGHT * defensive_score
            + BST_WEIGHT * bst_normalized;

        // Offensive coverage list: types/enemies we can hit SE
        let offensive_coverage: Vec<PokemonType> = match &self.counter {
            Some(ct) => {
                // In counter mode, list the types of enemy pokemon we can hit
                ct.enemy_types
                    .iter()
                    .filter(|enemy| {
                        team_types.iter().any(|my_types| {
                            my_types.iter().any(|&atk| {
                                self.type_chart.effectiveness_against_pokemon(atk, enemy) >= 2.0
                            })
                        })
                    })
                    .flat_map(|types| types.iter().copied())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            }
            None => PokemonType::ALL
                .iter()
                .filter(|&&target| {
                    team_types.iter().any(|ptypes| {
                        ptypes
                            .iter()
                            .any(|&atk| self.type_chart.effectiveness(atk, target) >= 2.0)
                    })
                })
                .copied()
                .collect(),
        };

        let defensive_weaknesses = match &self.counter {
            Some(ct) => {
                // In counter mode, list enemy STAB types that threaten our team
                ct.enemy_types
                    .iter()
                    .flat_map(|types| types.iter().copied())
                    .filter(|&atk_type| {
                        team_types.iter().any(|my_types| {
                            self.type_chart
                                .effectiveness_against_pokemon(atk_type, my_types)
                                >= 2.0
                        })
                    })
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            }
            None => self.type_chart.shared_weaknesses(&team_types),
        };

        let uncovered_types = match &self.counter {
            Some(ct) => {
                // Types of enemy pokemon we can't hit SE
                let uncovered_indices = self
                    .type_chart
                    .uncovered_enemies(&team_types, &ct.enemy_types);
                uncovered_indices
                    .iter()
                    .flat_map(|&i| ct.enemy_types[i].iter().copied())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            }
            None => self.type_chart.uncovered_types(&team_types),
        };

        let members = team
            .iter()
            .map(|p| {
                let (w2x, w4x) = self.type_chart.pokemon_weaknesses(&p.types);
                TeamMember {
                    pokemon: p.clone(),
                    weaknesses_2x: w2x,
                    weaknesses_4x: w4x,
                    recommended_moves: None,
                }
            })
            .collect();

        TeamPlan {
            team: members,
            total_bst,
            type_coverage: TypeCoverage {
                offensive_coverage,
                defensive_weaknesses,
                uncovered_types,
                coverage_score: offensive_score,
                move_coverage: None,
            },
            score,
        }
    }
}

/// Generate the next lexicographic combination of `indices` choosing from 0..n.
/// Returns false when no more combinations exist.
fn next_combination(indices: &mut [usize], n: usize) -> bool {
    let k = indices.len();
    let mut i = k;

    while i > 0 {
        i -= 1;
        if indices[i] < n - k + i {
            indices[i] += 1;
            for j in (i + 1)..k {
                indices[j] = indices[j - 1] + 1;
            }
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokeplanner_core::{BaseStats, PokemonType::*};

    fn make_pokemon(name: &str, types: Vec<PokemonType>, bst_total: u32) -> Pokemon {
        let per_stat = bst_total / 6;
        let remainder = bst_total % 6;
        Pokemon {
            species_name: name.to_string(),
            form_name: name.to_string(),
            pokedex_number: 1,
            types,
            stats: BaseStats {
                hp: per_stat + if remainder > 0 { 1 } else { 0 },
                attack: per_stat + if remainder > 1 { 1 } else { 0 },
                defense: per_stat + if remainder > 2 { 1 } else { 0 },
                special_attack: per_stat + if remainder > 3 { 1 } else { 0 },
                special_defense: per_stat + if remainder > 4 { 1 } else { 0 },
                speed: per_stat,
            },
            is_default_form: true,
        }
    }

    #[test]
    fn test_empty_candidates() {
        let chart = TypeChart::fallback();
        let planner = TeamPlanner::new(&chart);
        let result = planner.plan_teams(&[], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fewer_than_six_candidates() {
        let chart = TypeChart::fallback();
        let planner = TeamPlanner::new(&chart);
        let candidates = vec![
            make_pokemon("pikachu", vec![Electric], 320),
            make_pokemon("charizard", vec![Fire, Flying], 534),
        ];
        let result = planner.plan_teams(&candidates, 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].team.len(), 2);
    }

    #[test]
    fn test_exact_algorithm_small_set() {
        let chart = TypeChart::fallback();
        let planner = TeamPlanner::new(&chart);

        let candidates = vec![
            make_pokemon("fire_mon", vec![Fire], 500),
            make_pokemon("water_mon", vec![Water], 500),
            make_pokemon("grass_mon", vec![Grass], 500),
            make_pokemon("electric_mon", vec![Electric], 500),
            make_pokemon("ice_mon", vec![Ice], 500),
            make_pokemon("fighting_mon", vec![Fighting], 500),
            make_pokemon("psychic_mon", vec![Psychic], 500),
            make_pokemon("dark_mon", vec![Dark], 500),
            make_pokemon("fairy_mon", vec![Fairy], 500),
            make_pokemon("ground_mon", vec![Ground], 500),
        ];

        let results = planner.plan_teams(&candidates, 3);
        assert_eq!(results.len(), 3);

        for i in 0..results.len() - 1 {
            assert!(results[i].score >= results[i + 1].score);
        }

        for plan in &results {
            assert_eq!(plan.team.len(), 6);
        }
    }

    #[test]
    fn test_beam_algorithm_large_set() {
        let chart = TypeChart::fallback();
        let planner = TeamPlanner::new(&chart);

        let mut candidates = Vec::new();
        for (i, &ptype) in PokemonType::ALL.iter().enumerate() {
            candidates.push(make_pokemon(
                &format!("mon_{i}"),
                vec![ptype],
                400 + (i as u32 * 10),
            ));
        }
        candidates.push(make_pokemon("dual_1", vec![Fire, Flying], 534));
        candidates.push(make_pokemon("dual_2", vec![Water, Ground], 535));
        candidates.push(make_pokemon("dual_3", vec![Steel, Psychic], 600));
        candidates.push(make_pokemon("dual_4", vec![Grass, Poison], 525));
        candidates.push(make_pokemon("dual_5", vec![Dark, Ghost], 485));
        candidates.push(make_pokemon("dual_6", vec![Ice, Ground], 530));
        candidates.push(make_pokemon("dual_7", vec![Electric, Steel], 520));
        candidates.push(make_pokemon("dual_8", vec![Dragon, Fairy], 600));

        let results = planner.plan_teams(&candidates, 5);
        assert_eq!(results.len(), 5);

        for plan in &results {
            assert_eq!(plan.team.len(), 6);
            assert!(plan.score > 0.0);
            assert!(plan.total_bst > 0);
        }
    }

    #[test]
    fn test_top_k_respected() {
        let chart = TypeChart::fallback();
        let planner = TeamPlanner::new(&chart);

        let candidates: Vec<Pokemon> = (0..10)
            .map(|i| make_pokemon(&format!("mon_{i}"), vec![PokemonType::ALL[i % 18]], 500))
            .collect();

        let results = planner.plan_teams(&candidates, 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_next_combination() {
        let mut indices = [0, 1, 2];
        let n = 4;

        assert!(next_combination(&mut indices, n));
        assert_eq!(indices, [0, 1, 3]);

        assert!(next_combination(&mut indices, n));
        assert_eq!(indices, [0, 2, 3]);

        assert!(next_combination(&mut indices, n));
        assert_eq!(indices, [1, 2, 3]);

        assert!(!next_combination(&mut indices, n));
    }

    #[test]
    fn test_type_coverage_populated() {
        let chart = TypeChart::fallback();
        let planner = TeamPlanner::new(&chart);

        let candidates = vec![
            make_pokemon("fire", vec![Fire], 500),
            make_pokemon("water", vec![Water], 500),
            make_pokemon("grass", vec![Grass], 500),
            make_pokemon("electric", vec![Electric], 500),
            make_pokemon("ice", vec![Ice], 500),
            make_pokemon("fighting", vec![Fighting], 500),
        ];

        let results = planner.plan_teams(&candidates, 1);
        assert_eq!(results.len(), 1);

        let plan = &results[0];
        assert!(!plan.type_coverage.offensive_coverage.is_empty());
        assert!(plan.type_coverage.coverage_score > 0.0);
    }

    // --- Counter-team tests ---

    #[test]
    fn test_counter_team_prefers_super_effective() {
        let chart = TypeChart::fallback();

        // Enemy team: all Water types
        let enemy = vec![
            make_pokemon("enemy_1", vec![Water], 500),
            make_pokemon("enemy_2", vec![Water], 500),
            make_pokemon("enemy_3", vec![Water], 500),
        ];

        // Candidates: Grass (SE vs Water), Fire (not SE), Normal (not SE), etc.
        let candidates = vec![
            make_pokemon("grass_1", vec![Grass], 500),
            make_pokemon("electric_1", vec![Electric], 500),
            make_pokemon("fire_1", vec![Fire], 500),
            make_pokemon("normal_1", vec![Normal], 500),
            make_pokemon("grass_2", vec![Grass], 500),
            make_pokemon("electric_2", vec![Electric], 500),
            make_pokemon("normal_2", vec![Normal], 500),
            make_pokemon("normal_3", vec![Normal], 500),
        ];

        // General planner
        let general = TeamPlanner::new(&chart);
        let general_result = general.plan_teams(&candidates, 1);

        // Counter planner
        let counter = TeamPlanner::new(&chart).with_counter_team(&enemy);
        let counter_result = counter.plan_teams(&candidates, 1);

        // Counter planner should score higher on offensive coverage against Water
        assert!(
            counter_result[0].type_coverage.coverage_score
                >= general_result[0].type_coverage.coverage_score
                || counter_result[0].score >= general_result[0].score
        );

        // Counter team should include Grass and/or Electric types
        let has_se_type = counter_result[0]
            .team
            .iter()
            .any(|m| m.pokemon.types.contains(&Grass) || m.pokemon.types.contains(&Electric));
        assert!(
            has_se_type,
            "Counter team should include types SE against Water"
        );
    }

    #[test]
    fn test_counter_team_with_fewer_than_six_enemies() {
        let chart = TypeChart::fallback();
        // Only 2 enemy pokemon — should still work
        let enemy = vec![
            make_pokemon("enemy_fire", vec![Fire], 500),
            make_pokemon("enemy_grass", vec![Grass], 500),
        ];

        let candidates = vec![
            make_pokemon("water", vec![Water], 500),
            make_pokemon("fire", vec![Fire], 500),
            make_pokemon("ice", vec![Ice], 500),
            make_pokemon("rock", vec![Rock], 500),
            make_pokemon("ground", vec![Ground], 500),
            make_pokemon("flying", vec![Flying], 500),
        ];

        let planner = TeamPlanner::new(&chart).with_counter_team(&enemy);
        let results = planner.plan_teams(&candidates, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].team.len(), 6);
    }
}
