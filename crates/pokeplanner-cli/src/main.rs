mod unusable;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use pokeplanner_core::{
    PokemonQueryParams, PokemonType, SortField, SortOrder, TeamPlanRequest, TeamSource,
};
use pokeplanner_pokeapi::{PokeApiClientConfig, PokeApiHttpClient};
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tracing_subscriber::EnvFilter;
use unusable::UnusableStore;

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pokeplanner"))
        .unwrap_or_else(|| PathBuf::from(".pokeplanner"))
}

#[derive(Parser)]
#[command(name = "pokeplanner", about = "PokePlanner CLI — build optimal Pokemon teams", version)]
struct Cli {
    /// Directory for cached PokeAPI data
    #[arg(long, global = true, default_value_os_t = default_data_dir().join("cache"))]
    cache_dir: PathBuf,

    /// Directory for job storage data
    #[arg(long, global = true, default_value_os_t = default_data_dir().join("jobs"))]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available games (version groups)
    ListGames {
        #[arg(long)]
        no_cache: bool,
    },
    /// List pokemon available in a game
    GamePokemon {
        /// Version group name (e.g., "scarlet-violet", "red-blue")
        game: String,
        #[arg(long)]
        min_bst: Option<u32>,
        #[arg(long, value_enum)]
        sort_by: Option<CliSortField>,
        #[arg(long, value_enum, default_value = "asc")]
        sort_order: CliSortOrder,
        #[arg(long)]
        no_cache: bool,
        /// Exclude alternate forms (megas, regional variants, etc.)
        #[arg(long)]
        exclude_variants: bool,
        /// Limit number of results (useful with --sort-by for "top N")
        #[arg(long)]
        limit: Option<usize>,
    },
    /// List pokemon from a specific pokedex (e.g., "national" for all pokemon)
    PokedexPokemon {
        /// Pokedex name (e.g., "national", "kanto", "paldea")
        pokedex: String,
        #[arg(long)]
        min_bst: Option<u32>,
        #[arg(long, value_enum)]
        sort_by: Option<CliSortField>,
        #[arg(long, value_enum, default_value = "asc")]
        sort_order: CliSortOrder,
        #[arg(long)]
        no_cache: bool,
        /// Exclude alternate forms (megas, regional variants, etc.)
        #[arg(long)]
        exclude_variants: bool,
        /// Limit number of results (useful with --sort-by for "top N")
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Look up or search for pokemon
    Pokemon {
        #[command(subcommand)]
        action: PokemonAction,
    },
    /// Plan an optimal team
    PlanTeam {
        /// Plan from game pokedex(es), comma-separated (e.g., "red-blue" or "red-blue,gold-silver")
        #[arg(long, group = "source", value_delimiter = ',')]
        game: Option<Vec<String>>,
        /// Plan from a specific pokedex (e.g., "national" for global dex)
        #[arg(long, group = "source")]
        pokedex: Option<String>,
        /// Plan from a custom list of pokemon (comma-separated)
        #[arg(long, group = "source", value_delimiter = ',')]
        pokemon: Option<Vec<String>>,
        #[arg(long)]
        min_bst: Option<u32>,
        #[arg(long, default_value = "5")]
        top_k: usize,
        #[arg(long)]
        no_cache: bool,
        /// Exclude alternate forms (megas, regional variants, etc.)
        #[arg(long)]
        exclude_variants: bool,
        /// Exclude variants by type keyword (e.g., "mega", "gmax", "alola", "galar", "hisui", "totem")
        #[arg(long, value_delimiter = ',')]
        exclude_variant_type: Option<Vec<String>>,
        /// Exclude specific pokemon by form name (comma-separated)
        #[arg(long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
        /// Exclude all forms of a species (comma-separated, e.g., "charizard" removes base + megas)
        #[arg(long, value_delimiter = ',')]
        exclude_species: Option<Vec<String>>,
        /// Enemy pokemon to counter (comma-separated). Optimizes team against this specific team.
        #[arg(long, value_delimiter = ',')]
        counter: Option<Vec<String>>,
    },
    /// Analyze type coverage for a team
    AnalyzeTeam {
        /// Pokemon names (comma-separated)
        #[arg(value_delimiter = ',')]
        pokemon: Vec<String>,
        #[arg(long)]
        no_cache: bool,
    },
    /// Manage the PokeAPI disk cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// Manage unusable pokemon (excluded from team planning)
    Unusable {
        #[command(subcommand)]
        action: UnusableAction,
    },
}

#[derive(Subcommand)]
enum UnusableAction {
    /// Mark pokemon as unusable (comma-separated form names)
    Add {
        /// Pokemon form names (e.g., "charizard-mega-x", "mewtwo-mega-y")
        #[arg(value_delimiter = ',')]
        names: Vec<String>,
    },
    /// Unmark pokemon as unusable (comma-separated form names)
    Remove {
        /// Pokemon form names to unmark
        #[arg(value_delimiter = ',')]
        names: Vec<String>,
    },
    /// List all pokemon marked as unusable
    List,
    /// Clear the entire unusable list
    Clear,
}

#[derive(Subcommand)]
enum PokemonAction {
    /// Get details for a specific pokemon
    Show {
        /// Pokemon name (e.g., "pikachu", "charizard-mega-x")
        name: String,
        #[arg(long)]
        no_cache: bool,
    },
    /// Search for pokemon matching criteria
    Search {
        /// Search within a game (version group name, e.g., "red-blue")
        #[arg(long, value_delimiter = ',')]
        game: Option<Vec<String>>,
        /// Search within a pokedex (e.g., "national", "kanto")
        #[arg(long)]
        pokedex: Option<String>,

        /// Filter by name (substring match on form or species name)
        #[arg(long)]
        name: Option<String>,

        /// Filter by type (comma-separated, e.g., "fire", "fire,dragon")
        #[arg(long, value_delimiter = ',')]
        r#type: Option<Vec<String>>,
        /// Exclude pokemon with these types (comma-separated, e.g., "poison,fairy")
        #[arg(long, value_delimiter = ',')]
        not_type: Option<Vec<String>>,
        /// Only show single-type pokemon
        #[arg(long)]
        mono_type: bool,
        /// Only show dual-type pokemon
        #[arg(long)]
        dual_type: bool,

        /// Filter by BST (e.g., "ge500", "lt400", "eq600")
        #[arg(long)]
        bst: Option<String>,
        /// Filter by HP stat (e.g., "ge100", "lt50")
        #[arg(long)]
        hp: Option<String>,
        /// Filter by Attack stat
        #[arg(long)]
        attack: Option<String>,
        /// Filter by Defense stat
        #[arg(long)]
        defense: Option<String>,
        /// Filter by Special Attack stat
        #[arg(long)]
        special_attack: Option<String>,
        /// Filter by Special Defense stat
        #[arg(long)]
        special_defense: Option<String>,
        /// Filter by Speed stat
        #[arg(long)]
        speed: Option<String>,

        /// Only show default (base) forms
        #[arg(long)]
        default_only: bool,
        /// Only show variant (non-default) forms
        #[arg(long)]
        variants_only: bool,
        /// Only show specific variant types (e.g., "mega", "alola", "gmax")
        #[arg(long, value_delimiter = ',')]
        variant_type: Option<Vec<String>>,

        /// Sort results by field
        #[arg(long, value_enum)]
        sort_by: Option<CliSortField>,
        /// Sort order
        #[arg(long, value_enum, default_value = "asc")]
        sort_order: CliSortOrder,
        /// Limit number of results
        #[arg(long)]
        limit: Option<usize>,

        #[arg(long)]
        no_cache: bool,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Show cache statistics (entry counts, sizes, location)
    Stats,
    /// Pre-fetch and cache data from PokeAPI (uses lower concurrency)
    Populate {
        #[command(subcommand)]
        target: PopulateTarget,
    },
    /// Remove cached data
    Clear {
        #[command(subcommand)]
        target: ClearTarget,
    },
}

#[derive(Subcommand)]
enum PopulateTarget {
    /// Fetch all version group metadata
    Games,
    /// Fetch the type effectiveness chart
    TypeChart,
    /// Fetch all pokemon for a game (version group)
    Game {
        /// Version group name (e.g., "red-blue", "scarlet-violet")
        name: String,
        /// Include alternate forms (megas, regional variants, etc.)
        #[arg(long)]
        include_variants: bool,
    },
    /// Fetch all pokemon from a pokedex
    Pokedex {
        /// Pokedex name (e.g., "national", "kanto")
        name: String,
        /// Include alternate forms (megas, regional variants, etc.)
        #[arg(long)]
        include_variants: bool,
    },
    /// Fetch everything: all games, their pokemon, and the type chart
    All {
        /// Include alternate forms (megas, regional variants, etc.)
        #[arg(long)]
        include_variants: bool,
    },
}

#[derive(Subcommand)]
enum ClearTarget {
    /// Remove all cached data
    All,
    /// Remove only expired (stale) cache entries
    Stale,
    /// Remove cached data for a specific game
    Game {
        /// Version group name
        name: String,
    },
    /// Remove cached data for a specific pokedex
    Pokedex {
        /// Pokedex name
        name: String,
    },
    /// Remove cached data for a specific pokemon
    Pokemon {
        /// Pokemon form name (e.g., "pikachu", "charizard-mega-x")
        name: String,
    },
    /// Remove the cached type chart
    TypeChart,
}

#[derive(Clone, ValueEnum)]
enum CliSortField {
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

impl From<CliSortField> for SortField {
    fn from(f: CliSortField) -> Self {
        match f {
            CliSortField::Bst => SortField::Bst,
            CliSortField::Hp => SortField::Hp,
            CliSortField::Attack => SortField::Attack,
            CliSortField::Defense => SortField::Defense,
            CliSortField::SpecialAttack => SortField::SpecialAttack,
            CliSortField::SpecialDefense => SortField::SpecialDefense,
            CliSortField::Speed => SortField::Speed,
            CliSortField::Name => SortField::Name,
            CliSortField::PokedexNumber => SortField::PokedexNumber,
        }
    }
}

#[derive(Clone, ValueEnum)]
enum CliSortOrder {
    Asc,
    Desc,
}

impl From<CliSortOrder> for SortOrder {
    fn from(o: CliSortOrder) -> Self {
        match o {
            CliSortOrder::Asc => SortOrder::Asc,
            CliSortOrder::Desc => SortOrder::Desc,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let cache_dir = cli.cache_dir;
    let base_dir = default_data_dir();

    let storage = Arc::new(JsonFileStorage::new(cli.data_dir).await?);
    let pokeapi = Arc::new(PokeApiHttpClient::new(cache_dir.clone()).await?);
    let service = PokePlannerService::new(storage, pokeapi);
    let mut unusable = UnusableStore::load(&base_dir).await?;

    match cli.command {
        Commands::ListGames { no_cache } => {
            let groups = service.list_version_groups(no_cache).await?;
            println!("{}", format!("{} games available:", groups.len()).bold());
            for group in &groups {
                println!(
                    "  {:<25} {}",
                    group.name.bold(),
                    group.versions.join(", ").dimmed(),
                );
            }
        }
        Commands::GamePokemon {
            game,
            min_bst,
            sort_by,
            sort_order,
            no_cache,
            exclude_variants,
            limit,
        } => {
            let pokemon = service
                .get_game_pokemon(
                    &game,
                    &PokemonQueryParams {
                        min_bst,
                        no_cache,
                        sort_by: sort_by.map(SortField::from),
                        sort_order: sort_order.into(),
                        include_variants: !exclude_variants,
                        limit,
                    },
                )
                .await?;
            print_pokemon_list(&pokemon, &unusable);
        }
        Commands::PokedexPokemon {
            pokedex,
            min_bst,
            sort_by,
            sort_order,
            no_cache,
            exclude_variants,
            limit,
        } => {
            let pokemon = service
                .get_pokedex_pokemon(
                    &pokedex,
                    &PokemonQueryParams {
                        min_bst,
                        no_cache,
                        sort_by: sort_by.map(SortField::from),
                        sort_order: sort_order.into(),
                        include_variants: !exclude_variants,
                        limit,
                    },
                )
                .await?;
            print_pokemon_list(&pokemon, &unusable);
        }
        Commands::Pokemon { action } => {
            handle_pokemon_action(action, &service, &unusable).await?;
        }
        Commands::PlanTeam {
            game,
            pokedex,
            pokemon,
            min_bst,
            top_k,
            no_cache,
            exclude_variants,
            exclude_variant_type,
            exclude,
            exclude_species,
            counter,
        } => {
            let source = if let Some(games) = game {
                TeamSource::Game {
                    version_groups: games,
                }
            } else if let Some(pokedex_name) = pokedex {
                TeamSource::Pokedex { pokedex_name }
            } else if let Some(names) = pokemon {
                TeamSource::Custom {
                    pokemon_names: names,
                }
            } else {
                anyhow::bail!("Specify either --game, --pokedex, or --pokemon");
            };

            // Merge the persistent unusable list into the exclude list
            let mut all_exclude = exclude.unwrap_or_default();
            all_exclude.extend(unusable.to_exclude_list());

            let request = TeamPlanRequest {
                source,
                min_bst,
                no_cache,
                top_k: Some(top_k),
                include_variants: !exclude_variants,
                exclude: all_exclude,
                exclude_species: exclude_species.unwrap_or_default(),
                exclude_variant_types: exclude_variant_type.unwrap_or_default(),
                counter_team: counter,
            };

            let job_id = service.submit_team_plan(request).await?;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let job = service.get_job(&job_id).await?;

                if let Some(progress) = &job.progress {
                    eprint!(
                        "\r  {} ({}/{})",
                        progress.phase, progress.completed_steps, progress.total_steps
                    );
                }

                match job.status {
                    pokeplanner_core::JobStatus::Completed => {
                        eprintln!();
                        if let Some(result) = &job.result {
                            println!("{}", result.message.dimmed());
                            if let Some(data) = &result.data {
                                let plans: Vec<pokeplanner_core::TeamPlan> =
                                    serde_json::from_value(data.clone()).unwrap_or_default();
                                print_team_plans(&plans);
                            }
                        }
                        break;
                    }
                    pokeplanner_core::JobStatus::Failed => {
                        eprintln!();
                        if let Some(result) = &job.result {
                            eprintln!("{} {}", "Error:".red().bold(), result.message);
                        }
                        break;
                    }
                    _ => continue,
                }
            }
        }
        Commands::AnalyzeTeam { pokemon, no_cache } => {
            let coverage = service.analyze_team(pokemon, no_cache).await?;
            println!("{}", serde_json::to_string_pretty(&coverage)?);
        }
        Commands::Cache { action } => {
            handle_cache_action(action, &cache_dir).await?;
        }
        Commands::Unusable { action } => {
            handle_unusable_action(action, &mut unusable).await?;
        }
    }

    Ok(())
}

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

/// Parse a stat filter string like "ge500", "lt100", "eq120".
/// Returns a closure that tests a u32 value against the filter.
fn parse_stat_filter(s: &str) -> anyhow::Result<Box<dyn Fn(u32) -> bool>> {
    let s = s.trim();
    let (op, val_str) = if s.starts_with("ge") {
        ("ge", &s[2..])
    } else if s.starts_with("gt") {
        ("gt", &s[2..])
    } else if s.starts_with("le") {
        ("le", &s[2..])
    } else if s.starts_with("lt") {
        ("lt", &s[2..])
    } else if s.starts_with("eq") {
        ("eq", &s[2..])
    } else {
        // Default: treat bare number as "ge"
        ("ge", s)
    };

    let val: u32 = val_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid stat filter value: '{val_str}' (expected a number)"))?;

    Ok(match op {
        "ge" => Box::new(move |v| v >= val),
        "gt" => Box::new(move |v| v > val),
        "le" => Box::new(move |v| v <= val),
        "lt" => Box::new(move |v| v < val),
        "eq" => Box::new(move |v| v == val),
        _ => unreachable!(),
    })
}

async fn handle_pokemon_action<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    action: PokemonAction,
    service: &PokePlannerService<S, P>,
    unusable: &UnusableStore,
) -> anyhow::Result<()> {
    match action {
        PokemonAction::Show { name, no_cache } => {
            let pokemon = service.get_pokemon(&name, no_cache).await?;
            print_pokemon_detail(&pokemon, unusable);

            // Show other forms/varieties of this species
            let varieties = service
                .get_species_varieties(&pokemon.species_name, no_cache)
                .await?;
            let others: Vec<&pokeplanner_core::Pokemon> = varieties
                .iter()
                .filter(|v| v.form_name != pokemon.form_name)
                .collect();
            if !others.is_empty() {
                println!(
                    "  {} {}",
                    "Other forms:".bold(),
                    format!("({} total)", varieties.len()).dimmed(),
                );
                for v in &others {
                    print_pokemon_detail(v, unusable);
                }
            }
        }
        PokemonAction::Search {
            game,
            pokedex,
            name,
            r#type,
            not_type,
            mono_type,
            dual_type,
            bst,
            hp,
            attack,
            defense,
            special_attack,
            special_defense,
            speed,
            default_only,
            variants_only,
            variant_type,
            sort_by,
            sort_order,
            limit,
            no_cache,
        } => {
            // Step 1: Fetch candidate pokemon from source
            // Include all variants; we'll filter later
            let mut candidates = if let Some(games) = game {
                let mut all = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for g in &games {
                    let pokemon = service
                        .get_game_pokemon(
                            g,
                            &PokemonQueryParams {
                                no_cache,
                                include_variants: true,
                                ..Default::default()
                            },
                        )
                        .await?;
                    for p in pokemon {
                        if seen.insert(p.form_name.clone()) {
                            all.push(p);
                        }
                    }
                }
                all
            } else if let Some(ref dex) = pokedex {
                service
                    .get_pokedex_pokemon(
                        dex,
                        &PokemonQueryParams {
                            no_cache,
                            include_variants: true,
                            ..Default::default()
                        },
                    )
                    .await?
            } else {
                // Default: national pokedex
                service
                    .get_pokedex_pokemon(
                        "national",
                        &PokemonQueryParams {
                            no_cache,
                            include_variants: true,
                            ..Default::default()
                        },
                    )
                    .await?
            };

            // Step 2: Apply filters

            // Name substring filter
            if let Some(ref pattern) = name {
                let pattern_lower = pattern.to_lowercase();
                candidates.retain(|p| {
                    p.form_name.to_lowercase().contains(&pattern_lower)
                        || p.species_name.to_lowercase().contains(&pattern_lower)
                });
            }

            // Type inclusion filter: must have at least one of the specified types
            if let Some(ref type_names) = r#type {
                let types: Vec<PokemonType> = type_names
                    .iter()
                    .filter_map(|t| serde_json::from_value(serde_json::Value::String(t.to_lowercase())).ok())
                    .collect();
                if !types.is_empty() {
                    candidates.retain(|p| p.types.iter().any(|pt| types.contains(pt)));
                }
            }

            // Type exclusion filter: must NOT have any of the specified types
            if let Some(ref type_names) = not_type {
                let types: Vec<PokemonType> = type_names
                    .iter()
                    .filter_map(|t| serde_json::from_value(serde_json::Value::String(t.to_lowercase())).ok())
                    .collect();
                if !types.is_empty() {
                    candidates.retain(|p| !p.types.iter().any(|pt| types.contains(pt)));
                }
            }

            // Mono-type / dual-type
            if mono_type {
                candidates.retain(|p| p.types.len() == 1);
            }
            if dual_type {
                candidates.retain(|p| p.types.len() >= 2);
            }

            // Form filters
            if default_only {
                candidates.retain(|p| p.is_default_form);
            }
            if variants_only {
                candidates.retain(|p| !p.is_default_form);
            }
            if let Some(ref vt_keywords) = variant_type {
                candidates.retain(|p| {
                    if p.is_default_form {
                        return false;
                    }
                    let suffix = p
                        .form_name
                        .strip_prefix(&p.species_name)
                        .unwrap_or("")
                        .to_lowercase();
                    vt_keywords
                        .iter()
                        .any(|kw| suffix.contains(&kw.to_lowercase()))
                });
            }

            // Stat filters
            if let Some(ref f) = bst {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.bst()));
            }
            if let Some(ref f) = hp {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.hp));
            }
            if let Some(ref f) = attack {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.attack));
            }
            if let Some(ref f) = defense {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.defense));
            }
            if let Some(ref f) = special_attack {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.special_attack));
            }
            if let Some(ref f) = special_defense {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.special_defense));
            }
            if let Some(ref f) = speed {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.speed));
            }

            // Step 3: Sort and limit
            if let Some(field) = sort_by {
                let core_field: SortField = field.into();
                let core_order: SortOrder = sort_order.into();
                candidates.sort_by(|a, b| {
                    let cmp = match core_field {
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
                    match core_order {
                        SortOrder::Asc => cmp,
                        SortOrder::Desc => cmp.reverse(),
                    }
                });
            }
            if let Some(n) = limit {
                candidates.truncate(n);
            }

            // Step 4: Display
            print_pokemon_list(&candidates, unusable);
        }
    }
    Ok(())
}

async fn handle_cache_action(action: CacheAction, cache_dir: &std::path::Path) -> anyhow::Result<()> {
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
                    // Clear both variant combos of the aggregated cache
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
                    // Also clear individual type entries
                    count += cache.clear_category("type").await?;
                    if count > 0 {
                        println!("Cleared type chart cache ({count} entries).");
                    } else {
                        println!("No type chart data cached.");
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
                PopulateTarget::Game { name, include_variants } => {
                    populate_games(&client).await?;
                    populate_game_pokemon(&client, &name, include_variants).await?;
                }
                PopulateTarget::Pokedex { name, include_variants } => {
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

use pokeplanner_pokeapi::PokeApiClient;

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
    // The get_type_chart method fetches all 18 types. Since we want progress,
    // we call it — individual type fetches are cached along the way.
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

    let pokemon = client.get_game_pokemon(game, false, include_variants).await?;
    eprintln!(
        "{} ({} pokemon)",
        "done".green(),
        pokemon.len(),
    );
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
    eprintln!(
        "{} ({} pokemon)",
        "done".green(),
        pokemon.len(),
    );
    Ok(())
}

async fn handle_unusable_action(
    action: UnusableAction,
    store: &mut UnusableStore,
) -> anyhow::Result<()> {
    match action {
        UnusableAction::Add { names } => {
            if names.is_empty() {
                anyhow::bail!("Provide at least one pokemon form name");
            }
            let added = store.add(&names).await?;
            if added.is_empty() {
                println!("All specified pokemon were already marked unusable.");
            } else {
                for name in &added {
                    println!("  {} {}", "+".green(), name);
                }
                println!("Marked {} pokemon as unusable.", added.len());
            }
        }
        UnusableAction::Remove { names } => {
            if names.is_empty() {
                anyhow::bail!("Provide at least one pokemon form name");
            }
            let removed = store.remove(&names).await?;
            if removed.is_empty() {
                println!("None of the specified pokemon were in the unusable list.");
            } else {
                for name in &removed {
                    println!("  {} {}", "-".red(), name);
                }
                println!("Removed {} pokemon from unusable list.", removed.len());
            }
        }
        UnusableAction::List => {
            let list = store.list();
            if list.is_empty() {
                println!("No pokemon marked as unusable.");
            } else {
                println!(
                    "{}",
                    format!("{} pokemon marked as unusable:", list.len()).bold()
                );
                for name in &list {
                    println!("  {}", name);
                }
            }
        }
        UnusableAction::Clear => {
            let count = store.clear().await?;
            if count > 0 {
                println!("Cleared {} pokemon from unusable list.", count);
            } else {
                println!("Unusable list was already empty.");
            }
        }
    }
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn type_name(t: &PokemonType) -> &'static str {
    match t {
        PokemonType::Normal => "normal",
        PokemonType::Fire => "fire",
        PokemonType::Water => "water",
        PokemonType::Electric => "electric",
        PokemonType::Grass => "grass",
        PokemonType::Ice => "ice",
        PokemonType::Fighting => "fighting",
        PokemonType::Poison => "poison",
        PokemonType::Ground => "ground",
        PokemonType::Flying => "flying",
        PokemonType::Psychic => "psychic",
        PokemonType::Bug => "bug",
        PokemonType::Rock => "rock",
        PokemonType::Ghost => "ghost",
        PokemonType::Dragon => "dragon",
        PokemonType::Dark => "dark",
        PokemonType::Steel => "steel",
        PokemonType::Fairy => "fairy",
    }
}

fn color_type(t: &PokemonType) -> colored::ColoredString {
    let name = format!("{t}");
    match t {
        PokemonType::Fire => name.red(),
        PokemonType::Water => name.blue(),
        PokemonType::Grass => name.green(),
        PokemonType::Electric => name.yellow(),
        PokemonType::Ice => name.cyan(),
        PokemonType::Fighting => name.truecolor(194, 46, 27),
        PokemonType::Poison => name.purple(),
        PokemonType::Ground => name.truecolor(226, 191, 101),
        PokemonType::Flying => name.truecolor(169, 143, 243),
        PokemonType::Psychic => name.truecolor(249, 85, 135),
        PokemonType::Bug => name.truecolor(166, 185, 26),
        PokemonType::Rock => name.truecolor(182, 161, 54),
        PokemonType::Ghost => name.truecolor(115, 87, 151),
        PokemonType::Dragon => name.truecolor(111, 53, 252),
        PokemonType::Dark => name.truecolor(112, 87, 70),
        PokemonType::Steel => name.truecolor(183, 183, 206),
        PokemonType::Fairy => name.truecolor(214, 133, 173),
        PokemonType::Normal => name.white(),
    }
}

fn colored_type_list(types: &[PokemonType]) -> String {
    types
        .iter()
        .map(|t| format!("{}", color_type(t)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a stat bar: filled portion + dimmed remainder, fixed width.
fn stat_bar(value: u32, max: u32, width: usize) -> String {
    let filled = ((value as f64 / max as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;

    let bar_color = if value >= 130 {
        "green"
    } else if value >= 80 {
        "yellow"
    } else {
        "red"
    };

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);

    let colored_filled = match bar_color {
        "green" => filled_str.green(),
        "yellow" => filled_str.yellow(),
        _ => filled_str.red(),
    };

    format!("{colored_filled}{}", empty_str.dimmed())
}

fn print_pokemon_list(pokemon: &[pokeplanner_core::Pokemon], unusable: &UnusableStore) {
    println!("{}", format!("{} pokemon found:", pokemon.len()).bold());
    println!();
    for p in pokemon {
        let types_display: Vec<String> =
            p.types.iter().map(|t| format!("{}", color_type(t))).collect();
        let mut markers = String::new();
        if !p.is_default_form {
            markers.push_str(&" *".dimmed().to_string());
        }
        if unusable.is_unusable(&p.form_name) {
            markers.push_str(&" [unusable]".red().to_string());
        }
        println!(
            "  #{:>4} {:<25} {:<20} BST: {}{}",
            p.pokedex_number.to_string().dimmed(),
            p.form_name,
            types_display.join("/"),
            p.bst().to_string().bold(),
            markers,
        );
    }
}

fn print_pokemon_detail(p: &pokeplanner_core::Pokemon, unusable: &UnusableStore) {
    let types_display: Vec<String> =
        p.types.iter().map(|t| format!("{}", color_type(t))).collect();
    let mut tags = String::new();
    if !p.is_default_form {
        tags.push_str(&" (variant)".dimmed().to_string());
    }
    if unusable.is_unusable(&p.form_name) {
        tags.push_str(&" (unusable — excluded from team planning)".red().to_string());
    }

    println!();
    println!("  {} {}", p.form_name.bold(), tags);
    println!(
        "  #{} {}",
        p.pokedex_number,
        types_display.join(" / ")
    );
    println!();

    // Stats with bars (max single stat is 255 for bar scaling)
    let max = 255;
    let bar_w = 20;
    let stats = [
        ("HP ", p.stats.hp),
        ("Atk", p.stats.attack),
        ("Def", p.stats.defense),
        ("SpA", p.stats.special_attack),
        ("SpD", p.stats.special_defense),
        ("Spe", p.stats.speed),
    ];

    for (label, val) in &stats {
        println!(
            "  {} {:>3}  {}",
            label.dimmed(),
            val.to_string().bold(),
            stat_bar(*val, max, bar_w),
        );
    }
    println!(
        "  {} {}",
        "BST".dimmed(),
        p.bst().to_string().bold(),
    );
    println!();
}

fn print_team_plans(plans: &[pokeplanner_core::TeamPlan]) {
    if plans.is_empty() {
        println!("No team plans generated.");
        return;
    }

    for (i, plan) in plans.iter().enumerate() {
        let rank = i + 1;
        println!();
        println!(
            "{}",
            format!(
                "=== Team #{rank} (score: {:.3}, BST: {}) ===",
                plan.score, plan.total_bst
            )
            .bold()
        );
        println!();

        // Header — pad plain text first, then apply bold
        println!(
            "  {}  {}  {}  {}  {}  {}  {}  {}  {}",
            format!("{:<22}", "Pokemon").bold(),
            format!("{:<18}", "Types").bold(),
            format!("{:>5}", "BST").bold(),
            format!("{:>3}", "HP").bold(),
            format!("{:>3}", "Atk").bold(),
            format!("{:>3}", "Def").bold(),
            format!("{:>3}", "SpA").bold(),
            format!("{:>3}", "SpD").bold(),
            format!("{:>3}", "Spe").bold(),
        );
        println!("  {}", "-".repeat(78).dimmed());

        for member in &plan.team {
            let p = &member.pokemon;

            // Build the types string: pad the plain text width, then colorize
            let types_plain: Vec<&str> = p.types.iter().map(|t| type_name(t)).collect();
            let types_plain_joined = types_plain.join("/");
            let types_colored: Vec<String> =
                p.types.iter().map(|t| format!("{}", color_type(t))).collect();
            let types_colored_joined = types_colored.join("/");
            // Compute how many pad chars we need to reach column width 18
            let pad_needed = 18usize.saturating_sub(types_plain_joined.len());
            let types_padded = format!("{types_colored_joined}{}", " ".repeat(pad_needed));

            let name_display = if p.is_default_form {
                p.form_name.clone()
            } else {
                format!("{} *", p.form_name)
            };

            println!(
                "  {:<22}  {}  {:>5}  {:>3}  {:>3}  {:>3}  {:>3}  {:>3}  {:>3}",
                name_display,
                types_padded,
                p.bst().to_string().bold(),
                p.stats.hp,
                p.stats.attack,
                p.stats.defense,
                p.stats.special_attack,
                p.stats.special_defense,
                p.stats.speed,
            );

            // Per-pokemon weaknesses
            let mut weakness_parts = Vec::new();
            if !member.weaknesses_4x.is_empty() {
                let list = colored_type_list(&member.weaknesses_4x);
                weakness_parts.push(format!("{} {list}", "4x:".red().bold()));
            }
            if !member.weaknesses_2x.is_empty() {
                let list = colored_type_list(&member.weaknesses_2x);
                weakness_parts.push(format!("{} {list}", "2x:".yellow()));
            }
            if !weakness_parts.is_empty() {
                println!("  {:<25} {}", "", weakness_parts.join("  "));
            }
        }

        // Coverage summary
        let cov = &plan.type_coverage;
        println!();
        let pct = cov.coverage_score * 100.0;
        let pct_display = if pct >= 80.0 {
            format!("{pct:.0}%").green()
        } else if pct >= 50.0 {
            format!("{pct:.0}%").yellow()
        } else {
            format!("{pct:.0}%").red()
        };
        println!(
            "  {} {pct_display} ({}/18 types)",
            "Offensive coverage:".bold(),
            cov.offensive_coverage.len()
        );

        if !cov.offensive_coverage.is_empty() {
            println!(
                "    {} {}",
                "SE against:".dimmed(),
                colored_type_list(&cov.offensive_coverage)
            );
        }

        if !cov.uncovered_types.is_empty() {
            println!(
                "    {} {}",
                "No SE into:".dimmed(),
                colored_type_list(&cov.uncovered_types)
            );
        }

        if !cov.defensive_weaknesses.is_empty() {
            println!(
                "    {} {}",
                "Shared weaknesses:".dimmed(),
                colored_type_list(&cov.defensive_weaknesses)
            );
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stat_filter_ge() {
        let f = parse_stat_filter("ge500").unwrap();
        assert!(f(500));
        assert!(f(501));
        assert!(!f(499));
    }

    #[test]
    fn test_parse_stat_filter_gt() {
        let f = parse_stat_filter("gt100").unwrap();
        assert!(!f(100));
        assert!(f(101));
    }

    #[test]
    fn test_parse_stat_filter_le() {
        let f = parse_stat_filter("le50").unwrap();
        assert!(f(50));
        assert!(f(49));
        assert!(!f(51));
    }

    #[test]
    fn test_parse_stat_filter_lt() {
        let f = parse_stat_filter("lt200").unwrap();
        assert!(f(199));
        assert!(!f(200));
    }

    #[test]
    fn test_parse_stat_filter_eq() {
        let f = parse_stat_filter("eq120").unwrap();
        assert!(f(120));
        assert!(!f(119));
        assert!(!f(121));
    }

    #[test]
    fn test_parse_stat_filter_bare_number() {
        // Bare number defaults to ge
        let f = parse_stat_filter("500").unwrap();
        assert!(f(500));
        assert!(f(600));
        assert!(!f(499));
    }

    #[test]
    fn test_parse_stat_filter_invalid() {
        assert!(parse_stat_filter("geabc").is_err());
        assert!(parse_stat_filter("").is_err());
    }
}
