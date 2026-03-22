use pokeplanner_core::PokemonType;
use pokeplanner_pokeapi::{PokeApiClient, PokeApiClientConfig, PokeApiHttpClient};
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn json_response(fixture: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(load_fixture(fixture), "application/json")
}

async fn setup() -> (MockServer, PokeApiHttpClient, TempDir) {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config = PokeApiClientConfig {
        cache_path: cache_dir.path().to_path_buf(),
        base_url: server.uri(),
        requests_per_second: 100,
        burst_size: 100,
    };
    let client = PokeApiHttpClient::with_config(config).await.unwrap();
    (server, client, cache_dir)
}

/// Mount mocks for fetching a single pokemon by name (pokemon + species endpoints).
async fn mount_pokemon_mocks(server: &MockServer, name: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/pokemon/{name}")))
        .respond_with(json_response(&format!("pokemon-{name}.json")))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/pokemon-species/{name}")))
        .respond_with(json_response(&format!("species-{name}.json")))
        .mount(server)
        .await;
}

/// Mount mocks for the full game pokemon chain: version-group → pokedex → species → pokemon.
async fn mount_game_pokemon_chain(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/version-group/red-blue"))
        .respond_with(json_response("version-group-red-blue.json"))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pokedex/kanto"))
        .respond_with(json_response("pokedex-kanto.json"))
        .mount(server)
        .await;
    for name in ["bulbasaur", "charmander", "squirtle"] {
        mount_pokemon_mocks(server, name).await;
    }
}

#[tokio::test]
async fn test_get_pokemon_parses_response() {
    let (server, client, _dir) = setup().await;
    mount_pokemon_mocks(&server, "bulbasaur").await;

    let pokemon = client.get_pokemon("bulbasaur", false).await.unwrap();

    assert_eq!(pokemon.species_name, "bulbasaur");
    assert_eq!(pokemon.form_name, "bulbasaur");
    assert_eq!(pokemon.pokedex_number, 1);
    assert_eq!(pokemon.types, vec![PokemonType::Grass, PokemonType::Poison]);
    assert_eq!(pokemon.stats.hp, 45);
    assert_eq!(pokemon.stats.attack, 49);
    assert_eq!(pokemon.stats.defense, 49);
    assert_eq!(pokemon.stats.special_attack, 65);
    assert_eq!(pokemon.stats.special_defense, 65);
    assert_eq!(pokemon.stats.speed, 45);
    assert!(pokemon.is_default_form);
}

#[tokio::test]
async fn test_get_version_groups() {
    let (server, client, _dir) = setup().await;

    Mock::given(method("GET"))
        .and(path("/version-group"))
        .and(query_param("limit", "100"))
        .respond_with(json_response("version-group-list.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/version-group/red-blue"))
        .respond_with(json_response("version-group-red-blue.json"))
        .mount(&server)
        .await;

    let groups = client.get_version_groups(false).await.unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "red-blue");
    assert_eq!(groups[0].versions, vec!["red", "blue"]);
    assert_eq!(groups[0].pokedexes, vec!["kanto"]);
}

#[tokio::test]
async fn test_get_game_pokemon_chain() {
    let (server, client, _dir) = setup().await;
    mount_game_pokemon_chain(&server).await;

    let pokemon = client
        .get_game_pokemon("red-blue", false, false)
        .await
        .unwrap();

    assert_eq!(pokemon.len(), 3);
    // Should be sorted by pokedex number
    assert_eq!(pokemon[0].species_name, "bulbasaur");
    assert_eq!(pokemon[0].pokedex_number, 1);
    assert_eq!(pokemon[1].species_name, "charmander");
    assert_eq!(pokemon[1].pokedex_number, 4);
    assert_eq!(pokemon[2].species_name, "squirtle");
    assert_eq!(pokemon[2].pokedex_number, 7);
}

#[tokio::test]
async fn test_get_type_chart() {
    let (server, client, _dir) = setup().await;

    let type_names = [
        "normal", "fire", "water", "electric", "grass", "ice", "fighting", "poison", "ground",
        "flying", "psychic", "bug", "rock", "ghost", "dragon", "dark", "steel", "fairy",
    ];
    for name in type_names {
        Mock::given(method("GET"))
            .and(path(format!("/type/{name}")))
            .respond_with(json_response(&format!("type-{name}.json")))
            .mount(&server)
            .await;
    }

    let chart = client.get_type_chart(false).await.unwrap();

    // Verify some known relationships exist
    let has_entry = |atk: PokemonType, def: PokemonType, mult: f64| {
        chart.entries.iter().any(|e| {
            e.attack_type == atk && e.defend_type == def && (e.multiplier - mult).abs() < 0.01
        })
    };

    // Fire is super effective against Grass
    assert!(has_entry(PokemonType::Fire, PokemonType::Grass, 2.0));
    // Water is super effective against Fire
    assert!(has_entry(PokemonType::Water, PokemonType::Fire, 2.0));
    // Normal has no effect on Ghost
    assert!(has_entry(PokemonType::Normal, PokemonType::Ghost, 0.0));
    // Electric is not very effective against Grass
    assert!(has_entry(PokemonType::Electric, PokemonType::Grass, 0.5));
    // Ground has no effect on Flying
    assert!(has_entry(PokemonType::Ground, PokemonType::Flying, 0.0));
}

#[tokio::test]
async fn test_caching_prevents_duplicate_requests() {
    let (server, client, _dir) = setup().await;

    Mock::given(method("GET"))
        .and(path("/pokemon/bulbasaur"))
        .respond_with(json_response("pokemon-bulbasaur.json"))
        .expect(1) // exactly 1 request
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pokemon-species/bulbasaur"))
        .respond_with(json_response("species-bulbasaur.json"))
        .expect(1)
        .mount(&server)
        .await;

    let _first = client.get_pokemon("bulbasaur", false).await.unwrap();
    let second = client.get_pokemon("bulbasaur", false).await.unwrap();

    assert_eq!(second.species_name, "bulbasaur");
    // wiremock will panic on drop if more than 1 request was received
}

#[tokio::test]
async fn test_no_cache_bypasses_disk_cache() {
    let (server, client, _dir) = setup().await;

    Mock::given(method("GET"))
        .and(path("/pokemon/bulbasaur"))
        .respond_with(json_response("pokemon-bulbasaur.json"))
        .expect(2) // expect 2 requests
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pokemon-species/bulbasaur"))
        .respond_with(json_response("species-bulbasaur.json"))
        .expect(2)
        .mount(&server)
        .await;

    let _first = client.get_pokemon("bulbasaur", false).await.unwrap();
    let second = client.get_pokemon("bulbasaur", true).await.unwrap();

    assert_eq!(second.species_name, "bulbasaur");
}

#[tokio::test]
async fn test_http_error_returns_app_error() {
    let (server, client, _dir) = setup().await;

    Mock::given(method("GET"))
        .and(path("/pokemon/missingno"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let result = client.get_pokemon("missingno", false).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("404"), "Error should mention 404: {err}");
}

#[tokio::test]
async fn test_get_species_varieties() {
    let (server, client, _dir) = setup().await;

    // Charizard has 2 varieties: default + mega-x
    Mock::given(method("GET"))
        .and(path("/pokemon-species/charizard"))
        .respond_with(json_response("species-charizard.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pokemon/charizard"))
        .respond_with(json_response("pokemon-charizard.json"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pokemon/charizard-mega-x"))
        .respond_with(json_response("pokemon-charizard-mega-x.json"))
        .mount(&server)
        .await;

    let varieties = client
        .get_species_varieties("charizard", false)
        .await
        .unwrap();

    assert_eq!(varieties.len(), 2);
    let names: Vec<&str> = varieties.iter().map(|p| p.form_name.as_str()).collect();
    assert!(names.contains(&"charizard"));
    assert!(names.contains(&"charizard-mega-x"));

    // Mega should have dragon type
    let mega = varieties
        .iter()
        .find(|p| p.form_name == "charizard-mega-x")
        .unwrap();
    assert!(mega.types.contains(&PokemonType::Dragon));
    assert!(!mega.is_default_form);
}
