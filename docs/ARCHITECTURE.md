# PokePlanner Architecture

## Overview

PokePlanner is a Rust workspace organized into a layered architecture with clear separation of concerns. The system exposes both REST and gRPC APIs, backed by a shared service layer and pluggable storage.

## Layers

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  REST API    │  │  gRPC API    │  │     CLI      │
│  (Axum)      │  │  (Tonic)     │  │   (Clap)     │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       └─────────────────┼─────────────────┘
                         │
                ┌────────▼────────┐
                │    Service      │
                │ (Business Logic)│
                └────────┬────────┘
                         │
                ┌────────▼────────┐
                │    Storage      │
                │  (Trait-based)  │
                └────────┬────────┘
                         │
                ┌────────▼────────┐
                │    Core Types   │
                │ (Models, Errors)│
                └─────────────────┘
```

## Data Flow

1. **Request** enters via REST, gRPC, or CLI
2. **API layer** deserializes and validates input, then delegates to the service
3. **Service** orchestrates business logic and interacts with storage
4. **Storage** persists and retrieves data (currently JSON files, interface supports future SQL/NoSQL)
5. **Response** flows back up through the layers

## Job System

The job system supports long-running operations:
1. Client submits a job via `POST /jobs` (REST) or `SubmitJob` (gRPC)
2. Service creates a `Pending` job, persists it, and returns the job ID immediately
3. A background task picks up the job, transitions it through `Running` -> `Completed`/`Failed`
4. Client polls for status via `GET /jobs/{id}` or `GetJob`

## Crate Dependency Graph

```
pokeplanner-api-rest ──┐
pokeplanner-api-grpc ──┼──► pokeplanner-service ──► pokeplanner-storage ──► pokeplanner-core
pokeplanner-cli ───────┘
```

All crates depend on `pokeplanner-core` for shared types.
