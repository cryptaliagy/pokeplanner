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
│   ├── pokeplanner-core/       # Shared types: models, errors, job types
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs        # AppError enum
│   │       ├── model.rs        # Pokemon, HealthResponse
│   │       ├── job.rs          # Job, JobStatus, JobResult
│   │       └── tests.rs
│   ├── pokeplanner-storage/    # Storage trait + JSON file implementation
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs       # Storage trait (async, Send+Sync)
│   │       ├── json_store.rs   # JsonFileStorage implementation
│   │       └── tests.rs
│   ├── pokeplanner-service/    # Core business logic
│   │   └── src/
│   │       ├── lib.rs          # PokePlannerService
│   │       └── tests.rs
│   ├── pokeplanner-api-rest/   # Axum REST API server
│   │   └── src/
│   │       ├── main.rs         # Server binary + route handlers
│   │       └── tests.rs
│   ├── pokeplanner-api-grpc/   # Tonic gRPC API server
│   │   └── src/
│   │       ├── main.rs         # Server binary + gRPC handlers
│   │       └── build.rs        # Proto compilation
│   └── pokeplanner-cli/        # CLI application
│       └── src/
│           └── main.rs         # Clap-based CLI
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
