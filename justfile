# Run all CI checks in sequence
ci: format lint check test build

# Format all code
format:
    cargo fmt --all -- --check

# Run clippy lints (warnings are errors)
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Type-check the workspace
check:
    cargo check --workspace --all-targets

# Run all tests
test:
    cargo test --workspace

# Build release binaries
build:
    cargo build --workspace --release

# Run security audit
audit:
    cargo audit

# Auto-fix formatting
fmt:
    cargo fmt --all
