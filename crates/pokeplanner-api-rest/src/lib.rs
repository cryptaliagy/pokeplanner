use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use pokeplanner_core::{
    AppError, HealthResponse, PokemonQueryParams, SortField, SortOrder, TeamPlanRequest,
};
use pokeplanner_service::PokePlannerService;
use serde::Deserialize;
use serde_json::json;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

pub fn create_router<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    service: Arc<PokePlannerService<S, P>>,
) -> Router {
    Router::new()
        .route("/health", get(health::<S, P>))
        .route("/jobs", post(submit_job::<S, P>))
        .route("/jobs", get(list_jobs::<S, P>))
        .route("/jobs/{id}", get(get_job::<S, P>))
        .route("/version-groups", get(list_version_groups::<S, P>))
        .route(
            "/version-groups/{name}/pokemon",
            get(get_game_pokemon::<S, P>),
        )
        .route("/pokedex/{name}/pokemon", get(get_pokedex_pokemon::<S, P>))
        .route("/pokemon/{name}", get(get_pokemon::<S, P>))
        .route("/teams/plan", post(plan_team::<S, P>))
        .route("/teams/analyze", post(analyze_team::<S, P>))
        .layer(TraceLayer::new_for_http())
        .with_state(service)
}

async fn health<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
) -> Json<HealthResponse> {
    Json(service.health())
}

async fn submit_job<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
) -> impl IntoResponse {
    match service.submit_job().await {
        Ok(job_id) => (StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))),
        Err(e) => error_response(e),
    }
}

async fn get_job<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let job_id = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid job ID" })),
            )
        }
    };
    match service.get_job(&job_id).await {
        Ok(job) => (StatusCode::OK, Json(json!(job))),
        Err(e) => error_response(e),
    }
}

async fn list_jobs<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
) -> impl IntoResponse {
    match service.list_jobs().await {
        Ok(jobs) => (StatusCode::OK, Json(json!({ "jobs": jobs }))),
        Err(e) => error_response(e),
    }
}

#[derive(Deserialize)]
struct GamePokemonQuery {
    min_bst: Option<u32>,
    no_cache: Option<bool>,
    sort_by: Option<SortField>,
    sort_order: Option<SortOrder>,
    include_variants: Option<bool>,
    limit: Option<usize>,
}

async fn list_version_groups<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Query(params): Query<NoCacheQuery>,
) -> impl IntoResponse {
    match service
        .list_version_groups(params.no_cache.unwrap_or(false))
        .await
    {
        Ok(groups) => (StatusCode::OK, Json(json!({ "version_groups": groups }))),
        Err(e) => error_response(e),
    }
}

#[derive(Deserialize)]
struct NoCacheQuery {
    no_cache: Option<bool>,
}

async fn get_game_pokemon<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Path(name): Path<String>,
    Query(params): Query<GamePokemonQuery>,
) -> impl IntoResponse {
    match service
        .get_game_pokemon(
            &name,
            &PokemonQueryParams {
                min_bst: params.min_bst,
                no_cache: params.no_cache.unwrap_or(false),
                sort_by: params.sort_by,
                sort_order: params.sort_order.unwrap_or_default(),
                include_variants: params.include_variants.unwrap_or(true),
                limit: params.limit,
            },
        )
        .await
    {
        Ok(pokemon) => (
            StatusCode::OK,
            Json(json!({ "pokemon": pokemon, "count": pokemon.len() })),
        ),
        Err(e) => error_response(e),
    }
}

async fn get_pokedex_pokemon<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Path(name): Path<String>,
    Query(params): Query<GamePokemonQuery>,
) -> impl IntoResponse {
    match service
        .get_pokedex_pokemon(
            &name,
            &PokemonQueryParams {
                min_bst: params.min_bst,
                no_cache: params.no_cache.unwrap_or(false),
                sort_by: params.sort_by,
                sort_order: params.sort_order.unwrap_or_default(),
                include_variants: params.include_variants.unwrap_or(true),
                limit: params.limit,
            },
        )
        .await
    {
        Ok(pokemon) => (
            StatusCode::OK,
            Json(json!({ "pokemon": pokemon, "count": pokemon.len() })),
        ),
        Err(e) => error_response(e),
    }
}

async fn get_pokemon<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Path(name): Path<String>,
    Query(params): Query<NoCacheQuery>,
) -> impl IntoResponse {
    match service
        .get_pokemon(&name, params.no_cache.unwrap_or(false))
        .await
    {
        Ok(pokemon) => (StatusCode::OK, Json(json!(pokemon))),
        Err(e) => error_response(e),
    }
}

async fn plan_team<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Json(request): Json<TeamPlanRequest>,
) -> impl IntoResponse {
    match service.submit_team_plan(request).await {
        Ok(job_id) => (StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))),
        Err(e) => error_response(e),
    }
}

#[derive(Deserialize)]
struct AnalyzeTeamRequest {
    pokemon_names: Vec<String>,
    #[serde(default)]
    no_cache: bool,
}

async fn analyze_team<S: pokeplanner_storage::Storage, P: pokeplanner_pokeapi::PokeApiClient>(
    State(service): State<Arc<PokePlannerService<S, P>>>,
    Json(request): Json<AnalyzeTeamRequest>,
) -> impl IntoResponse {
    match service
        .analyze_team(request.pokemon_names, request.no_cache)
        .await
    {
        Ok(coverage) => (StatusCode::OK, Json(json!(coverage))),
        Err(e) => error_response(e),
    }
}

fn error_response(e: AppError) -> (StatusCode, Json<serde_json::Value>) {
    let (status, msg) = match &e {
        AppError::NotFound(_) | AppError::JobNotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    (status, Json(json!({ "error": msg })))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use pokeplanner_core::{BaseStats, Pokemon, PokemonType};
    use pokeplanner_pokeapi::{PokeApiClient, TypeEffectivenessData, VersionGroupInfo};
    use pokeplanner_service::PokePlannerService;
    use pokeplanner_storage::JsonFileStorage;
    use tower::ServiceExt;

    use super::*;

    struct MockPokeApi;

    impl PokeApiClient for MockPokeApi {
        async fn get_version_groups(
            &self,
            _no_cache: bool,
        ) -> Result<Vec<VersionGroupInfo>, AppError> {
            Ok(vec![])
        }
        async fn get_game_pokemon(
            &self,
            _vg: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![])
        }
        async fn get_pokemon(&self, name: &str, _no_cache: bool) -> Result<Pokemon, AppError> {
            Ok(Pokemon {
                species_name: name.to_string(),
                form_name: name.to_string(),
                pokedex_number: 1,
                types: vec![PokemonType::Normal],
                stats: BaseStats {
                    hp: 50,
                    attack: 50,
                    defense: 50,
                    special_attack: 50,
                    special_defense: 50,
                    speed: 50,
                },
                is_default_form: true,
            })
        }
        async fn get_species_varieties(
            &self,
            _name: &str,
            _no_cache: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![])
        }
        async fn get_pokedex_pokemon(
            &self,
            _pokedex_name: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![])
        }
        async fn get_type_chart(&self, _no_cache: bool) -> Result<TypeEffectivenessData, AppError> {
            Ok(TypeEffectivenessData { entries: vec![] })
        }
        async fn get_pokemon_learnset(
            &self,
            _pokemon_name: &str,
            _version_group: Option<&str>,
            _no_cache: bool,
        ) -> Result<Vec<pokeplanner_core::LearnsetEntry>, AppError> {
            Ok(vec![])
        }
        async fn get_move(
            &self,
            _move_name: &str,
            _no_cache: bool,
        ) -> Result<pokeplanner_core::Move, AppError> {
            Ok(pokeplanner_core::Move {
                name: _move_name.to_string(),
                move_type: PokemonType::Normal,
                power: None,
                accuracy: None,
                pp: None,
                damage_class: "status".to_string(),
                priority: 0,
                effect: None,
                drain: 0,
                self_stat_changes: Vec::new(),
            })
        }
    }

    async fn make_app() -> Router {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.keep()).await.unwrap());
        let pokeapi = Arc::new(MockPokeApi);
        let service = Arc::new(PokePlannerService::new(storage, pokeapi));
        create_router(service)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_submit_job_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::post("/jobs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_list_jobs_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::get("/jobs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_nonexistent_job() {
        let app = make_app().await;
        let resp = app
            .oneshot(
                Request::get("/jobs/00000000-0000-0000-0000-000000000000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_version_groups_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::get("/version-groups").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
