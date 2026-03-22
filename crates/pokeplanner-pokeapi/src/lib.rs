pub mod cache;
pub mod client;
pub mod traits;
pub mod types;

pub use client::PokeApiHttpClient;
pub use traits::{PokeApiClient, TypeEffectivenessData, TypeEffectivenessEntry};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VersionGroupInfo {
    pub name: String,
    pub versions: Vec<String>,
    pub pokedexes: Vec<String>,
}
