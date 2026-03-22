use serde::{Deserialize, Serialize};

/// The 18 standard Pokemon types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PokemonType {
    Normal,
    Fire,
    Water,
    Electric,
    Grass,
    Ice,
    Fighting,
    Poison,
    Ground,
    Flying,
    Psychic,
    Bug,
    Rock,
    Ghost,
    Dragon,
    Dark,
    Steel,
    Fairy,
}

impl PokemonType {
    pub const ALL: [PokemonType; 18] = [
        PokemonType::Normal,
        PokemonType::Fire,
        PokemonType::Water,
        PokemonType::Electric,
        PokemonType::Grass,
        PokemonType::Ice,
        PokemonType::Fighting,
        PokemonType::Poison,
        PokemonType::Ground,
        PokemonType::Flying,
        PokemonType::Psychic,
        PokemonType::Bug,
        PokemonType::Rock,
        PokemonType::Ghost,
        PokemonType::Dragon,
        PokemonType::Dark,
        PokemonType::Steel,
        PokemonType::Fairy,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for PokemonType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{:?}", self));
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseStats {
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub special_attack: u32,
    pub special_defense: u32,
    pub speed: u32,
}

impl BaseStats {
    pub fn total(&self) -> u32 {
        self.hp
            + self.attack
            + self.defense
            + self.special_attack
            + self.special_defense
            + self.speed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pokemon {
    pub species_name: String,
    pub form_name: String,
    pub pokedex_number: u32,
    pub types: Vec<PokemonType>,
    pub stats: BaseStats,
    pub is_default_form: bool,
}

impl Pokemon {
    pub fn display_name(&self) -> &str {
        &self.form_name
    }

    pub fn bst(&self) -> u32 {
        self.stats.total()
    }
}

/// How a pokemon learns a move.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum LearnMethod {
    LevelUp,
    Machine,
    Egg,
    Tutor,
    #[serde(other)]
    Other,
}

impl std::fmt::Display for LearnMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LearnMethod::LevelUp => write!(f, "level-up"),
            LearnMethod::Machine => write!(f, "machine"),
            LearnMethod::Egg => write!(f, "egg"),
            LearnMethod::Tutor => write!(f, "tutor"),
            LearnMethod::Other => write!(f, "other"),
        }
    }
}

/// A single entry in a pokemon's learnset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnsetEntry {
    pub move_name: String,
    pub learn_method: LearnMethod,
    pub level: u32,
    pub version_group: String,
}

/// Detailed move information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Move {
    pub name: String,
    pub move_type: PokemonType,
    pub power: Option<u32>,
    pub accuracy: Option<u32>,
    pub pp: Option<u32>,
    pub damage_class: String,
    pub priority: i32,
    pub effect: Option<String>,
}

/// A learnset entry enriched with move details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedLearnsetEntry {
    pub move_details: Move,
    pub learn_method: LearnMethod,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

impl HealthResponse {
    pub fn ok() -> Self {
        Self {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_ok() {
        let h = HealthResponse::ok();
        assert_eq!(h.status, "ok");
    }

    #[test]
    fn test_base_stats_total() {
        let stats = BaseStats {
            hp: 78,
            attack: 84,
            defense: 78,
            special_attack: 109,
            special_defense: 85,
            speed: 100,
        };
        assert_eq!(stats.total(), 534);
    }

    #[test]
    fn test_pokemon_type_serde_roundtrip() {
        let t = PokemonType::Fire;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"fire\"");
        let deserialized: PokemonType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, t);
    }

    #[test]
    fn test_pokemon_type_all_count() {
        assert_eq!(PokemonType::ALL.len(), 18);
    }

    #[test]
    fn test_pokemon_type_indices_unique() {
        let indices: Vec<usize> = PokemonType::ALL.iter().map(|t| t.index()).collect();
        for (i, idx) in indices.iter().enumerate() {
            assert_eq!(*idx, i);
        }
    }

    #[test]
    fn test_pokemon_bst() {
        let pokemon = Pokemon {
            species_name: "charizard".to_string(),
            form_name: "charizard".to_string(),
            pokedex_number: 6,
            types: vec![PokemonType::Fire, PokemonType::Flying],
            stats: BaseStats {
                hp: 78,
                attack: 84,
                defense: 78,
                special_attack: 109,
                special_defense: 85,
                speed: 100,
            },
            is_default_form: true,
        };
        assert_eq!(pokemon.bst(), 534);
        assert_eq!(pokemon.display_name(), "charizard");
    }

    #[test]
    fn test_pokemon_serialization_roundtrip() {
        let pokemon = Pokemon {
            species_name: "charizard".to_string(),
            form_name: "charizard-mega-x".to_string(),
            pokedex_number: 6,
            types: vec![PokemonType::Fire, PokemonType::Dragon],
            stats: BaseStats {
                hp: 78,
                attack: 130,
                defense: 111,
                special_attack: 130,
                special_defense: 85,
                speed: 100,
            },
            is_default_form: false,
        };
        let json = serde_json::to_string(&pokemon).unwrap();
        let deserialized: Pokemon = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.form_name, "charizard-mega-x");
        assert_eq!(
            deserialized.types,
            vec![PokemonType::Fire, PokemonType::Dragon]
        );
        assert!(!deserialized.is_default_form);
    }
}
