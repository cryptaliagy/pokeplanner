use std::sync::Arc;

use clap::{Parser, Subcommand};
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "pokeplanner", about = "PokePlanner CLI", version)]
struct Cli {
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let storage = Arc::new(JsonFileStorage::new("data/jobs".into()).await?);
    let service = PokePlannerService::new(storage);

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
    }

    Ok(())
}
