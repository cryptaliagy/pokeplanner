# Improve error handling: preserve source errors and log storage write failures

## TL;DR

`AppError` flattens all source errors into `String`, losing type information. Additionally, 7 storage writes silently discard errors via `let _ =`. Fix both to improve debuggability and observability.

## Problem

### Stringly-typed errors

Every `AppError` variant wraps a `String`:

```rust
pub enum AppError {
    Storage(String),
    Internal(String),
    PokeApi(String),
    Cache(String),
}
```

There are ~20 call sites across 4 files that construct these with `format!("context: {e}")`, discarding the original `reqwest::Error`, `serde_json::Error`, or `std::io::Error`. This makes programmatic error inspection impossible — you can't retry on network errors vs. parse errors, and backtraces are lost.

### Silent storage write failures

In `crates/pokeplanner-service/src/lib.rs`, 7 occurrences of `let _ = storage.update_job(&job).await;` (lines 70, 80, 246, 314, 361, 408, 464) silently discard storage errors during job state transitions. If the filesystem fills up or permissions change, jobs appear permanently stuck with no log evidence.

## Acceptance Criteria

- [ ] `AppError` variants preserve source error types for at least IO, HTTP, and serialization errors
- [ ] `AppError` remains usable with `thiserror` and has meaningful `Display` output
- [ ] All 7 `let _ = storage.update_job(...)` calls are replaced with `if let Err` + `warn!` logging
- [ ] Error-to-HTTP-status mapping in REST (`crates/pokeplanner-api-rest/src/lib.rs`) still works correctly
- [ ] Error-to-gRPC-status mapping in gRPC (`crates/pokeplanner-api-grpc/src/main.rs`) still works correctly
- [ ] `cargo test` passes with no regressions

## Implementation Guidance

### Phase 1: Fix silent storage writes (trivial, do this first)

In `crates/pokeplanner-service/src/lib.rs`, replace all 7 instances of:
```rust
let _ = storage.update_job(&job).await;
```
with:
```rust
if let Err(e) = storage.update_job(&job).await {
    warn!(job_id = %job.id, "Failed to persist job update: {e}");
}
```

These are at lines 70, 80, 246, 314, 361, 408, and 464 (the last one in `fail_job()`).

### Phase 2: Restructure AppError

In `crates/pokeplanner-core/src/error.rs`, add typed source variants:

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("IO error: {context}")]
    Io { context: String, #[source] source: std::io::Error },

    #[error("Serialization error: {context}")]
    Serialization { context: String, #[source] source: serde_json::Error },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("PokeAPI error: {0}")]
    PokeApi(String),
}
```

Then update the ~20 construction sites:

- **`crates/pokeplanner-storage/src/json_store.rs`** (9 sites): Replace `AppError::Storage(format!("Write error: {e}"))` with `AppError::Io { context: "writing job file".into(), source: e }`
- **`crates/pokeplanner-pokeapi/src/cache.rs`** (7 sites): Same IO/serde pattern
- **`crates/pokeplanner-pokeapi/src/client.rs`** (4 sites): Use `#[from]` for `reqwest::Error`, structured variants for serde errors

### Phase 3: Update error mapping

- **REST** (`crates/pokeplanner-api-rest/src/lib.rs`): The `error_response()` function matches on `AppError` variants to produce HTTP status codes. Add arms for new variants (all map to 500 except `NotFound`/`JobNotFound`).
- **gRPC** (`crates/pokeplanner-api-grpc/src/main.rs`): The `app_error_to_status()` function does the same. Add arms mapping new variants to `Status::internal()`.

## Things to Note

- Adding `reqwest::Error` and `std::io::Error` as sources makes `AppError` non-`Clone`. Currently `AppError` only derives `Debug` and `Error`, so this should be fine.
- The `pokeplanner-core` crate would gain a dependency on `reqwest` if using `#[from]`. To avoid core depending on `reqwest`, keep `PokeApi(String)` for HTTP-status errors and only use `#[from]` in the pokeapi crate's own error type, converting at the boundary. Alternatively, keep `PokeApi(String)` as-is — the HTTP client already handles reqwest errors locally.
- The `Cache(String)` variant can be merged into `Io`/`Serialization` since cache errors are always one of those two categories.
