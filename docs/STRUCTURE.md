# PokePlanner Repository Structure

```
pokeplanner/
├── Cargo.toml                  # Workspace root — defines members and shared dependencies
├── README.md                   # Project overview and quickstart
├── AGENTS.md                   # Knowledge map (primary reference for AI agents)
├── CLAUDE.md -> AGENTS.md      # Symlink to AGENTS.md
├── proto/
│   └── pokeplanner.proto       # Protocol Buffer definitions for gRPC
├── crates/
│   ├── pokeplanner-core/       # Shared types: models, errors, job types, team types
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs        # AppError enum (+ PokeApi, Cache variants)
│   │       ├── model.rs        # PokemonType, BaseStats, Pokemon, HealthResponse (+ inline tests)
│   │       ├── job.rs          # Job, JobStatus, JobKind, JobProgress, JobResult (+ inline tests)
│   │       └── team.rs         # TeamPlanRequest, TeamSource, TeamPlan, TypeCoverage, SortField (+ inline tests)
│   ├── pokeplanner-storage/    # Storage trait + JSON file implementation
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs       # Storage trait (async, Send+Sync)
│   │       └── json_store.rs   # JsonFileStorage implementation (+ inline tests)
│   ├── pokeplanner-pokeapi/    # PokeAPI v2 client with caching and rate limiting
│   │   ├── src/
│   │   │   ├── lib.rs          # Re-exports, VersionGroupInfo
│   │   │   ├── types.rs        # PokeAPI response deserialization types (+ inline tests)
│   │   │   ├── cache.rs        # DiskCache with 1-year TTL (+ inline tests)
│   │   │   ├── client.rs       # PokeApiHttpClient (HTTP + cache + rate limit, configurable base_url)
│   │   │   └── traits.rs       # PokeApiClient trait, TypeEffectivenessData
│   │   └── tests/
│   │       ├── http_client_integration.rs  # wiremock-based integration tests
│   │       └── fixtures/       # JSON fixture files (PokeAPI response shapes)
│   ├── pokeplanner-service/    # Core business logic
│   │   └── src/
│   │       ├── lib.rs          # PokePlannerService<S, P> (+ inline tests)
│   │       ├── type_chart.rs   # TypeChart: 18x18 effectiveness matrix (+ inline tests)
│   │       └── team_planner.rs # TeamPlanner: hybrid exact/beam search (+ inline tests)
│   ├── pokeplanner-api-rest/   # Axum REST API server
│   │   ├── src/
│   │   │   ├── lib.rs          # Router factory + route handlers (+ inline tests)
│   │   │   └── main.rs         # Server binary entry point
│   │   └── tests/
│   │       └── rest_api_integration.rs  # Integration tests with mock PokeAPI
│   ├── pokeplanner-api-grpc/   # Tonic gRPC API server
│   │   └── src/
│   │       ├── main.rs         # Server binary + gRPC handlers
│   │       └── build.rs        # Proto compilation
│   └── pokeplanner-cli/        # CLI application
│       └── src/
│           └── main.rs         # Clap-based CLI with team planning commands
├── data/
│   ├── jobs/                   # Job state persistence (JSON files)
│   └── cache/                  # PokeAPI response cache
│       ├── pokemon/            # Individual pokemon responses
│       ├── species/            # Species/varieties responses
│       ├── pokedex/            # Pokedex entries
│       ├── version-group/      # Version group data
│       ├── type/               # Type effectiveness data
│       ├── game-pokemon/       # Aggregated pokemon per game
│       └── type-chart/         # Computed type effectiveness matrix
├── docs/
│   ├── ARCHITECTURE.md         # System architecture and data flow
│   ├── DEPENDENCIES.md         # Dependency choices and rationale
│   └── STRUCTURE.md            # This file — repository layout
├── tools/                      # Placeholder for additional tooling
│   └── .gitkeep
└── frontend/                   # Placeholder for future frontend
    └── .gitkeep
```

## Orchestration

The repository is organized to support three concerns:

1. **Main application** (`crates/`): The core Rust workspace with service, APIs, and CLI
2. **Tooling** (`tools/`): Reserved for build scripts, code generation, or developer utilities
3. **Frontend** (`frontend/`): Reserved for a potential web frontend (TBD)
