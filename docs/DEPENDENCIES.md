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
| **reqwest** | 0.12 | HTTP client | De facto async HTTP client for Rust; used for PokeAPI requests with JSON deserialization |
| **governor** | 0.8 | Rate limiting | Token-bucket rate limiter for client-side outbound request throttling; prevents PokeAPI hammering |
| **futures** | 0.3 | Async utilities | `stream::BufferedUnordered` for concurrent pokemon fetching with bounded concurrency |
| **tower** | 0.5 | HTTP middleware | Shared middleware ecosystem between axum and tonic |

## Build Dependencies

| Dependency | Version | Purpose |
|---|---|---|
| **tonic-build** | 0.13 | Generates Rust code from .proto files at build time |

## Design Decisions

- **Axum over Actix-web**: Axum is built directly on tower and hyper, sharing the same ecosystem as tonic. This means middleware and extractors can be reused across REST and gRPC. Axum is also lighter and more composable.
- **Tonic for gRPC**: Tonic is the only mature gRPC framework in Rust and integrates seamlessly with the tokio ecosystem.
- **thiserror + anyhow**: `thiserror` for library crates (strongly typed errors), `anyhow` for the CLI binary (ergonomic error propagation).
- **Trait-based storage**: The `Storage` trait in `pokeplanner-storage` is designed to be implementation-agnostic. JSON file storage is the current implementation, but the interface supports future migration to SQL (e.g., sqlx) or NoSQL (e.g., MongoDB) without changing the service layer.
- **Native async traits over async-trait**: Both the `Storage` trait and `PokeApiClient` trait use native `impl Future` return types (Rust 1.75+) with explicit `+ Send` bounds instead of the `async-trait` crate. Combined with generics on `PokePlannerService<S: Storage, P: PokeApiClient>`, this avoids heap-allocated boxed futures.
- **reqwest over hyper directly**: reqwest provides a high-level API with JSON deserialization, connection pooling, and TLS out of the box. PokeAPI integration doesn't need low-level HTTP control.
- **governor over tower rate limiting**: Tower's rate limiting is designed for incoming request middleware. Governor provides client-side outgoing rate limiting with a clean async API (token-bucket algorithm), which is exactly what's needed for PokeAPI fair use compliance.
