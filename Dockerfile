# ─── Stage 1: Rust + cargo-chef + system build dependencies ──────────────────
FROM lukemathwalker/cargo-chef:latest-rust-slim AS chef

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       protobuf-compiler \
       pkg-config \
       libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ─── Stage 2: Compute the dependency recipe ───────────────────────────────────
FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ─── Stage 3: Build & cache all dependencies, then build the binaries ─────────
FROM chef AS builder

ENV PROTOC=/usr/bin/protoc

# Restore dependency artifacts (re-run only when Cargo.toml / Cargo.lock change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy the full workspace source and proto definitions
COPY . .

# Build both API binaries in one pass (shares the already-compiled dependency layer)
RUN cargo build --release --bin pokeplanner-rest --bin pokeplanner-grpc

# ─── Stage 4: REST API production image ───────────────────────────────────────
FROM gcr.io/distroless/cc-debian12 AS rest

COPY --from=builder /app/target/release/pokeplanner-rest /pokeplanner-rest

EXPOSE 3000
ENTRYPOINT ["/pokeplanner-rest"]

# ─── Stage 5: gRPC API production image ───────────────────────────────────────
FROM gcr.io/distroless/cc-debian12 AS grpc

COPY --from=builder /app/target/release/pokeplanner-grpc /pokeplanner-grpc

EXPOSE 50051
ENTRYPOINT ["/pokeplanner-grpc"]
