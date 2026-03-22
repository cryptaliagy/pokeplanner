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
            "versions": [{"name": "red", "url": ""}, {"name": "blue", "url": ""}]
        }"#;
        let vg: VersionGroupResponse = serde_json::from_str(json).unwrap();
        assert_eq!(vg.name, "red-blue");
        assert_eq!(vg.pokedexes.len(), 1);
        assert_eq!(vg.versions.len(), 2);
    }
}
