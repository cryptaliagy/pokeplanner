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
│   │       ├── model.rs        # PokemonType, BaseStats, Pokemon, Move, MoveStatChange, HealthResponse (+ inline tests)
│   │       ├── job.rs          # Job, JobStatus, JobKind, JobProgress, JobResult (+ inline tests)
│   │       └── team.rs         # TeamPlanRequest, TeamSource, TeamPlan, TeamMember, RecommendedMove, MoveRole, TypeCoverage, SortField (+ inline tests)
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
│   ├── pokeplanner-cli/        # CLI application
│   │   └── src/
│   │       └── main.rs         # Clap-based CLI with team planning commands
│   └── pokeplanner-telemetry/  # Shared observability initialization
│       └── src/
│           ├── lib.rs          # Subscriber init (server + CLI), TelemetryGuard, OTEL setup
│           └── metrics.rs      # Metrics struct with OTEL counters and histograms
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
│   ├── CACHING.md              # Caching strategy, rate limiting, cache CLI
│   ├── COST_FUNCTION.md        # Team scoring algorithm (offensive + defensive + BST)
│   ├── DEPENDENCIES.md         # Dependency choices and rationale
│   ├── FAQ.md                  # Frequently asked questions
│   ├── IMPLEMENTATION_CHECKLIST.md  # Implementation progress tracker
│   ├── OBSERVABILITY.md        # Observability reference: metrics, tracing, logging, correlation
│   └── STRUCTURE.md            # This file — repository layout
├── ops/
│   └── RUNBOOK.md              # Operational runbook for production incident diagnosis
├── dashboards/
│   ├── README.md               # Dashboard import instructions and metric name mapping
│   ├── overview.json           # Grafana: service health (request rate, latency, job throughput)
│   ├── pokeapi.json            # Grafana: upstream PokeAPI health (latency, cache hit ratio)
│   └── jobs.json               # Grafana: job processing (duration, candidate pool, fallbacks)
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
