use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedApiResource {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedApiResourceList {
    pub count: u32,
    pub results: Vec<NamedApiResource>,
}

// --- Version Group ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionGroupResponse {
    pub id: u32,
    pub name: String,
    pub pokedexes: Vec<NamedApiResource>,
    pub versions: Vec<NamedApiResource>,
    pub generation: NamedApiResource,
}

// --- Pokedex ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokedexEntry {
    pub entry_number: u32,
    pub pokemon_species: NamedApiResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokedexResponse {
    pub id: u32,
    pub name: String,
    pub pokemon_entries: Vec<PokedexEntry>,
}

// --- Pokemon Species ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeciesVariety {
    pub is_default: bool,
    pub pokemon: NamedApiResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonSpeciesResponse {
    pub id: u32,
    pub name: String,
    pub varieties: Vec<SpeciesVariety>,
}

// --- Pokemon ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonStatEntry {
    pub base_stat: u32,
    pub stat: NamedApiResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonTypeSlot {
    pub slot: u32,
    #[serde(rename = "type")]
    pub type_info: NamedApiResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonResponse {
    pub id: u32,
    pub name: String,
    pub stats: Vec<PokemonStatEntry>,
    pub types: Vec<PokemonTypeSlot>,
    pub species: NamedApiResource,
}

// --- Pokemon (full, with moves for learnset queries) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonMoveVersionDetail {
    pub level_learned_at: u32,
    pub version_group: NamedApiResource,
    pub move_learn_method: NamedApiResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonMoveEntry {
    #[serde(rename = "move")]
    pub move_info: NamedApiResource,
    pub version_group_details: Vec<PokemonMoveVersionDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokemonFullResponse {
    pub id: u32,
    pub name: String,
    pub stats: Vec<PokemonStatEntry>,
    pub types: Vec<PokemonTypeSlot>,
    pub species: NamedApiResource,
    pub moves: Vec<PokemonMoveEntry>,
}

// --- Move ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveEffectEntry {
    pub effect: String,
    pub short_effect: String,
    pub language: NamedApiResource,
}

/// Metadata from the PokeAPI `meta` object on a move response.
///
/// - `drain`: percentage of damage drained as HP. Negative = recoil (user loses HP),
///   positive = HP drain (user recovers HP), 0 = neither.
/// - `stat_chance`: probability that the move's `stat_changes` apply. **0 means guaranteed**
///   (not "never") — this is PokeAPI's convention. Values 1–99 are probabilities; ≥100 is guaranteed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveMeta {
    pub drain: i32,
    pub healing: i32,
    pub crit_rate: i32,
    pub ailment_chance: i32,
    pub flinch_chance: i32,
    pub stat_chance: i32,
}

/// A stat change entry from the top-level `stat_changes` array on a move response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveStatChangeResponse {
    pub change: i32,
    pub stat: NamedApiResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveResponse {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub type_info: NamedApiResource,
    pub power: Option<u32>,
    pub accuracy: Option<u32>,
    pub pp: Option<u32>,
    pub damage_class: NamedApiResource,
    pub priority: i32,
    #[serde(default)]
    pub effect_entries: Vec<MoveEffectEntry>,
    #[serde(default)]
    pub meta: Option<MoveMeta>,
    #[serde(default)]
    pub stat_changes: Vec<MoveStatChangeResponse>,
}

// --- Type ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageRelations {
    pub double_damage_from: Vec<NamedApiResource>,
    pub double_damage_to: Vec<NamedApiResource>,
    pub half_damage_from: Vec<NamedApiResource>,
    pub half_damage_to: Vec<NamedApiResource>,
    pub no_damage_from: Vec<NamedApiResource>,
    pub no_damage_to: Vec<NamedApiResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeResponse {
    pub id: u32,
    pub name: String,
    pub damage_relations: DamageRelations,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pokemon_response_deser() {
        let json = r#"{
            "id": 6,
            "name": "charizard",
            "stats": [
                {"base_stat": 78, "effort": 0, "stat": {"name": "hp", "url": ""}},
                {"base_stat": 84, "effort": 0, "stat": {"name": "attack", "url": ""}}
            ],
            "types": [
                {"slot": 1, "type": {"name": "fire", "url": ""}},
                {"slot": 2, "type": {"name": "flying", "url": ""}}
            ],
            "species": {"name": "charizard", "url": ""}
        }"#;
        let pokemon: PokemonResponse = serde_json::from_str(json).unwrap();
        assert_eq!(pokemon.name, "charizard");
        assert_eq!(pokemon.stats.len(), 2);
        assert_eq!(pokemon.types.len(), 2);
        assert_eq!(pokemon.types[0].type_info.name, "fire");
    }

    #[test]
    fn test_species_response_deser() {
        let json = r#"{
            "id": 6,
            "name": "charizard",
            "varieties": [
                {"is_default": true, "pokemon": {"name": "charizard", "url": ""}},
                {"is_default": false, "pokemon": {"name": "charizard-mega-x", "url": ""}}
            ]
        }"#;
        let species: PokemonSpeciesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(species.varieties.len(), 2);
        assert!(species.varieties[0].is_default);
        assert!(!species.varieties[1].is_default);
    }

    #[test]
    fn test_version_group_response_deser() {
        let json = r#"{
            "id": 1,
            "name": "red-blue",
            "pokedexes": [{"name": "kanto", "url": ""}],
            "versions": [{"name": "red", "url": ""}, {"name": "blue", "url": ""}],
            "generation": {"name": "generation-i", "url": ""}
        }"#;
        let vg: VersionGroupResponse = serde_json::from_str(json).unwrap();
        assert_eq!(vg.name, "red-blue");
        assert_eq!(vg.pokedexes.len(), 1);
        assert_eq!(vg.versions.len(), 2);
        assert_eq!(vg.generation.name, "generation-i");
    }

    #[test]
    fn test_move_meta_deser() {
        let json = r#"{
            "drain": -25,
            "healing": 0,
            "crit_rate": 0,
            "ailment_chance": 0,
            "flinch_chance": 0,
            "stat_chance": 0
        }"#;
        let meta: MoveMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.drain, -25);
        assert_eq!(meta.stat_chance, 0);
    }

    #[test]
    fn test_move_stat_change_response_deser() {
        let json = r#"{
            "change": -2,
            "stat": {"name": "special-attack", "url": ""}
        }"#;
        let sc: MoveStatChangeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(sc.change, -2);
        assert_eq!(sc.stat.name, "special-attack");
    }

    #[test]
    fn test_move_response_with_meta_and_stat_changes() {
        let json = r#"{
            "id": 315, "name": "overheat",
            "type": {"name": "fire", "url": ""},
            "power": 130, "accuracy": 90, "pp": 5,
            "damage_class": {"name": "special", "url": ""},
            "priority": 0, "effect_entries": [],
            "meta": {"drain": 0, "healing": 0, "crit_rate": 0, "ailment_chance": 0, "flinch_chance": 0, "stat_chance": 0},
            "stat_changes": [{"change": -2, "stat": {"name": "special-attack", "url": ""}}]
        }"#;
        let resp: MoveResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.name, "overheat");
        let meta = resp.meta.unwrap();
        assert_eq!(meta.stat_chance, 0);
        assert_eq!(meta.drain, 0);
        assert_eq!(resp.stat_changes.len(), 1);
        assert_eq!(resp.stat_changes[0].change, -2);
        assert_eq!(resp.stat_changes[0].stat.name, "special-attack");
    }

    #[test]
    fn test_move_response_without_meta_backward_compat() {
        let json = r#"{
            "id": 85, "name": "thunderbolt",
            "type": {"name": "electric", "url": ""},
            "power": 90, "accuracy": 100, "pp": 15,
            "damage_class": {"name": "special", "url": ""},
            "priority": 0, "effect_entries": []
        }"#;
        let resp: MoveResponse = serde_json::from_str(json).unwrap();
        assert!(resp.meta.is_none());
        assert!(resp.stat_changes.is_empty());
    }

    #[test]
    fn test_move_response_with_recoil() {
        let json = r#"{
            "id": 394, "name": "flare-blitz",
            "type": {"name": "fire", "url": ""},
            "power": 120, "accuracy": 100, "pp": 15,
            "damage_class": {"name": "physical", "url": ""},
            "priority": 0, "effect_entries": [],
            "meta": {"drain": -33, "healing": 0, "crit_rate": 0, "ailment_chance": 10, "flinch_chance": 0, "stat_chance": 0},
            "stat_changes": []
        }"#;
        let resp: MoveResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.meta.unwrap().drain, -33);
        assert!(resp.stat_changes.is_empty());
    }
}
