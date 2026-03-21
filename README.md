# PokePlanner

A Rust workspace for planning and managing Pokémon-related operations, featuring REST and gRPC APIs, a CLI, and a pluggable storage layer.

## Quickstart

```bash
# Build
cargo build

# Run tests
cargo test

# CLI
cargo run -p pokeplanner-cli -- hello
cargo run -p pokeplanner-cli -- health

# REST API (port 3000)
cargo run -p pokeplanner-api-rest

# gRPC API (port 50051)
cargo run -p pokeplanner-api-grpc
```

## Project Structure

| Directory | Description |
|-----------|-------------|
| `crates/pokeplanner-core` | Shared types — models, errors, job types |
| `crates/pokeplanner-storage` | Storage trait + JSON file implementation |
| `crates/pokeplanner-service` | Core business logic and job orchestration |
| `crates/pokeplanner-api-rest` | Axum REST API |
| `crates/pokeplanner-api-grpc` | Tonic gRPC API |
| `crates/pokeplanner-cli` | Clap CLI |
| `proto/` | Protocol Buffer definitions |
| `docs/` | Architecture, dependency, and structure documentation |
| `tools/` | Additional tooling (placeholder) |
| `frontend/` | Future frontend (placeholder) |

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — System design and data flow
- [Dependencies](docs/DEPENDENCIES.md) — Dependency choices and rationale
- [Structure](docs/STRUCTURE.md) — Repository layout
- [Knowledge Map](AGENTS.md) — AI agent reference and key truths
