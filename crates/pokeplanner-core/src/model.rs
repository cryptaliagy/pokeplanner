use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type PokemonId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pokemon {
    pub id: PokemonId,
    pub name: String,
    pub pokedex_number: u32,
    pub pokemon_type: String,
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
}
