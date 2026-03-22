use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pokeplanner_pokeapi::PokeApiHttpClient;
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tracing_subscriber::EnvFilter;

use pokeplanner_api_rest::create_router;

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pokeplanner"))
        .unwrap_or_else(|| PathBuf::from(".pokeplanner"))
}

#[derive(Parser)]
#[command(name = "pokeplanner-rest", about = "PokePlanner REST API server", version)]
struct Cli {
    /// Host address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// Directory for cached PokeAPI data
    #[arg(long, default_value_os_t = default_data_dir().join("cache"))]
    cache_dir: PathBuf,

    /// Directory for job storage data
    #[arg(long, default_value_os_t = default_data_dir().join("jobs"))]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let storage = Arc::new(
        JsonFileStorage::new(cli.data_dir)
            .await
            .expect("Failed to initialize storage"),
    );
    let pokeapi = Arc::new(
        PokeApiHttpClient::new(cli.cache_dir)
            .await
            .expect("Failed to initialize PokeAPI client"),
    );
    let service = Arc::new(PokePlannerService::new(storage, pokeapi));

    let app = create_router(service);

    let addr = format!("{}:{}", cli.host, cli.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    tracing::info!("REST API listening on {addr}");
    axum::serve(listener, app).await.expect("Server error");
}
