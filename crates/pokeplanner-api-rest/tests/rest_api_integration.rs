use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use pokeplanner_core::{AppError, BaseStats, Pokemon, PokemonType};
use pokeplanner_pokeapi::{
    PokeApiClient, TypeEffectivenessData, TypeEffectivenessEntry, VersionGroupInfo,
};
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use serde_json::Value;
use tower::ServiceExt;

use pokeplanner_api_rest::create_router;

struct MockPokeApi;

fn make_test_pokemon(name: &str, types: Vec<PokemonType>, bst: u32) -> Pokemon {
    let per = bst / 6;
    let rem = bst - per * 6;
    Pokemon {
        species_name: name.to_string(),
        form_name: name.to_string(),
        pokedex_number: 1,
        types,
        stats: BaseStats {
            hp: per + rem,
            attack: per,
            defense: per,
            special_attack: per,
            special_defense: per,
            speed: per,
        },
        is_default_form: true,
    }
}

fn test_type_chart() -> TypeEffectivenessData {
    TypeEffectivenessData {
        entries: vec![
            TypeEffectivenessEntry {
                attack_type: PokemonType::Fire,
                defend_type: PokemonType::Grass,
                multiplier: 2.0,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Water,
                defend_type: PokemonType::Fire,
                multiplier: 2.0,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Grass,
                defend_type: PokemonType::Water,
                multiplier: 2.0,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Electric,
                defend_type: PokemonType::Water,
                multiplier: 2.0,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Fire,
                defend_type: PokemonType::Water,
                multiplier: 0.5,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Water,
                defend_type: PokemonType::Grass,
                multiplier: 0.5,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Grass,
                defend_type: PokemonType::Fire,
                multiplier: 0.5,
            },
            TypeEffectivenessEntry {
                attack_type: PokemonType::Normal,
                defend_type: PokemonType::Ghost,
                multiplier: 0.0,
            },
        ],
    }
}

impl PokeApiClient for MockPokeApi {
    async fn get_version_groups(&self, _no_cache: bool) -> Result<Vec<VersionGroupInfo>, AppError> {
        Ok(vec![VersionGroupInfo {
            name: "red-blue".to_string(),
            versions: vec!["red".to_string(), "blue".to_string()],
            pokedexes: vec!["kanto".to_string()],
            generation: "generation-i".to_string(),
        }])
    }

    async fn get_game_pokemon(
        &self,
        _vg: &str,
        _no_cache: bool,
        _include_variants: bool,
    ) -> Result<Vec<Pokemon>, AppError> {
        Ok(vec![
            make_test_pokemon("pikachu", vec![PokemonType::Electric], 320),
            make_test_pokemon(
                "charizard",
                vec![PokemonType::Fire, PokemonType::Flying],
                534,
            ),
            make_test_pokemon("mewtwo", vec![PokemonType::Psychic], 680),
        ])
    }

    async fn get_pokemon(&self, name: &str, _no_cache: bool) -> Result<Pokemon, AppError> {
        match name {
            "pikachu" => Ok(make_test_pokemon(
                "pikachu",
                vec![PokemonType::Electric],
                320,
            )),
            "charizard" => Ok(make_test_pokemon(
                "charizard",
                vec![PokemonType::Fire, PokemonType::Flying],
                534,
            )),
            "mewtwo" => Ok(make_test_pokemon("mewtwo", vec![PokemonType::Psychic], 680)),
            other => Err(AppError::NotFound(format!("Pokemon {other} not found"))),
        }
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
        Ok(test_type_chart())
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

async fn make_app() -> axum::Router {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(JsonFileStorage::new(dir.keep()).await.unwrap());
    let pokeapi = Arc::new(MockPokeApi);
    let service = Arc::new(PokePlannerService::new(storage, pokeapi));
    create_router(service, None)
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_health_response_body() {
    let app = make_app().await;
    let resp = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_get_pokemon_response_shape() {
    let app = make_app().await;
    let resp = app
        .oneshot(
            Request::get("/pokemon/pikachu")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["species_name"], "pikachu");
    assert_eq!(body["form_name"], "pikachu");
    assert!(body["types"].is_array());
    assert_eq!(body["types"][0], "electric");
    assert!(body["stats"].is_object());
    assert!(body["stats"]["hp"].is_number());
}

#[tokio::test]
async fn test_game_pokemon_with_filters() {
    let app = make_app().await;
    let resp = app
        .oneshot(
            Request::get(
                "/version-groups/red-blue/pokemon?min_bst=400&sort_by=bst&sort_order=desc",
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    let pokemon = body["pokemon"].as_array().unwrap();
    // Only charizard (534) and mewtwo (680) have BST >= 400
    assert_eq!(pokemon.len(), 2);
    // Sorted desc by BST: mewtwo first, then charizard
    assert_eq!(pokemon[0]["species_name"], "mewtwo");
    assert_eq!(pokemon[1]["species_name"], "charizard");
}

#[tokio::test]
async fn test_analyze_team_response() {
    let app = make_app().await;
    let resp = app
        .oneshot(
            Request::post("/teams/analyze")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "pokemon_names": ["pikachu", "charizard"]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["coverage_score"].is_number());
    assert!(body["offensive_coverage"].is_array());
    assert!(body["defensive_weaknesses"].is_array());
    assert!(body["uncovered_types"].is_array());
}

#[tokio::test]
async fn test_plan_team_returns_job_id() {
    let app = make_app().await;
    let resp = app
        .oneshot(
            Request::post("/teams/plan")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "source": {"game": {"version_groups": ["red-blue"]}},
                        "no_cache": false,
                        "include_variants": false
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = body_json(resp).await;
    let job_id = body["job_id"].as_str().unwrap();
    // Should be a valid UUID
    uuid::Uuid::parse_str(job_id).expect("job_id should be a valid UUID");
}

#[tokio::test]
async fn test_plan_team_with_learnset_game_returns_job_id() {
    let app = make_app().await;
    let resp = app
        .oneshot(
            Request::post("/teams/plan")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&serde_json::json!({
                        "source": {"game": {"version_groups": ["red-blue"]}},
                        "no_cache": false,
                        "include_variants": false,
                        "learnset_version_group": "red-blue"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = body_json(resp).await;
    let job_id = body["job_id"].as_str().unwrap();
    uuid::Uuid::parse_str(job_id).expect("job_id should be a valid UUID");
}

#[tokio::test]
async fn test_nonexistent_job_returns_404() {
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
    let body = body_json(resp).await;
    assert!(body["error"].is_string());
}
