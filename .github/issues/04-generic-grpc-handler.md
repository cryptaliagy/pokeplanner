# Make gRPC handler generic over Storage and PokeApiClient traits

## TL;DR

`GrpcHandler` is hardcoded to concrete types (`JsonFileStorage`, `PokeApiHttpClient`), unlike the REST handler which is fully generic. This prevents mock-based integration testing of the gRPC layer.

## Problem

In `crates/pokeplanner-api-grpc/src/main.rs:24-26`:

```rust
pub struct GrpcHandler {
    service: Arc<PokePlannerService<JsonFileStorage, PokeApiHttpClient>>,
}
```

The REST API's `create_router<S, P>()` is generic over both trait bounds, enabling integration tests with `MockPokeApi` via `tower::ServiceExt::oneshot()`. The gRPC handler hardwires concrete types, making it impossible to test without a real filesystem and HTTP server.

As a result, the REST layer has 6+ integration tests while the gRPC layer has **zero**.

## Acceptance Criteria

- [ ] `GrpcHandler` is generic: `GrpcHandler<S: Storage, P: PokeApiClient>`
- [ ] The `#[tonic::async_trait] impl GrpcService for GrpcHandler<S, P>` compiles and works
- [ ] `main()` still constructs the handler with concrete types (no runtime change)
- [ ] At least basic integration tests exist for key gRPC endpoints (health, get_pokemon, plan_team) using trait mocks
- [ ] `cargo test` passes

## Implementation Guidance

### Step 1: Make the handler generic

In `crates/pokeplanner-api-grpc/src/main.rs`, change:

```rust
pub struct GrpcHandler<S: Storage, P: PokeApiClient> {
    service: Arc<PokePlannerService<S, P>>,
}
```

Update the two `impl` blocks:
- `impl<S: Storage, P: PokeApiClient> GrpcHandler<S, P>` (helper methods block starting at line 28)
- `#[tonic::async_trait] impl<S: Storage, P: PokeApiClient> GrpcService for GrpcHandler<S, P>` (trait impl starting at line 112)

All helper methods (`pokemon_to_proto`, `coverage_to_proto`, `job_to_proto`, `app_error_to_status`) are already independent of the concrete types — they operate on core types. No changes needed to their bodies.

### Step 2: Update main()

The `main()` function (around line 310+) already creates concrete types. Update only the type annotation if needed:

```rust
let handler = GrpcHandler { service };
// Type inferred as GrpcHandler<JsonFileStorage, PokeApiHttpClient>
```

### Step 3: Split the file

Currently everything is in `main.rs` (~380 lines). Consider splitting into:
- `lib.rs` — `GrpcHandler<S, P>` + trait impl + helpers (so tests can import it)
- `main.rs` — just `main()` with concrete type wiring

This mirrors the REST crate's structure (`lib.rs` has `create_router`, `main.rs` has `main()`).

### Step 4: Add integration tests

Create `crates/pokeplanner-api-grpc/tests/grpc_integration.rs` or add `#[cfg(test)] mod tests` in `lib.rs`. Use the same `MockPokeApi` pattern from the REST tests or the service tests.

For testing gRPC without binding a port, you can call handler methods directly on `GrpcHandler` — they accept `Request<T>` and return `Result<Response<T>, Status>`, so you don't need a running server.

## Things to Note

- The `#[tonic::async_trait]` macro should handle generic impls, but verify it compiles. Tonic's codegen for the `GrpcService` trait may require `Send + Sync + 'static` bounds that are already on `Storage` and `PokeApiClient`.
- The proto module (`pub mod proto { tonic::include_proto!("pokeplanner"); }`) is generated at build time. It defines the `PokePlannerService` trait (aliased as `GrpcService`). Check that the generated trait doesn't have hidden `Self: Sized` or other bounds that conflict with generics.
- Proto conversion helpers (`pokemon_to_proto`, etc.) don't depend on `S` or `P` — they convert core types to proto types. They can stay as associated functions or be extracted to a `conversions.rs` module.
- The REST crate's `MockPokeApi` in its test file could potentially be extracted to a shared test utility crate to avoid duplicating mock implementations across REST and gRPC tests. This is optional but worth considering.
