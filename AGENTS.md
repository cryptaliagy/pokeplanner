# PokePlanner Knowledge Map

## Key Truths

1. **Don't guess shapes, build types to enforce structures and use them across API boundaries.**
   - All data structures live in `pokeplanner-core` (`crates/pokeplanner-core/src/`)
   - Models (`model.rs`), errors (`error.rs`), and job types (`job.rs`) are the single source of truth
   - Both REST and gRPC handlers convert to/from these core types — never define ad-hoc structures in API layers

2. **Reference this knowledge map as the first order of research.**
   - Before reading code, check this file and `docs/` for answers
   - Only dive into source code if the knowledge map doesn't cover your question

3. **Focus on building intent documents while building any code.**
   - Every code change should be reflected in the relevant documentation
   - `docs/ARCHITECTURE.md` — system design and data flow
   - `docs/DEPENDENCIES.md` — dependency choices and rationale
   - `docs/STRUCTURE.md` — repository layout

4. **Triple helix: intent documents, tests, and functionality.**
   - All three must stay in sync. When changing code, update tests and docs together
   - Intent documents are a primary actor, not an afterthought

5. **Follow idiomatic Rust testing conventions.**
   - Unit tests go **inline** in the same file as the code they test, inside a `#[cfg(test)] mod tests { ... }` block at the bottom of the file
   - Never create separate `src/tests.rs` files — this splits tests from the code they cover and is not idiomatic Rust
   - Use `use super::*;` inside the test module to access the parent module's items
   - Integration tests (cross-crate, end-to-end) go in a top-level `tests/` directory per crate
   - Run all tests with `cargo test` from the workspace root

## Architecture Quick Reference

- **Core types**: `crates/pokeplanner-core/` — shared models, errors, job types
- **Storage**: `crates/pokeplanner-storage/` — `Storage` trait + `JsonFileStorage`
- **Service**: `crates/pokeplanner-service/` — business logic, job orchestration
- **REST API**: `crates/pokeplanner-api-rest/` — Axum server on port 3000
- **gRPC API**: `crates/pokeplanner-api-grpc/` — Tonic server on port 50051
- **CLI**: `crates/pokeplanner-cli/` — Clap CLI (`pokeplanner` binary)
- **Proto**: `proto/pokeplanner.proto` — gRPC service definitions

## Storage Interface

The `Storage` trait (`crates/pokeplanner-storage/src/traits.rs`) provides:
- `save_job`, `get_job`, `list_jobs`, `update_job`
- Currently implemented by `JsonFileStorage` (JSON files in `data/jobs/`)
- Designed for future swap to SQL or NoSQL — only implement the trait

## Job Lifecycle

`Pending` → `Running` → `Completed` | `Failed`

Jobs are submitted, assigned a UUID, and processed asynchronously via `tokio::spawn`.

## API Endpoints

### REST (port 3000)
| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/jobs` | Submit a new job |
| GET | `/jobs` | List all jobs |
| GET | `/jobs/{id}` | Get job by ID |

### gRPC (port 50051)
| RPC | Description |
|-----|-------------|
| `Health` | Health check |
| `Ping` | Echo ping/pong |
| `SubmitJob` | Submit a new job |
| `GetJob` | Get job by ID |
| `ListJobs` | List all jobs |

### CLI
| Command | Description |
|---------|-------------|
| `hello` | Hello world |
| `health` | Service health check |
| `submit-job` | Submit a new job |
| `get-job <id>` | Get job status |
| `list-jobs` | List all jobs |

## Build & Run

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests
cargo run -p pokeplanner-cli -- hello     # CLI hello world
cargo run -p pokeplanner-api-rest         # Start REST server
cargo run -p pokeplanner-api-grpc         # Start gRPC server
```
