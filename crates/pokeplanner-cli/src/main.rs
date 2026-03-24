mod commands;
mod display;
mod unusable;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pokeplanner_core::{PokemonQueryParams, SortField, SortOrder, TeamPlanRequest, TeamSource};
use pokeplanner_pokeapi::PokeApiHttpClient;
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use unusable::UnusableStore;

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pokeplanner"))
        .unwrap_or_else(|| PathBuf::from(".pokeplanner"))
}

#[derive(Parser)]
#[command(
    name = "pokeplanner",
    about = "PokePlanner CLI — build optimal Pokemon teams",
    version
)]
struct Cli {
    /// Directory for cached PokeAPI data
    #[arg(long, global = true, default_value_os_t = default_data_dir().join("cache"))]
    cache_dir: PathBuf,

    /// Directory for job storage data
    #[arg(long, global = true, default_value_os_t = default_data_dir().join("jobs"))]
    data_dir: PathBuf,

    /// Increase verbosity (-v for info, -vv for debug)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

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
        /// Version group for learnset-based move selection (e.g., "red-blue").
        /// Defaults to the game for --game sources; required for --pokedex/--pokemon move selection.
        #[arg(long)]
        learnset_game: Option<String>,
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
    /// Look up or search for moves
    Moves {
        #[command(subcommand)]
        action: MoveAction,
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

#[derive(Args)]
struct PokemonSearchArgs {
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
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum PokemonAction {
    /// Get details for a specific pokemon
    Show {
        /// Pokemon name (e.g., "pikachu", "charizard-mega-x")
        name: String,
        #[arg(long)]
        no_cache: bool,
        /// Display the pokemon's learnset (moves it can learn)
        #[arg(long)]
        show_learnset: bool,
        /// Filter learnset by game (version group name, e.g., "red-blue")
        #[arg(long)]
        learnset_game: Option<String>,
    },
    /// Search for pokemon matching criteria
    Search(PokemonSearchArgs),
}

#[derive(Subcommand)]
enum MoveAction {
    /// Get details for a specific move
    Show {
        /// Move name (e.g., "thunderbolt", "flamethrower")
        name: String,
        #[arg(long)]
        no_cache: bool,
    },
    /// Search a pokemon's learnset for moves matching criteria
    Search {
        /// Pokemon whose learnset to search
        pokemon: String,
        /// Filter by game (version group name)
        #[arg(long)]
        game: Option<String>,
        /// Filter by move type (e.g., "fire", "water")
        #[arg(long)]
        r#type: Option<String>,
        /// Filter by damage class (physical, special, status)
        #[arg(long)]
        damage_class: Option<String>,
        /// Minimum power
        #[arg(long)]
        min_power: Option<u32>,
        /// Filter by learn method (level-up, machine, egg, tutor)
        #[arg(long)]
        learn_method: Option<String>,
        /// Sort by field (name, power, accuracy, pp, level)
        #[arg(long, default_value = "level")]
        sort_by: String,
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
    /// Remove cached learnset data (for a specific pokemon or all)
    Learnset {
        /// Pokemon form name (omit to clear all learnset data)
        name: Option<String>,
    },
    /// Remove cached move data (for a specific move or all)
    #[command(name = "moves")]
    Moves {
        /// Move name (omit to clear all move data)
        name: Option<String>,
    },
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
    let cli = Cli::parse();
    pokeplanner_telemetry::init_cli_telemetry(cli.verbose);
    let cache_dir = cli.cache_dir;
    let base_dir = default_data_dir();

    let storage = Arc::new(JsonFileStorage::new(cli.data_dir).await?);
    let pokeapi = Arc::new(PokeApiHttpClient::new(cache_dir.clone()).await?);
    let service = PokePlannerService::new(storage, pokeapi);
    let mut unusable = UnusableStore::load(&base_dir).await?;

    match cli.command {
        Commands::ListGames { no_cache } => {
            commands::games::handle_list_games(&service, no_cache).await?;
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
            let params = PokemonQueryParams {
                min_bst,
                no_cache,
                sort_by: sort_by.map(SortField::from),
                sort_order: sort_order.into(),
                include_variants: !exclude_variants,
                limit,
            };
            commands::games::handle_game_pokemon(&service, &unusable, &game, &params).await?;
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
            let params = PokemonQueryParams {
                min_bst,
                no_cache,
                sort_by: sort_by.map(SortField::from),
                sort_order: sort_order.into(),
                include_variants: !exclude_variants,
                limit,
            };
            commands::games::handle_pokedex_pokemon(&service, &unusable, &pokedex, &params).await?;
        }
        Commands::Pokemon { action } => {
            commands::pokemon::handle_pokemon_action(action, &service, &unusable).await?;
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
            learnset_game,
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
                learnset_version_group: learnset_game,
            };

            commands::team::handle_plan_team(&service, request).await?;
        }
        Commands::AnalyzeTeam { pokemon, no_cache } => {
            commands::team::handle_analyze_team(&service, pokemon, no_cache).await?;
        }
        Commands::Moves { action } => {
            commands::moves::handle_move_action(action, &service).await?;
        }
        Commands::Cache { action } => {
            commands::cache::handle_cache_action(action, &cache_dir).await?;
        }
        Commands::Unusable { action } => {
            commands::unusable_cmd::handle_unusable_action(action, &mut unusable).await?;
        }
    }

    Ok(())
}
