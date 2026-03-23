# PokePlanner

PokePlanner is a competitive Pokémon team planner built in Rust. Given a game (or set of games), it fetches every available Pokémon from [PokéAPI](https://pokeapi.co), scores them on offensive coverage, defensive resilience, and base stats, and returns optimally balanced teams. It also supports counter-team analysis — supply your opponent's team and PokePlanner will find the best response.

The project exposes three interfaces — a CLI, a REST API, and a gRPC API — all backed by the same service layer and pluggable storage.

## Features

- **Team planning** — brute-force optimal search for ≤25 candidates, greedy beam search for larger pools
- **Counter-team support** — plan a team specifically to counter a known opponent lineup
- **Type coverage analysis** — evaluate offensive and defensive type matchups for any team
- **Pokémon browser** — look up stats, types, forms, and learnsets for any Pokémon
- **Move browser** — search and filter a Pokémon's learnset by type, damage class, power, and learn method
- **Multi-game support** — combine Pokémon pools across games (e.g. `--game red-blue,gold-silver`)
- **Unusable list** — mark Pokémon you don't want included in team plans
- **Variant filtering** — include or exclude megas, regional forms, and Gigantamax with `--include-variants` / `--exclude-variant-type`

## Quickstart

```bash
# Build
cargo build

# Run tests
cargo test

# CLI examples
cargo run -p pokeplanner-cli -- list-games
cargo run -p pokeplanner-cli -- game-pokemon red-blue
cargo run -p pokeplanner-cli -- pokemon show charizard
cargo run -p pokeplanner-cli -- moves search pikachu --game red-blue
cargo run -p pokeplanner-cli -- plan-team --game red-blue
cargo run -p pokeplanner-cli -- analyze-team charizard,blastoise,venusaur

# REST API (default port 3000)
cargo run -p pokeplanner-api-rest

# gRPC API (default port 50051)
cargo run -p pokeplanner-api-grpc
```

## Why a CLI, gRPC, and REST API?

Honestly? Because it was fun to build all three. PokePlanner is a learning project, and one of the goals was to see what it takes to serve the same Rust business logic through completely different interfaces. None of this needs three separate APIs — a CLI alone would do the job just fine.

That said, each one teaches something different:

- **CLI** — the quickest way to actually use the thing. Good for poking around, scripting, and sanity-checking during development.
- **REST API** — the obvious choice if a web frontend ever materializes. Axum makes it almost too easy to bolt on.
- **gRPC API** — the excuse to learn Protobuf and Tonic. Strongly-typed service contracts, binary serialization, the whole deal. Overkill for a Pokémon planner? Absolutely.

The nice part is that all three are thin adapters over the same `PokePlannerService`, so adding a feature or fixing a bug only happens once in the service layer. The interfaces just translate requests in and responses out.

## Caching

PokePlanner aggressively caches PokéAPI responses to avoid hammering the upstream service. Understanding the caching layer matters if you're self-hosting or developing — it is the primary mechanism that keeps request volume low.

### How it works

Every PokéAPI response is cached to disk the first time it is fetched. Subsequent requests for the same resource are served from the local cache without making any network call.

- **Location**: `~/.pokeplanner/cache/` (overridable with `--cache-dir`)
- **Layout**: one JSON file per resource, organized by category — e.g. `cache/pokemon/charizard.json`, `cache/type-chart/type-chart.json`, `cache/game-pokemon/red-blue.json`
- **TTL**: 1 year. PokéAPI data changes extremely rarely, so a long TTL is safe.
- **Corruption handling**: if a cached file can't be deserialized, it is logged, deleted, and treated as a cache miss

### Rate limiting

Even with caching, the first fetch of a new game's Pokémon pool can issue hundreds of requests (one per species + one per form). To stay respectful of PokéAPI's infrastructure:

- **Rate limiter**: a `governor`-based token bucket shared across all concurrent jobs and API handlers — default **20 requests/second** with a burst allowance of **5**
- **Concurrency**: outbound fetches run via `BufferedUnordered` with a cap of **10 concurrent requests**
- **Bulk population mode**: `cache populate all` uses a gentler profile — **3 concurrent requests** at **5 requests/second** — designed for pre-warming the entire cache without spiking load

### Cache management CLI

```bash
pokeplanner cache stats                   # Show entry counts, sizes, and location
pokeplanner cache populate game red-blue  # Pre-fetch all Pokémon for a game
pokeplanner cache populate all            # Pre-fetch everything (gentle rate limit)
pokeplanner cache clear stale             # Remove only expired entries
pokeplanner cache clear all               # Nuke the entire cache
```

### Bypass

Pass `--no-cache` (CLI), `?no_cache=true` (REST query param), or `no_cache: true` (gRPC field) to skip the cache and fetch fresh data. The fresh response is still written back to cache.

## Project Structure

| Crate | Description |
|-------|-------------|
| `pokeplanner-core` | Shared types — models, errors, job and team types |
| `pokeplanner-pokeapi` | PokéAPI HTTP client with disk caching and rate limiting |
| `pokeplanner-storage` | Storage trait + JSON file implementation |
| `pokeplanner-service` | Business logic, team planner, type chart, job orchestration |
| `pokeplanner-api-rest` | Axum REST server |
| `pokeplanner-api-grpc` | Tonic gRPC server |
| `pokeplanner-cli` | Clap CLI (`pokeplanner` binary) |

See `proto/pokeplanner.proto` for gRPC service definitions and the `docs/` directory for detailed architecture, dependency, and structure documentation.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — system design and data flow
- [Dependencies](docs/DEPENDENCIES.md) — dependency choices and rationale
- [Structure](docs/STRUCTURE.md) — repository layout

## License

This project is licensed under the [MIT License](LICENSE).
