# PokePlanner Dependencies

## Runtime Dependencies

| Dependency | Version | Purpose | Why Chosen |
|---|---|---|---|
| **tokio** | 1.x | Async runtime | Industry standard for async Rust; full-featured with timers, IO, sync primitives |
| **axum** | 0.8 | REST API framework | Built on tower/hyper, ergonomic extractors, excellent performance, first-class tokio integration |
| **tonic** | 0.13 | gRPC framework | The de facto Rust gRPC library, built on hyper/tower, pairs naturally with axum |
| **prost** | 0.13 | Protobuf serialization | Standard protobuf implementation for Rust, used by tonic |
| **clap** | 4.x | CLI argument parsing | Feature-rich, derive macro support, widely adopted |
| **serde** | 1.x | Serialization framework | Universal Rust serialization; derive macros for zero-boilerplate |
| **serde_json** | 1.x | JSON serialization | Standard JSON implementation for serde |
| **thiserror** | 2.x | Error types | Derive macro for `std::error::Error`; clean, zero-cost error enums |
| **anyhow** | 1.x | Error handling (CLI) | Ergonomic error handling for application code (CLI binary) |
| **uuid** | 1.x | Unique identifiers | RFC-compliant UUIDs; v4 for random IDs, serde support |
| **chrono** | 0.4 | Date/time handling | Full-featured datetime library with serde and timezone support |
| **tracing** | 0.1 | Structured logging | Async-aware, structured, composable instrumentation |
| **tracing-subscriber** | 0.3 | Log output | Configurable subscribers with env-filter for log levels |
## Build Dependencies

| Dependency | Version | Purpose |
|---|---|---|
| **tonic-build** | 0.13 | Generates Rust code from .proto files at build time |

## Design Decisions

- **Axum over Actix-web**: Axum is built directly on tower and hyper, sharing the same ecosystem as tonic. This means middleware and extractors can be reused across REST and gRPC. Axum is also lighter and more composable.
- **Tonic for gRPC**: Tonic is the only mature gRPC framework in Rust and integrates seamlessly with the tokio ecosystem.
- **thiserror + anyhow**: `thiserror` for library crates (strongly typed errors), `anyhow` for the CLI binary (ergonomic error propagation).
- **Trait-based storage**: The `Storage` trait in `pokeplanner-storage` is designed to be implementation-agnostic. JSON file storage is the current implementation, but the interface supports future migration to SQL (e.g., sqlx) or NoSQL (e.g., MongoDB) without changing the service layer.
- **Native async traits over async-trait**: The `Storage` trait uses native `impl Future` return types (Rust 1.75+) with explicit `+ Send` bounds instead of the `async-trait` crate. Combined with generics on `PokePlannerService<S: Storage>`, this avoids heap-allocated boxed futures and eliminates the `async-trait` dependency.
