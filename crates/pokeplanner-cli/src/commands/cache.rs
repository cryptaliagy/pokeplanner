use colored::Colorize;
use pokeplanner_pokeapi::{PokeApiClient, PokeApiClientConfig, PokeApiHttpClient};

use crate::display::format_bytes;
use crate::{CacheAction, ClearTarget, PopulateTarget};

/// Create a PokeApiHttpClient with lower concurrency for cache populate operations.
async fn make_populate_client(cache_dir: &std::path::Path) -> anyhow::Result<PokeApiHttpClient> {
    let config = PokeApiClientConfig {
        cache_path: cache_dir.to_path_buf(),
        base_url: "https://pokeapi.co/api/v2".to_string(),
        requests_per_second: 5,
        burst_size: 2,
        concurrent_requests: 3,
    };
    Ok(PokeApiHttpClient::with_config(config).await?)
}

pub async fn handle_cache_action(
    action: CacheAction,
    cache_dir: &std::path::Path,
) -> anyhow::Result<()> {
    match action {
        CacheAction::Stats => {
            let client = PokeApiHttpClient::new(cache_dir.to_path_buf()).await?;
            let stats = client.cache().stats().await;
            println!("{}", "Cache Statistics".bold());
            println!("  {} {}", "Location:".dimmed(), stats.base_path.display());
            println!(
                "  {} {} entries, {}",
                "Total:".dimmed(),
                stats.total_entries,
                format_bytes(stats.total_size_bytes),
            );
            println!();

            if stats.categories.is_empty() {
                println!("  {}", "(cache is empty)".dimmed());
            } else {
                println!(
                    "  {:<22} {:>8} {:>10}",
                    "Category".bold(),
                    "Entries".bold(),
                    "Size".bold(),
                );
                println!("  {}", "-".repeat(42).dimmed());
                for cat in &stats.categories {
                    println!(
                        "  {:<22} {:>8} {:>10}",
                        cat.name,
                        cat.entries,
                        format_bytes(cat.size_bytes),
                    );
                }
            }
        }
        CacheAction::Clear { target } => {
            let client = PokeApiHttpClient::new(cache_dir.to_path_buf()).await?;
            let cache = client.cache();

            match target {
                ClearTarget::All => {
                    let count = cache.clear_all().await?;
                    println!("Removed {} cached entries.", count);
                }
                ClearTarget::Stale => {
                    let count = cache.clear_stale().await?;
                    if count > 0 {
                        println!("Removed {} expired entries.", count);
                    } else {
                        println!("No expired entries found.");
                    }
                }
                ClearTarget::Game { name } => {
                    let mut count = 0u64;
                    for variants in [true, false] {
                        let key = format!("{name}-variants-{variants}");
                        if cache.remove("game-pokemon", &key).await? {
                            count += 1;
                        }
                    }
                    if count > 0 {
                        println!("Cleared game pokemon cache for '{name}'.");
                    } else {
                        println!("No cached data found for game '{name}'.");
                    }
                }
                ClearTarget::Pokedex { name } => {
                    let mut count = 0u64;
                    for variants in [true, false] {
                        let key = format!("{name}-variants-{variants}");
                        if cache.remove("pokedex-pokemon", &key).await? {
                            count += 1;
                        }
                    }
                    if count > 0 {
                        println!("Cleared pokedex pokemon cache for '{name}'.");
                    } else {
                        println!("No cached data found for pokedex '{name}'.");
                    }
                }
                ClearTarget::Pokemon { name } => {
                    let removed_pokemon = cache.remove("pokemon", &name).await?;
                    let removed_species = cache.remove("species", &name).await?;
                    if removed_pokemon || removed_species {
                        println!("Cleared cached data for pokemon '{name}'.");
                    } else {
                        println!("No cached data found for pokemon '{name}'.");
                    }
                }
                ClearTarget::TypeChart => {
                    let mut count = 0u64;
                    if cache.remove("type-chart", "current").await? {
                        count += 1;
                    }
                    count += cache.clear_category("type").await?;
                    if count > 0 {
                        println!("Cleared type chart cache ({count} entries).");
                    } else {
                        println!("No type chart data cached.");
                    }
                }
                ClearTarget::Learnset { name } => {
                    if let Some(pokemon_name) = name {
                        if cache.remove("pokemon-full", &pokemon_name).await? {
                            println!("Cleared learnset cache for '{pokemon_name}'.");
                        } else {
                            println!("No learnset data cached for '{pokemon_name}'.");
                        }
                    } else {
                        let count = cache.clear_category("pokemon-full").await?;
                        if count > 0 {
                            println!("Cleared all learnset cache ({count} entries).");
                        } else {
                            println!("No learnset data cached.");
                        }
                    }
                }
                ClearTarget::Moves { name } => {
                    if let Some(move_name) = name {
                        if cache.remove("move", &move_name).await? {
                            println!("Cleared cache for move '{move_name}'.");
                        } else {
                            println!("No cached data for move '{move_name}'.");
                        }
                    } else {
                        let count = cache.clear_category("move").await?;
                        if count > 0 {
                            println!("Cleared all move cache ({count} entries).");
                        } else {
                            println!("No move data cached.");
                        }
                    }
                }
            }
        }
        CacheAction::Populate { target } => {
            let client = make_populate_client(cache_dir).await?;

            match target {
                PopulateTarget::Games => {
                    populate_games(&client).await?;
                }
                PopulateTarget::TypeChart => {
                    populate_type_chart(&client).await?;
                }
                PopulateTarget::Game {
                    name,
                    include_variants,
                } => {
                    populate_games(&client).await?;
                    populate_game_pokemon(&client, &name, include_variants).await?;
                }
                PopulateTarget::Pokedex {
                    name,
                    include_variants,
                } => {
                    populate_pokedex_pokemon(&client, &name, include_variants).await?;
                }
                PopulateTarget::All { include_variants } => {
                    let groups = populate_games(&client).await?;
                    populate_type_chart(&client).await?;

                    println!();
                    println!(
                        "{} Populating pokemon for {} games...",
                        "==>".bold(),
                        groups.len(),
                    );
                    for (i, group) in groups.iter().enumerate() {
                        println!();
                        println!(
                            "{} [{}/{}] {}",
                            "==>".bold(),
                            i + 1,
                            groups.len(),
                            group.name.bold(),
                        );
                        populate_game_pokemon(&client, &group.name, include_variants).await?;
                    }

                    println!();
                    println!("{}", "Cache population complete!".green().bold());
                }
            }
        }
    }
    Ok(())
}

async fn populate_games(
    client: &PokeApiHttpClient,
) -> anyhow::Result<Vec<pokeplanner_pokeapi::VersionGroupInfo>> {
    eprint!("  Fetching version groups... ");
    let groups = client.get_version_groups(false).await?;
    eprintln!("{} ({} games)", "done".green(), groups.len());
    for group in &groups {
        eprintln!(
            "    {} {}",
            group.name,
            format!("({})", group.versions.join(", ")).dimmed(),
        );
    }
    Ok(groups)
}

async fn populate_type_chart(client: &PokeApiHttpClient) -> anyhow::Result<()> {
    eprintln!("  Fetching type chart (18 types)...");
    client.get_type_chart(false).await?;
    eprintln!("  Type chart: {}", "done".green());
    Ok(())
}

async fn populate_game_pokemon(
    client: &PokeApiHttpClient,
    game: &str,
    include_variants: bool,
) -> anyhow::Result<()> {
    let variant_label = if include_variants {
        " (with variants)"
    } else {
        ""
    };
    eprint!("  Fetching pokemon for '{game}'{variant_label}... ");

    let pokemon = client
        .get_game_pokemon(game, false, include_variants)
        .await?;
    eprintln!("{} ({} pokemon)", "done".green(), pokemon.len(),);
    Ok(())
}

async fn populate_pokedex_pokemon(
    client: &PokeApiHttpClient,
    pokedex: &str,
    include_variants: bool,
) -> anyhow::Result<()> {
    let variant_label = if include_variants {
        " (with variants)"
    } else {
        ""
    };
    eprint!("  Fetching pokemon for pokedex '{pokedex}'{variant_label}... ");

    let pokemon = client
        .get_pokedex_pokemon(pokedex, false, include_variants)
        .await?;
    eprintln!("{} ({} pokemon)", "done".green(), pokemon.len(),);
    Ok(())
}
