use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use pokeplanner_core::{
    PokemonQueryParams, PokemonType, SortField, SortOrder, TeamPlanRequest, TeamSource,
};
use pokeplanner_pokeapi::PokeApiHttpClient;
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pokeplanner"))
        .unwrap_or_else(|| PathBuf::from(".pokeplanner"))
}

#[derive(Parser)]
#[command(name = "pokeplanner", about = "PokePlanner CLI", version)]
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
    /// Say hello
    Hello,
    /// Check service health
    Health,
    /// Submit a new job
    SubmitJob,
    /// Get job status by ID
    GetJob {
        /// Job ID (UUID)
        id: String,
    },
    /// List all jobs
    ListJobs,
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
        #[arg(long)]
        include_variants: bool,
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
        #[arg(long)]
        include_variants: bool,
        /// Limit number of results (useful with --sort-by for "top N")
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get details for a specific pokemon
    Pokemon {
        /// Pokemon name (e.g., "pikachu", "charizard-mega-x")
        name: String,
        #[arg(long)]
        no_cache: bool,
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
        #[arg(long)]
        include_variants: bool,
        /// Enemy pokemon to counter (comma-separated). Optimizes team against this specific team.
        #[arg(long, value_delimiter = ',')]
        counter: Option<Vec<String>>,
        /// Wait for the job to complete and print results
        #[arg(long)]
        wait: bool,
    },
    /// Analyze type coverage for a team
    AnalyzeTeam {
        /// Pokemon names (comma-separated)
        #[arg(value_delimiter = ',')]
        pokemon: Vec<String>,
        #[arg(long)]
        no_cache: bool,
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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let storage = Arc::new(JsonFileStorage::new(cli.data_dir).await?);
    let pokeapi = Arc::new(PokeApiHttpClient::new(cli.cache_dir).await?);
    let service = PokePlannerService::new(storage, pokeapi);

    match cli.command {
        Commands::Hello => {
            println!("Hello from PokePlanner!");
        }
        Commands::Health => {
            let health = service.health();
            println!("{}", serde_json::to_string_pretty(&health)?);
        }
        Commands::SubmitJob => {
            let job_id = service.submit_job().await?;
            println!("Job submitted: {job_id}");
        }
        Commands::GetJob { id } => {
            let job_id = Uuid::parse_str(&id)?;
            let job = service.get_job(&job_id).await?;
            println!("{}", serde_json::to_string_pretty(&job)?);
        }
        Commands::ListJobs => {
            let jobs = service.list_jobs().await?;
            println!("{}", serde_json::to_string_pretty(&jobs)?);
        }
        Commands::ListGames { no_cache } => {
            let groups = service.list_version_groups(no_cache).await?;
            for group in &groups {
                println!(
                    "{} (versions: {}, pokedexes: {})",
                    group.name,
                    group.versions.join(", "),
                    group.pokedexes.join(", ")
                );
            }
        }
        Commands::GamePokemon {
            game,
            min_bst,
            sort_by,
            sort_order,
            no_cache,
            include_variants,
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
                        include_variants,
                        limit,
                    },
                )
                .await?;
            print_pokemon_list(&pokemon);
        }
        Commands::PokedexPokemon {
            pokedex,
            min_bst,
            sort_by,
            sort_order,
            no_cache,
            include_variants,
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
                        include_variants,
                        limit,
                    },
                )
                .await?;
            print_pokemon_list(&pokemon);
        }
        Commands::Pokemon { name, no_cache } => {
            let pokemon = service.get_pokemon(&name, no_cache).await?;
            println!("{}", serde_json::to_string_pretty(&pokemon)?);
        }
        Commands::PlanTeam {
            game,
            pokedex,
            pokemon,
            min_bst,
            top_k,
            no_cache,
            include_variants,
            counter,
            wait,
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

            let request = TeamPlanRequest {
                source,
                min_bst,
                no_cache,
                top_k: Some(top_k),
                include_variants,
                counter_team: counter,
            };

            let job_id = service.submit_team_plan(request).await?;
            println!("Team plan job submitted: {job_id}");

            if wait {
                println!("Waiting for results...");
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
                                println!("{}", result.message);
                                if let Some(data) = &result.data {
                                    let plans: Vec<pokeplanner_core::TeamPlan> =
                                        serde_json::from_value(data.clone())
                                            .unwrap_or_default();
                                    print_team_plans(&plans);
                                }
                            }
                            break;
                        }
                        pokeplanner_core::JobStatus::Failed => {
                            eprintln!();
                            if let Some(result) = &job.result {
                                eprintln!("Job failed: {}", result.message);
                            }
                            break;
                        }
                        _ => continue,
                    }
                }
            }
        }
        Commands::AnalyzeTeam { pokemon, no_cache } => {
            let coverage = service.analyze_team(pokemon, no_cache).await?;
            println!("{}", serde_json::to_string_pretty(&coverage)?);
        }
    }

    Ok(())
}

fn print_pokemon_list(pokemon: &[pokeplanner_core::Pokemon]) {
    println!("{} pokemon found:", pokemon.len());
    for p in pokemon {
        let types_str: Vec<String> = p.types.iter().map(|t| format!("{t}")).collect();
        let variant_marker = if !p.is_default_form { " *" } else { "" };
        println!(
            "  #{:>4} {:<25} [{:<20}] BST: {}{}",
            p.pokedex_number,
            p.form_name,
            types_str.join("/"),
            p.bst(),
            variant_marker,
        );
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

        // Header
        println!(
            "  {:<25} {:<20} {:>5}  {:>3} {:>3} {:>3} {:>3} {:>3} {:>3}",
            "Pokemon".bold(),
            "Types".bold(),
            "BST".bold(),
            "HP".bold(),
            "Atk".bold(),
            "Def".bold(),
            "SpA".bold(),
            "SpD".bold(),
            "Spe".bold(),
        );
        println!("  {}", "-".repeat(78).dimmed());

        for member in &plan.team {
            let p = &member.pokemon;
            let types_display: Vec<String> =
                p.types.iter().map(|t| format!("{}", color_type(t))).collect();
            let variant = if !p.is_default_form {
                " *".dimmed().to_string()
            } else {
                String::new()
            };
            println!(
                "  {:<25} {:<20} {:>5}  {:>3} {:>3} {:>3} {:>3} {:>3} {:>3}",
                format!("{}{variant}", p.form_name),
                types_display.join("/"),
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
            println!("    {} {}", "SE against:".dimmed(), colored_type_list(&cov.offensive_coverage));
        }

        if !cov.uncovered_types.is_empty() {
            println!("    {} {}", "No SE into:".dimmed(), colored_type_list(&cov.uncovered_types));
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
