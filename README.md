# PokePlanner

A competitive Pokemon team planner built in Rust. Given a game (or set of games), PokePlanner fetches every available Pokemon from [PokeAPI](https://pokeapi.co), scores them on offensive coverage, defensive resilience, and base stats, and returns optimally balanced teams.

## Key Features

- **Optimal team planning** -- brute-force search for small pools (N<=25), greedy beam search for larger ones. Configurable `top_k` results.
- **Move selection** -- recommends 4 moves per team member: 2 STAB + 2 coverage, filtered for safety (no recoil, no self-debuff). Generation-aware learnset fallback when data is sparse.
- **Counter-team analysis** -- supply an opponent's team and PokePlanner finds the best response.
- **Type coverage analysis** -- evaluate offensive and defensive type matchups for any team composition.
- **Multi-game support** -- combine Pokemon pools across games (e.g., `--game red-blue,gold-silver`).
- **Three interfaces** -- CLI, REST API, and gRPC API, all backed by the same service layer.
- **Aggressive caching** -- disk-cached PokeAPI responses with 1-year TTL, rate limiting, and a CLI for cache management.
- **Observability** -- structured logging, distributed tracing (OpenTelemetry), and 13 metrics instruments. Optional OTLP export.

## Navigation

| Document | What's in it |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | System design, data flow, crate dependency graph |
| [Observability](docs/OBSERVABILITY.md) | Metrics catalog, tracing, logging, correlation model |
| [Cost Function](docs/COST_FUNCTION.md) | Team scoring algorithm (offensive + defensive + BST) |
| [Caching](docs/CACHING.md) | How caching works, rate limiting, cache CLI, bypass |
| [Dependencies](docs/DEPENDENCIES.md) | Dependency choices and rationale |
| [Structure](docs/STRUCTURE.md) | Repository layout, file-by-file |
| [FAQ](docs/FAQ.md) | Why three APIs? How does the planner work? And more |
| [Runbook](ops/RUNBOOK.md) | Production incident diagnosis playbook |
| [Dashboards](dashboards/README.md) | Grafana dashboard import and metric mapping |
| [Contributing](CONTRIBUTING.md) | How to contribute, dev setup, testing, PR workflow |

## Quickstart

### With Nix (recommended)

PokePlanner includes a Nix flake with all dependencies (Rust toolchain, protobuf, just, etc.):

```bash
# Enter the dev shell (or use direnv with the included .envrc)
nix develop

# Run CI checks
just ci

# Plan a team
cargo run -p pokeplanner-cli -- plan-team --game red-blue
```

### Without Nix

Requires: Rust stable, `protoc` (protobuf compiler), and `just` (command runner).

```bash
cargo build
cargo test
```

### CLI Examples

```bash
# Browse games and Pokemon
cargo run -p pokeplanner-cli -- list-games
cargo run -p pokeplanner-cli -- game-pokemon red-blue --min-bst 400
cargo run -p pokeplanner-cli -- pokemon show charizard --show-learnset

# Search Pokemon by type, stats, name
cargo run -p pokeplanner-cli -- pokemon search --type fire --min-bst 500

# Browse and search moves
cargo run -p pokeplanner-cli -- moves show flamethrower
cargo run -p pokeplanner-cli -- moves search charizard --game red-blue --type fire

# Plan teams
cargo run -p pokeplanner-cli -- plan-team --game red-blue
cargo run -p pokeplanner-cli -- plan-team --game red-blue,gold-silver --min-bst 400
cargo run -p pokeplanner-cli -- plan-team --game sword-shield --learnset-game sword-shield

# Analyze an existing team
cargo run -p pokeplanner-cli -- analyze-team charizard,blastoise,venusaur

# Manage unusable list
cargo run -p pokeplanner-cli -- unusable add magikarp,metapod
cargo run -p pokeplanner-cli -- plan-team --game red-blue  # excludes magikarp and metapod

# Cache management
cargo run -p pokeplanner-cli -- cache stats
cargo run -p pokeplanner-cli -- cache populate game red-blue
cargo run -p pokeplanner-cli -- cache populate all
```

### Start the APIs

```bash
# REST API (default port 3000)
cargo run -p pokeplanner-api-rest

# gRPC API (default port 50051)
cargo run -p pokeplanner-api-grpc

# With observability export
cargo run -p pokeplanner-api-rest -- --otlp-endpoint http://localhost:4317 --log-format json
```

## Project Structure

| Crate | Description |
|---|---|
| `pokeplanner-core` | Shared types -- models, errors, job and team types |
| `pokeplanner-pokeapi` | PokeAPI HTTP client with disk caching and rate limiting |
| `pokeplanner-storage` | Storage trait + JSON file implementation |
| `pokeplanner-service` | Business logic, team planner, move selector, type chart |
| `pokeplanner-telemetry` | Shared observability initialization (tracing, OTEL, metrics) |
| `pokeplanner-api-rest` | Axum REST server (port 3000) |
| `pokeplanner-api-grpc` | Tonic gRPC server (port 50051) |
| `pokeplanner-cli` | Clap CLI (`pokeplanner` binary) |

See [docs/STRUCTURE.md](docs/STRUCTURE.md) for the full file-by-file layout.

## Development

```bash
just ci          # Run all checks (format, lint, check, test, build)
just fmt         # Auto-fix formatting
just test        # Run tests only
just audit       # Security audit dependencies
just install-hooks  # Install pre-commit hook (format + lint + check)
```

## License

This project is licensed under the [MIT License](LICENSE).
