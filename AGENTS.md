# PokePlanner Knowledge Map

## Key Truths

1. **Don't guess shapes, build types to enforce structures and use them across API boundaries.**
   - All data structures live in `pokeplanner-core` (`crates/pokeplanner-core/src/`)
   - Models (`model.rs`), errors (`error.rs`), job types (`job.rs`), and team types (`team.rs`) are the single source of truth
   - Both REST and gRPC handlers convert to/from these core types — never define ad-hoc structures in API layers

2. **Reference this knowledge map as the first order of research.**
   - Before reading code, check this file and `docs/` for answers
   - Only dive into source code if the knowledge map doesn't cover your question

3. **Focus on building intent documents while building any code.**
   - Every code change should be reflected in the relevant documentation
   - `docs/ARCHITECTURE.md` — system design and data flow
   - `docs/DEPENDENCIES.md` — dependency choices and rationale
   - `docs/STRUCTURE.md` — repository layout
   - `docs/IMPLEMENTATION_CHECKLIST.md` — tracks implementation progress for crash resilience

4. **Triple helix: intent documents, tests, and functionality.**
   - All three must stay in sync. When changing code, update tests and docs together
   - Intent documents are a primary actor, not an afterthought

5. **Follow idiomatic Rust testing conventions.**
   - Unit tests go **inline** in the same file as the code they test, inside a `#[cfg(test)] mod tests { ... }` block at the bottom of the file
   - Never create separate `src/tests.rs` files — this splits tests from the code they cover and is not idiomatic Rust
   - Use `use super::*;` inside the test module to access the parent module's items
   - Integration tests (cross-crate, end-to-end) go in a top-level `tests/` directory per crate
   - Run all tests with `cargo test` from the workspace root

6. **Always run `just ci` (or at minimum `just format lint check test`) before committing.**
   - `just format` — check formatting (`cargo fmt --all -- --check`); CI rejects unformatted code
   - `just lint` — run clippy (`cargo clippy --workspace --all-targets -- -D warnings`); warnings are errors
   - `just check` — type-check the workspace
   - `just test` — run all tests
   - `just build` — build release binaries
   - `just ci` — runs all of the above in sequence
   - `just fmt` — auto-fix formatting (not a check, actually rewrites files)
   - `just install-hooks` — install a git pre-commit hook that runs `format`, `lint`, and `check` automatically
   - These commands must pass cleanly before any commit. Do not skip them.

## Architecture Quick Reference

- **Core types**: `crates/pokeplanner-core/` — shared models (Pokemon, Move, MoveStatChange, LearnsetEntry, DetailedLearnsetEntry, RecommendedMove, MoveRole), errors, job types, team types
- **Storage**: `crates/pokeplanner-storage/` — `Storage` trait + `JsonFileStorage`
- **PokeAPI Client**: `crates/pokeplanner-pokeapi/` — `PokeApiClient` trait + `PokeApiHttpClient` with disk cache and rate limiting. `MoveResponse` includes `meta` (drain, stat_chance, etc.) and `stat_changes` fields for move safety filtering
- **Service**: `crates/pokeplanner-service/` — business logic, job orchestration, team planner, move selector, type chart
- **Telemetry**: `crates/pokeplanner-telemetry/` — shared observability init (tracing subscriber, OTEL traces, metrics), `Metrics` struct, `TelemetryGuard`
- **REST API**: `crates/pokeplanner-api-rest/` — Axum server on port 3000
- **gRPC API**: `crates/pokeplanner-api-grpc/` — Tonic server on port 50051
- **CLI**: `crates/pokeplanner-cli/` — Clap CLI (`pokeplanner` binary)
- **Proto**: `proto/pokeplanner.proto` — gRPC service definitions

## Storage Interface

The `Storage` trait (`crates/pokeplanner-storage/src/traits.rs`) provides:
- `save_job`, `get_job`, `list_jobs`, `update_job`
- Uses native async via `impl Future` return types (no `async-trait` dependency)
- Currently implemented by `JsonFileStorage` (JSON files in `data/jobs/`)
- Designed for future swap to SQL or NoSQL — only implement the trait

## PokeAPI Client Interface

The `PokeApiClient` trait (`crates/pokeplanner-pokeapi/src/traits.rs`) provides:
- `get_version_groups`, `get_game_pokemon`, `get_pokemon`, `get_species_varieties`, `get_type_chart`
- Uses native async via `impl Future` return types (same pattern as `Storage`)
- Currently implemented by `PokeApiHttpClient` with:
  - Configurable `base_url` via `PokeApiClientConfig` (defaults to `https://pokeapi.co/api/v2`, overridden in tests to point at `wiremock` mock server)
  - Disk cache at `data/cache/` with 1-year TTL
  - Rate limiting via `governor` (default 20 req/s, burst 5 — configurable via `PokeApiClientConfig`)
  - Concurrent fetching via `BufferedUnordered` (10 concurrent requests)
  - Single shared rate limiter across all concurrent jobs and API handlers
- `PokePlannerService<S: Storage, P: PokeApiClient>` is generic over both — concrete types resolved at each binary's `main()`

## PokeAPI Navigation Chain

```
version-group/{name} → pokedexes[]
pokedex/{name} → pokemon_entries[] (species names)
pokemon-species/{name} → varieties[] (base, mega, regional forms)
pokemon/{form_name} → stats[], types[]
type/{name} → damage_relations
```

Pokedex entries reference **species only**. Megas, regional forms, and Gigantamax are non-default varieties discovered via the species endpoint.

## Caching Strategy

- Raw API responses cached per-resource in `~/.pokeplanner/cache/{category}/{key}.json`
- Aggregated results cached in `~/.pokeplanner/cache/game-pokemon/` and `~/.pokeplanner/cache/type-chart/`
- 1-year TTL. Bypass with `--no-cache` / `?no_cache=true` / gRPC `no_cache: true`
- On cache corruption: log, delete, treat as miss

## Job Lifecycle

`Pending` → `Running` → `Completed` | `Failed`

Jobs are submitted, assigned a UUID, and processed asynchronously via `tokio::spawn`. Team planning jobs include `progress` tracking (phase, completed/total steps).

## Team Planning Algorithm

- **N ≤ 25**: exact brute-force (provably optimal)
- **N > 25**: greedy beam search (beam width 50)
- Score = 0.4 × offensive coverage + 0.3 × defensive score + 0.3 × normalized BST
- Configurable `top_k` (default 5)
- v1: base type chart only (no abilities/moves)

## API Endpoints

### REST (port 3000)
| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/jobs` | Submit a generic job |
| GET | `/jobs` | List all jobs |
| GET | `/jobs/{id}` | Get job by ID |
| GET | `/version-groups` | List available games |
| GET | `/version-groups/{name}/pokemon` | Get pokemon for a game (query: `min_bst`, `sort_by`, `sort_order`, `no_cache`, `include_variants`) |
| GET | `/pokemon/{name}` | Get pokemon details |
| POST | `/teams/plan` | Submit team planning job (body: `TeamPlanRequest`, optional `learnset_version_group`) |
| POST | `/teams/analyze` | Synchronous type coverage analysis |

### gRPC (port 50051)
| RPC | Description |
|-----|-------------|
| `Health` | Health check |
| `Ping` | Echo ping/pong |
| `SubmitJob` | Submit a new job |
| `GetJob` | Get job by ID (includes kind, progress, result data) |
| `ListJobs` | List all jobs |
| `GetVersionGroups` | List available games (version groups) |
| `GetGamePokemon` | Get pokemon for a game (supports min_bst, sort, limit, variants) |
| `GetPokedexPokemon` | Get pokemon from a pokedex |
| `GetPokemon` | Get single pokemon details |
| `PlanTeam` | Submit team planning job (game/pokedex/custom source, counter-team, `learnset_version_group`) |
| `AnalyzeTeam` | Synchronous type coverage analysis |

### CLI
| Command | Description |
|---------|-------------|
| `list-games` | List available games (version groups) |
| `game-pokemon <game>` | List pokemon for a game (`--min-bst`, `--sort-by`, `--sort-order`, `--include-variants`) |
| `pokedex-pokemon <pokedex>` | List pokemon from a pokedex (`--min-bst`, `--sort-by`, `--sort-order`) |
| `pokemon show <name>` | Get pokemon details (colored stat bars, types, other forms) (`--show-learnset`, `--learnset-game`) |
| `pokemon search [filters]` | Search pokemon by type, stats, name, game, variant type (see below) |
| `moves show <name>` | Get detailed move info (type, power, accuracy, pp, effect) |
| `moves search <pokemon>` | Search a pokemon's learnset (`--game`, `--type`, `--damage-class`, `--min-power`, `--learn-method`, `--sort-by`) |
| `plan-team` | Plan optimal team (`--game` (CSV) or `--pokedex` or `--pokemon`, `--min-bst`, `--top-k`, `--exclude-variant-type`, `--learnset-game`) |
| `analyze-team <names>` | Analyze type coverage |
| `cache stats` | Show cache statistics (entry counts, sizes, location) |
| `cache populate games` | Pre-fetch all version group metadata |
| `cache populate type-chart` | Pre-fetch the type effectiveness chart |
| `cache populate game <name>` | Pre-fetch all pokemon for a game (`--include-variants`) |
| `cache populate pokedex <name>` | Pre-fetch all pokemon from a pokedex (`--include-variants`) |
| `cache populate all` | Pre-fetch everything (`--include-variants`); uses lower concurrency (3 req, 5 rps) |
| `cache clear all` | Remove all cached data |
| `cache clear stale` | Remove only expired entries |
| `cache clear game <name>` | Remove cached data for a game |
| `cache clear pokedex <name>` | Remove cached data for a pokedex |
| `cache clear pokemon <name>` | Remove cached data for a pokemon |
| `cache clear type-chart` | Remove the cached type chart |
| `unusable add <names>` | Mark pokemon as unusable (comma-separated form names) |
| `unusable remove <names>` | Unmark pokemon as unusable |
| `unusable list` | List all pokemon marked as unusable |
| `unusable clear` | Clear the entire unusable list |

## Testing

Integration tests use two strategies:
- **HTTP-level mocking** (`pokeplanner-pokeapi`): `wiremock` spins up a mock HTTP server; `PokeApiHttpClient` is configured with `base_url` pointing at it. Tests exercise the full HTTP→cache→parse pipeline. Fixtures live in `crates/pokeplanner-pokeapi/tests/fixtures/`.
- **Trait-level mocking** (`pokeplanner-api-rest`): `create_router` is generic over `Storage` and `PokeApiClient`, so integration tests pass in a `MockPokeApi` struct and drive the router via `tower::ServiceExt::oneshot`.

```bash
cargo test                                                       # All tests
cargo test -p pokeplanner-pokeapi --test http_client_integration  # HTTP client integration tests
cargo test -p pokeplanner-api-rest --test rest_api_integration    # REST API integration tests
```

## Build & Run

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests
cargo run -p pokeplanner-cli -- hello                          # CLI hello world
cargo run -p pokeplanner-cli -- list-games                     # List available games
cargo run -p pokeplanner-cli -- game-pokemon red-blue          # Pokemon in Red/Blue
cargo run -p pokeplanner-cli -- plan-team --game red-blue --wait               # Plan optimal team
cargo run -p pokeplanner-cli -- plan-team --game red-blue,gold-silver --wait  # Plan across games
cargo run -p pokeplanner-cli -- -v list-games                   # Verbose (info level) output
cargo run -p pokeplanner-cli -- -vv plan-team --game red-blue   # Debug-level output (filtering decisions, cache hits)
cargo run -p pokeplanner-api-rest                              # Start REST server (--host, --port, --cache-dir, --data-dir)
cargo run -p pokeplanner-api-rest -- --otlp-endpoint http://localhost:4317  # REST with OTEL export
cargo run -p pokeplanner-api-grpc                              # Start gRPC server (--host, --port, --cache-dir, --data-dir)
```
