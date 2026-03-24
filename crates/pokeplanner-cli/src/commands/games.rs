use colored::Colorize;
use pokeplanner_core::PokemonQueryParams;
use pokeplanner_service::PokePlannerService;

use crate::display::print_pokemon_list;
use crate::unusable::UnusableStore;

pub async fn handle_list_games<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    service: &PokePlannerService<S, P>,
    no_cache: bool,
) -> anyhow::Result<()> {
    let groups = service.list_version_groups(no_cache).await?;
    println!("{}", format!("{} games available:", groups.len()).bold());
    for group in &groups {
        println!(
            "  {:<25} {}",
            group.name.bold(),
            group.versions.join(", ").dimmed(),
        );
    }
    Ok(())
}

pub async fn handle_game_pokemon<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    service: &PokePlannerService<S, P>,
    unusable: &UnusableStore,
    game: &str,
    params: &PokemonQueryParams,
) -> anyhow::Result<()> {
    let pokemon = service.get_game_pokemon(game, params).await?;
    print_pokemon_list(&pokemon, unusable);
    Ok(())
}

pub async fn handle_pokedex_pokemon<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    service: &PokePlannerService<S, P>,
    unusable: &UnusableStore,
    pokedex: &str,
    params: &PokemonQueryParams,
) -> anyhow::Result<()> {
    let pokemon = service.get_pokedex_pokemon(pokedex, params).await?;
    print_pokemon_list(&pokemon, unusable);
    Ok(())
}
