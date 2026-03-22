use std::sync::Arc;

use pokeplanner_pokeapi::PokeApiHttpClient;
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tracing_subscriber::EnvFilter;

use pokeplanner_api_rest::create_router;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let storage = Arc::new(
        JsonFileStorage::new("data/jobs".into())
            .await
            .expect("Failed to initialize storage"),
    );
    let pokeapi = Arc::new(
        PokeApiHttpClient::new("data/cache".into())
            .await
            .expect("Failed to initialize PokeAPI client"),
    );
    let service = Arc::new(PokePlannerService::new(storage, pokeapi));

    let app = create_router(service);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind");
    tracing::info!("REST API listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.expect("Server error");
}
