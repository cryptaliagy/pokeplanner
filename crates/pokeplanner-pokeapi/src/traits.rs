use std::future::Future;

use pokeplanner_core::{AppError, Pokemon, PokemonType};

use crate::VersionGroupInfo;

/// Trait for PokeAPI client, enabling mockability in tests.
/// Follows the same `impl Future` pattern as the `Storage` trait.
pub trait PokeApiClient: Send + Sync + 'static {
    fn get_version_groups(
        &self,
        no_cache: bool,
    ) -> impl Future<Output = Result<Vec<VersionGroupInfo>, AppError>> + Send;

    fn get_game_pokemon(
        &self,
        version_group: &str,
        no_cache: bool,
        include_variants: bool,
    ) -> impl Future<Output = Result<Vec<Pokemon>, AppError>> + Send;

    fn get_pokemon(
        &self,
        name: &str,
        no_cache: bool,
    ) -> impl Future<Output = Result<Pokemon, AppError>> + Send;

    fn get_species_varieties(
        &self,
        species_name: &str,
        no_cache: bool,
    ) -> impl Future<Output = Result<Vec<Pokemon>, AppError>> + Send;

    fn get_pokedex_pokemon(
        &self,
        pokedex_name: &str,
        no_cache: bool,
        include_variants: bool,
    ) -> impl Future<Output = Result<Vec<Pokemon>, AppError>> + Send;

    fn get_type_chart(
        &self,
        no_cache: bool,
    ) -> impl Future<Output = Result<TypeEffectivenessData, AppError>> + Send;
}

/// Raw type effectiveness data from PokeAPI, to be consumed by the TypeChart in the service layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeEffectivenessData {
    pub entries: Vec<TypeEffectivenessEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeEffectivenessEntry {
    pub attack_type: PokemonType,
    pub defend_type: PokemonType,
    pub multiplier: f64,
}
