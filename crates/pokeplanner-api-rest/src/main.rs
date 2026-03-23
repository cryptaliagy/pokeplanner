use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pokeplanner_pokeapi::PokeApiHttpClient;
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use pokeplanner_telemetry::{LogFormat, ServerTelemetryConfig};

use pokeplanner_api_rest::create_router;

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pokeplanner"))
        .unwrap_or_else(|| PathBuf::from(".pokeplanner"))
}

#[derive(Parser)]
#[command(
    name = "pokeplanner-rest",
    about = "PokePlanner REST API server",
    version
)]
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

    /// OTLP exporter endpoint (e.g., http://localhost:4317). OTEL disabled when absent.
    #[arg(long, env = "OTEL_EXPORTER_OTLP_ENDPOINT")]
    otlp_endpoint: Option<String>,

    /// Log output format
    #[arg(long, default_value = "text")]
    log_format: LogFormat,

    /// Base log level filter
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let _guard = pokeplanner_telemetry::init_server_telemetry(ServerTelemetryConfig {
        otlp_endpoint: cli.otlp_endpoint,
        log_format: cli.log_format,
        log_level: cli.log_level,
    });

    let storage = Arc::new(
        JsonFileStorage::new(cli.data_dir)
            .await
            .expect("Failed to initialize storage"),
    );
    let metrics = pokeplanner_telemetry::Metrics::from_global();
    let pokeapi = Arc::new(
        PokeApiHttpClient::new(cli.cache_dir)
            .await
            .expect("Failed to initialize PokeAPI client")
            .with_metrics(metrics.clone()),
    );
    let service = Arc::new(PokePlannerService::new(storage, pokeapi).with_metrics(metrics.clone()));

    let app = create_router(service, Some(metrics));

    let addr = format!("{}:{}", cli.host, cli.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    tracing::info!("REST API listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");
    tracing::info!("Shutdown signal received");
}
