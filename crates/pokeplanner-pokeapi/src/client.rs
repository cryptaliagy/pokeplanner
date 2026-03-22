use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use governor::{Quota, RateLimiter};
use pokeplanner_core::{AppError, BaseStats, Pokemon, PokemonType};
use tracing::{debug, warn};

use crate::cache::DiskCache;
use crate::traits::{PokeApiClient, TypeEffectivenessData, TypeEffectivenessEntry};
use crate::types::*;
use crate::VersionGroupInfo;

const BASE_URL: &str = "https://pokeapi.co/api/v2";
const CONCURRENT_REQUESTS: usize = 10;

type DefaultRateLimiter =
    RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>;

pub struct PokeApiHttpClient {
    http: reqwest::Client,
    cache: DiskCache,
    rate_limiter: Arc<DefaultRateLimiter>,
}

impl PokeApiHttpClient {
    pub async fn new(cache_path: std::path::PathBuf) -> Result<Self, AppError> {
        let cache = DiskCache::new(cache_path).await?;
        let quota = Quota::per_second(NonZeroU32::new(100).unwrap())
            .allow_burst(NonZeroU32::new(10).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Ok(Self {
            http: reqwest::Client::new(),
            cache,
            rate_limiter,
        })
    }

    async fn fetch<T: serde::de::DeserializeOwned + serde::Serialize>(
        &self,
        url: &str,
        cache_category: &str,
        cache_key: &str,
        no_cache: bool,
    ) -> Result<T, AppError> {
        // Check cache first
        if let Some(cached) = self.cache.get::<T>(cache_category, cache_key, no_cache).await {
            return Ok(cached);
        }

        // Rate limit
        self.rate_limiter.until_ready().await;

        debug!("Fetching {url}");
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::PokeApi(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AppError::PokeApi(format!(
                "PokeAPI returned status {} for {url}",
                response.status()
            )));
        }

        let data: T = response
            .json()
            .await
            .map_err(|e| AppError::PokeApi(format!("Failed to deserialize response: {e}")))?;

        // Cache the result
        if let Err(e) = self.cache.set(cache_category, cache_key, &data).await {
            warn!("Failed to cache {cache_category}/{cache_key}: {e}");
        }

        Ok(data)
    }

    fn parse_pokemon_type(name: &str) -> Option<PokemonType> {
        serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
    }

    fn convert_pokemon_response(
        &self,
        resp: &PokemonResponse,
        pokedex_number: u32,
        is_default: bool,
    ) -> Result<Pokemon, AppError> {
        let types: Vec<PokemonType> = resp
            .types
            .iter()
            .filter_map(|t| Self::parse_pokemon_type(&t.type_info.name))
            .collect();

        if types.is_empty() {
            return Err(AppError::PokeApi(format!(
                "Pokemon {} has no recognized types",
                resp.name
            )));
        }

        let mut stats = BaseStats {
            hp: 0,
            attack: 0,
            defense: 0,
            special_attack: 0,
            special_defense: 0,
            speed: 0,
        };

        for stat_entry in &resp.stats {
            match stat_entry.stat.name.as_str() {
                "hp" => stats.hp = stat_entry.base_stat,
                "attack" => stats.attack = stat_entry.base_stat,
                "defense" => stats.defense = stat_entry.base_stat,
                "special-attack" => stats.special_attack = stat_entry.base_stat,
                "special-defense" => stats.special_defense = stat_entry.base_stat,
                "speed" => stats.speed = stat_entry.base_stat,
                _ => {}
            }
        }

        Ok(Pokemon {
            species_name: resp.species.name.clone(),
            form_name: resp.name.clone(),
            pokedex_number,
            types,
            stats,
            is_default_form: is_default,
        })
    }

    async fn fetch_species_pokemon(
        &self,
        species_name: &str,
        pokedex_number: u32,
        no_cache: bool,
        include_variants: bool,
    ) -> Result<Vec<Pokemon>, AppError> {
        let species_url = format!("{BASE_URL}/pokemon-species/{species_name}");
        let species: PokemonSpeciesResponse = self
            .fetch(&species_url, "species", species_name, no_cache)
            .await?;

        let varieties: Vec<&SpeciesVariety> = if include_variants {
            species.varieties.iter().collect()
        } else {
            species.varieties.iter().filter(|v| v.is_default).collect()
        };

        let mut pokemon_list = Vec::new();
        for variety in varieties {
            let pokemon_name = &variety.pokemon.name;
            let pokemon_url = format!("{BASE_URL}/pokemon/{pokemon_name}");
            match self
                .fetch::<PokemonResponse>(&pokemon_url, "pokemon", pokemon_name, no_cache)
                .await
            {
                Ok(resp) => {
                    match self.convert_pokemon_response(&resp, pokedex_number, variety.is_default) {
                        Ok(p) => pokemon_list.push(p),
                        Err(e) => warn!("Skipping {pokemon_name}: {e}"),
                    }
                }
                Err(e) => warn!("Failed to fetch {pokemon_name}: {e}"),
            }
        }

        Ok(pokemon_list)
    }
}

impl PokeApiClient for PokeApiHttpClient {
    async fn get_version_groups(&self, no_cache: bool) -> Result<Vec<VersionGroupInfo>, AppError> {
        let url = format!("{BASE_URL}/version-group?limit=100");
        let list: NamedApiResourceList = self
            .fetch(&url, "meta", "version-groups-list", no_cache)
            .await?;

        let mut result = Vec::new();
        for resource in &list.results {
            let vg: VersionGroupResponse = self
                .fetch(
                    &format!("{BASE_URL}/version-group/{}", resource.name),
                    "version-group",
                    &resource.name,
                    no_cache,
                )
                .await?;

            result.push(VersionGroupInfo {
                name: vg.name,
                versions: vg.versions.iter().map(|v| v.name.clone()).collect(),
                pokedexes: vg.pokedexes.iter().map(|p| p.name.clone()).collect(),
            });
        }

        Ok(result)
    }

    async fn get_game_pokemon(
        &self,
        version_group: &str,
        no_cache: bool,
        include_variants: bool,
    ) -> Result<Vec<Pokemon>, AppError> {
        // Check aggregated cache
        let cache_key = format!("{version_group}-variants-{include_variants}");
        if let Some(cached) = self
            .cache
            .get::<Vec<Pokemon>>("game-pokemon", &cache_key, no_cache)
            .await
        {
            return Ok(cached);
        }

        // Fetch version group -> pokedexes -> species -> pokemon
        let vg_url = format!("{BASE_URL}/version-group/{version_group}");
        let vg: VersionGroupResponse = self
            .fetch(&vg_url, "version-group", version_group, no_cache)
            .await?;

        // Collect all species across all pokedexes, deduplicating
        let mut species_entries: Vec<(String, u32)> = Vec::new();
        let mut seen_species: HashSet<String> = HashSet::new();

        for pokedex_ref in &vg.pokedexes {
            let pokedex_url = format!("{BASE_URL}/pokedex/{}", pokedex_ref.name);
            let pokedex: PokedexResponse = self
                .fetch(&pokedex_url, "pokedex", &pokedex_ref.name, no_cache)
                .await?;

            for entry in &pokedex.pokemon_entries {
                let species_name = entry.pokemon_species.name.clone();
                if seen_species.insert(species_name.clone()) {
                    species_entries.push((species_name, entry.entry_number));
                }
            }
        }

        debug!(
            "Fetching {} species for {version_group}",
            species_entries.len()
        );

        // Mass-fetch pokemon with concurrency limit
        let client = self;
        let results: Vec<Result<Vec<Pokemon>, AppError>> = stream::iter(species_entries)
            .map(|(species_name, pokedex_number)| async move {
                client
                    .fetch_species_pokemon(&species_name, pokedex_number, no_cache, include_variants)
                    .await
            })
            .buffer_unordered(CONCURRENT_REQUESTS)
            .collect()
            .await;

        let mut all_pokemon: Vec<Pokemon> = Vec::new();
        for result in results {
            match result {
                Ok(pokemon_list) => all_pokemon.extend(pokemon_list),
                Err(e) => warn!("Failed to fetch species: {e}"),
            }
        }

        // Sort by pokedex number for consistent ordering
        all_pokemon.sort_by_key(|p| (p.pokedex_number, !p.is_default_form as u8));

        // Cache the aggregated result
        if let Err(e) = self.cache.set("game-pokemon", &cache_key, &all_pokemon).await {
            warn!("Failed to cache game pokemon: {e}");
        }

        Ok(all_pokemon)
    }

    async fn get_pokemon(&self, name: &str, no_cache: bool) -> Result<Pokemon, AppError> {
        let url = format!("{BASE_URL}/pokemon/{name}");
        let resp: PokemonResponse = self.fetch(&url, "pokemon", name, no_cache).await?;

        // We need the species to get pokedex_number and is_default
        let species_url = format!("{BASE_URL}/pokemon-species/{}", resp.species.name);
        let species: PokemonSpeciesResponse = self
            .fetch(&species_url, "species", &resp.species.name, no_cache)
            .await?;

        let is_default = species
            .varieties
            .iter()
            .find(|v| v.pokemon.name == name)
            .map(|v| v.is_default)
            .unwrap_or(true);

        self.convert_pokemon_response(&resp, species.id, is_default)
    }

    async fn get_species_varieties(
        &self,
        species_name: &str,
        no_cache: bool,
    ) -> Result<Vec<Pokemon>, AppError> {
        let species_url = format!("{BASE_URL}/pokemon-species/{species_name}");
        let species: PokemonSpeciesResponse = self
            .fetch(&species_url, "species", species_name, no_cache)
            .await?;

        self.fetch_species_pokemon(species_name, species.id, no_cache, true)
            .await
    }

    async fn get_pokedex_pokemon(
        &self,
        pokedex_name: &str,
        no_cache: bool,
        include_variants: bool,
    ) -> Result<Vec<Pokemon>, AppError> {
        let cache_key = format!("{pokedex_name}-variants-{include_variants}");
        if let Some(cached) = self
            .cache
            .get::<Vec<Pokemon>>("pokedex-pokemon", &cache_key, no_cache)
            .await
        {
            return Ok(cached);
        }

        let pokedex_url = format!("{BASE_URL}/pokedex/{pokedex_name}");
        let pokedex: PokedexResponse = self
            .fetch(&pokedex_url, "pokedex", pokedex_name, no_cache)
            .await?;

        let species_entries: Vec<(String, u32)> = pokedex
            .pokemon_entries
            .iter()
            .map(|e| (e.pokemon_species.name.clone(), e.entry_number))
            .collect();

        debug!(
            "Fetching {} species for pokedex {pokedex_name}",
            species_entries.len()
        );

        let client = self;
        let results: Vec<Result<Vec<Pokemon>, AppError>> = stream::iter(species_entries)
            .map(|(species_name, pokedex_number)| async move {
                client
                    .fetch_species_pokemon(&species_name, pokedex_number, no_cache, include_variants)
                    .await
            })
            .buffer_unordered(CONCURRENT_REQUESTS)
            .collect()
            .await;

        let mut all_pokemon: Vec<Pokemon> = Vec::new();
        for result in results {
            match result {
                Ok(pokemon_list) => all_pokemon.extend(pokemon_list),
                Err(e) => warn!("Failed to fetch species: {e}"),
            }
        }

        all_pokemon.sort_by_key(|p| (p.pokedex_number, !p.is_default_form as u8));

        if let Err(e) = self
            .cache
            .set("pokedex-pokemon", &cache_key, &all_pokemon)
            .await
        {
            warn!("Failed to cache pokedex pokemon: {e}");
        }

        Ok(all_pokemon)
    }

    async fn get_type_chart(
        &self,
        no_cache: bool,
    ) -> Result<TypeEffectivenessData, AppError> {
        // Check aggregated cache
        if let Some(cached) = self
            .cache
            .get::<TypeEffectivenessData>("type-chart", "current", no_cache)
            .await
        {
            return Ok(cached);
        }

        let mut entries = Vec::new();

        for pokemon_type in PokemonType::ALL {
            let type_name = serde_json::to_value(pokemon_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();

            let url = format!("{BASE_URL}/type/{type_name}");
            let type_resp: TypeResponse = self
                .fetch(&url, "type", &type_name, no_cache)
                .await?;

            let relations = &type_resp.damage_relations;

            // double_damage_to = 2.0
            for target in &relations.double_damage_to {
                if let Some(defend_type) = Self::parse_pokemon_type(&target.name) {
                    entries.push(TypeEffectivenessEntry {
                        attack_type: pokemon_type,
                        defend_type,
                        multiplier: 2.0,
                    });
                }
            }

            // half_damage_to = 0.5
            for target in &relations.half_damage_to {
                if let Some(defend_type) = Self::parse_pokemon_type(&target.name) {
                    entries.push(TypeEffectivenessEntry {
                        attack_type: pokemon_type,
                        defend_type,
                        multiplier: 0.5,
                    });
                }
            }

            // no_damage_to = 0.0
            for target in &relations.no_damage_to {
                if let Some(defend_type) = Self::parse_pokemon_type(&target.name) {
                    entries.push(TypeEffectivenessEntry {
                        attack_type: pokemon_type,
                        defend_type,
                        multiplier: 0.0,
                    });
                }
            }
        }

        let data = TypeEffectivenessData { entries };

        if let Err(e) = self.cache.set("type-chart", "current", &data).await {
            warn!("Failed to cache type chart: {e}");
        }

        Ok(data)
    }
}
