# Contributing to PokePlanner

Thank you for your interest in PokePlanner! This is a personal learning project, but issues, bug reports, and pull requests are welcome.

## Getting Started

Set up your development environment by following [INSTALL.md](INSTALL.md). The short version:

```bash
# With Nix + direnv (recommended)
direnv allow       # first time only; auto-activates on every subsequent cd

# Without Nix
# Requires: Rust stable, protoc, just
cargo build
```

## Making Changes

### Branching

Create a topic branch from `main`:

```bash
git checkout -b your-branch-name
```

### Code Style

This project enforces formatting and linting via CI. Before committing, run:

```bash
just fmt          # auto-fix formatting
just ci           # format check + clippy + type-check + tests + release build
```

The `just ci` command runs these steps in order:

| Step | Command | Notes |
|------|---------|-------|
| `format` | `cargo fmt --all -- --check` | Unformatted code fails CI |
| `lint` | `cargo clippy --workspace --all-targets -- -D warnings` | Warnings are errors |
| `check` | `cargo check --workspace --all-targets` | Type-check without compiling |
| `test` | `cargo test --workspace` | All unit and integration tests |
| `build` | `cargo build --workspace --release` | Release binary compilation |

You can install a pre-commit hook that runs `format`, `lint`, and `check` automatically:

```bash
just install-hooks
```

### Testing

Run all tests from the workspace root:

```bash
cargo test
```

Run tests for a specific crate or test file:

```bash
cargo test -p pokeplanner-service
cargo test -p pokeplanner-pokeapi --test http_client_integration
cargo test -p pokeplanner-api-rest --test rest_api_integration
```

**Testing conventions:**

- Unit tests are **inline** in the same file, inside a `#[cfg(test)] mod tests { ... }` block at the bottom. Do not create separate `tests.rs` files.
- Integration tests (cross-crate or end-to-end) go in a top-level `tests/` directory inside the relevant crate.
- Use `use super::*;` inside test modules to access the parent module's items.

### Where Code Lives

| Crate | What to change |
|-------|---------------|
| `pokeplanner-core` | Shared types, models, errors |
| `pokeplanner-pokeapi` | PokeAPI HTTP client, caching, response parsing |
| `pokeplanner-storage` | Storage trait or `JsonFileStorage` |
| `pokeplanner-service` | Business logic, team planner, move selector, type chart |
| `pokeplanner-telemetry` | Observability init, metrics |
| `pokeplanner-api-rest` | REST route handlers |
| `pokeplanner-api-grpc` | gRPC handlers, proto definitions (`proto/pokeplanner.proto`) |
| `pokeplanner-cli` | CLI commands |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the system design and [docs/STRUCTURE.md](docs/STRUCTURE.md) for the full file-by-file layout.

### Keeping Docs in Sync

When you change code, update the relevant documentation:

- `docs/ARCHITECTURE.md` — system design and data flow
- `docs/DEPENDENCIES.md` — dependency choices and rationale
- `docs/STRUCTURE.md` — repository layout

## Submitting a Pull Request

1. Ensure `just ci` passes cleanly.
2. Write a clear PR description: what changed and why.
3. Keep commits focused — one logical change per commit.
4. Reference any related issue with `Fixes #N` or `Closes #N` in the PR description.

CI runs automatically on every pull request and checks format, clippy, tests, and build. PRs that fail CI will not be merged.

## Reporting Issues

Open a GitHub issue with:
- A clear description of the problem or feature request
- Steps to reproduce (for bugs)
- Expected vs. actual behaviour (for bugs)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
