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

# Install git pre-commit hook
install-hooks:
    @echo '#!/usr/bin/env bash' > .git/hooks/pre-commit
    @echo 'set -euo pipefail' >> .git/hooks/pre-commit
    @echo '' >> .git/hooks/pre-commit
    @echo '# Run fast pre-commit checks via just' >> .git/hooks/pre-commit
    @echo 'echo "Running pre-commit checks..."' >> .git/hooks/pre-commit
    @echo 'just format' >> .git/hooks/pre-commit
    @echo 'just lint' >> .git/hooks/pre-commit
    @echo 'just check' >> .git/hooks/pre-commit
    @chmod +x .git/hooks/pre-commit
    @echo "Pre-commit hook installed (.git/hooks/pre-commit)"

# Uninstall git pre-commit hook
uninstall-hooks:
    @rm -f .git/hooks/pre-commit
    @echo "Pre-commit hook removed"

# Build Docker image for the REST API
docker-rest:
    docker build --target rest -t pokeplanner-rest .

# Build Docker image for the gRPC API
docker-grpc:
    docker build --target grpc -t pokeplanner-grpc .

# Build Docker images for both API services
docker-build: docker-rest docker-grpc
