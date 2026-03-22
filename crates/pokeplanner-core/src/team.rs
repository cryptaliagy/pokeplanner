use serde::{Deserialize, Serialize};

use crate::model::{Pokemon, PokemonType};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TeamSource {
    Game { version_groups: Vec<String> },
    Pokedex { pokedex_name: String },
    Custom { pokemon_names: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamPlanRequest {
    pub source: TeamSource,
    #[serde(default)]
    pub min_bst: Option<u32>,
    #[serde(default)]
    pub no_cache: bool,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default = "default_include_variants")]
    pub include_variants: bool,
    /// Specific pokemon form names to exclude from candidates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    /// Species names to exclude (removes all forms/variants of that species).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_species: Vec<String>,
    /// Variant type keywords to exclude (e.g., "mega", "gmax", "alola").
    /// Filters out non-default forms whose variant suffix contains any of these keywords.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_variant_types: Vec<String>,
    /// Enemy pokemon names to counter-team against. When set, the planner
    /// optimizes for coverage against this specific team rather than general coverage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter_team: Option<Vec<String>>,
}

fn default_include_variants() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub pokemon: Pokemon,
    pub weaknesses_2x: Vec<PokemonType>,
    pub weaknesses_4x: Vec<PokemonType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamPlan {
    pub team: Vec<TeamMember>,
    pub total_bst: u32,
    pub type_coverage: TypeCoverage,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCoverage {
    pub offensive_coverage: Vec<PokemonType>,
    pub defensive_weaknesses: Vec<PokemonType>,
    pub uncovered_types: Vec<PokemonType>,
    pub coverage_score: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    Bst,
    Hp,
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
    Name,
    PokedexNumber,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

/// Common parameters for querying and filtering pokemon lists.
#[derive(Debug, Clone, Default)]
pub struct PokemonQueryParams {
    pub min_bst: Option<u32>,
    pub no_cache: bool,
    pub sort_by: Option<SortField>,
    pub sort_order: SortOrder,
    pub include_variants: bool,
    pub limit: Option<usize>,
}

/// Sort a slice of pokemon by the given field and order.
pub fn sort_pokemon(pokemon: &mut [Pokemon], field: SortField, order: SortOrder) {
    pokemon.sort_by(|a, b| {
        let cmp = match field {
            SortField::Bst => a.bst().cmp(&b.bst()),
            SortField::Hp => a.stats.hp.cmp(&b.stats.hp),
            SortField::Attack => a.stats.attack.cmp(&b.stats.attack),
            SortField::Defense => a.stats.defense.cmp(&b.stats.defense),
            SortField::SpecialAttack => a.stats.special_attack.cmp(&b.stats.special_attack),
            SortField::SpecialDefense => a.stats.special_defense.cmp(&b.stats.special_defense),
            SortField::Speed => a.stats.speed.cmp(&b.stats.speed),
            SortField::Name => a.form_name.cmp(&b.form_name),
            SortField::PokedexNumber => a.pokedex_number.cmp(&b.pokedex_number),
        };
        match order {
            SortOrder::Asc => cmp,
            SortOrder::Desc => cmp.reverse(),
        }
    });
}

/// Filter, sort, and limit a list of pokemon.
pub fn filter_sort_limit(
    mut pokemon: Vec<Pokemon>,
    min_bst: Option<u32>,
    sort_by: Option<SortField>,
    sort_order: SortOrder,
    limit: Option<usize>,
) -> Vec<Pokemon> {
    if let Some(min) = min_bst {
        pokemon.retain(|p| p.bst() >= min);
    }
    if let Some(field) = sort_by {
        sort_pokemon(&mut pokemon, field, sort_order);
    }
    if let Some(n) = limit {
        pokemon.truncate(n);
    }
    pokemon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_source_game_serde() {
        let source = TeamSource::Game {
            version_groups: vec!["scarlet-violet".to_string()],
        };
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: TeamSource = serde_json::from_str(&json).unwrap();
        match deserialized {
            TeamSource::Game { version_groups } => {
                assert_eq!(version_groups, vec!["scarlet-violet"])
            }
            _ => panic!("expected Game variant"),
        }
    }

    #[test]
    fn test_team_source_multi_game_serde() {
        let source = TeamSource::Game {
            version_groups: vec!["red-blue".to_string(), "gold-silver".to_string()],
        };
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: TeamSource = serde_json::from_str(&json).unwrap();
        match deserialized {
            TeamSource::Game { version_groups } => {
                assert_eq!(version_groups, vec!["red-blue", "gold-silver"])
            }
            _ => panic!("expected Game variant"),
        }
    }

    #[test]
    fn test_team_source_custom_serde() {
        let source = TeamSource::Custom {
            pokemon_names: vec!["pikachu".to_string(), "charizard".to_string()],
        };
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: TeamSource = serde_json::from_str(&json).unwrap();
        match deserialized {
            TeamSource::Custom { pokemon_names } => {
                assert_eq!(pokemon_names, vec!["pikachu", "charizard"]);
            }
            _ => panic!("expected Custom variant"),
        }
    }

    #[test]
    fn test_team_plan_request_defaults() {
        let json = r#"{"source":{"game":{"version_groups":["red-blue"]}}}"#;
        let req: TeamPlanRequest = serde_json::from_str(json).unwrap();
        assert!(req.min_bst.is_none());
        assert!(!req.no_cache);
        assert!(req.top_k.is_none());
        assert!(req.include_variants);
        assert!(req.exclude_variant_types.is_empty());
    }

    #[test]
    fn test_team_plan_request_exclude_variant_types_serde() {
        let json = r#"{"source":{"game":{"version_groups":["red-blue"]}},"exclude_variant_types":["mega","gmax"]}"#;
        let req: TeamPlanRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.exclude_variant_types, vec!["mega", "gmax"]);

        // Round-trip: should serialize back
        let serialized = serde_json::to_string(&req).unwrap();
        let req2: TeamPlanRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req2.exclude_variant_types, vec!["mega", "gmax"]);
    }

    #[test]
    fn test_team_plan_request_exclude_variant_types_omitted() {
        // When omitted, should not appear in serialized output
        let req = TeamPlanRequest {
            source: TeamSource::Game {
                version_groups: vec!["red-blue".to_string()],
            },
            min_bst: None,
            no_cache: false,
            top_k: None,
            include_variants: true,
            exclude: Vec::new(),
            exclude_species: Vec::new(),
            exclude_variant_types: Vec::new(),
            counter_team: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("exclude_variant_types"));
    }

    #[test]
    fn test_sort_field_serde() {
        let field = SortField::SpecialAttack;
        let json = serde_json::to_string(&field).unwrap();
        assert_eq!(json, "\"special_attack\"");
        let deserialized: SortField = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SortField::SpecialAttack);
    }

    #[test]
    fn test_sort_order_default() {
        assert_eq!(SortOrder::default(), SortOrder::Asc);
    }
}
