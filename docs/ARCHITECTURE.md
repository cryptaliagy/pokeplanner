# PokePlanner Architecture

## Overview

PokePlanner is a Rust workspace organized into a layered architecture with clear separation of concerns. The system exposes both REST and gRPC APIs, backed by a shared service layer and pluggable storage. It integrates with [PokeAPI v2](https://pokeapi.co/api/v2/) to discover pokemon per game and compute optimal team compositions.

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
                │ + Team Planner  │
                │ + Type Chart    │
                └───┬─────────┬───┘
                    │         │
           ┌────────▼──┐  ┌──▼───────────┐
           │  Storage   │  │  PokeAPI     │
           │ (Trait)    │  │  Client      │
           └────────┬───┘  │ + HTTP + Cache│
                    │      │ + Rate Limit  │
           ┌────────▼───┐  └──────┬───────┘
           │ Core Types  │◄───────┘
           │(Models, Err)│
           └─────────────┘
```

## Data Flow

1. **Request** enters via REST, gRPC, or CLI
2. **API layer** deserializes and validates input, then delegates to the service
3. **Service** orchestrates business logic — fetching pokemon, filtering, planning teams
4. **PokeAPI Client** handles external HTTP calls with rate limiting and disk caching
5. **Storage** persists job state (currently JSON files, interface supports future SQL/NoSQL)
6. **Response** flows back up through the layers

## Job System

The job system supports long-running operations:
1. Client submits a job via `POST /jobs` (REST) or `SubmitJob` (gRPC) or CLI
2. Service creates a `Pending` job with a `JobKind` (Generic or TeamPlan), persists it, and returns the job ID immediately
3. A background task picks up the job, transitions it through `Running` -> `Completed`/`Failed`
4. Job `progress` field is updated during long operations (e.g., "Fetching pokemon 47/312")
5. Client polls for status via `GET /jobs/{id}` or `GetJob` or CLI `get-job`

## Team Planning Flow

1. User selects a source: game (version-group) or custom pokemon list
2. Service fetches candidate pokemon via PokeAPI (cached aggressively, 1-year TTL)
3. Optional BST filter reduces candidates
4. **Hybrid algorithm** selects optimal teams:
   - N ≤ 25: exact brute-force (provably optimal)
   - N > 25: greedy beam search (beam width 50, high-quality heuristic)
5. Score function: 40% offensive type coverage + 30% defensive score + 30% normalized BST
6. Returns top-K teams with type coverage analysis

## PokeAPI Navigation Chain

```
version-group/{name} → pokedexes[]
pokedex/{name} → pokemon_entries[] (species names)
pokemon-species/{name} → varieties[] (forms: base, mega, regional)
pokemon/{form_name} → stats[], types[]
```

## Caching Strategy

Two layers of caching in `data/cache/`:
- **Raw API responses**: `pokemon/`, `species/`, `pokedex/`, `version-group/`, `type/` — individual JSON files per resource
- **Aggregated results**: `game-pokemon/`, `type-chart/` — pre-computed for fast repeated access

All caches use 1-year TTL. Bypassed via `--no-cache` (CLI), `?no_cache=true` (REST), or `no_cache: true` (gRPC).

## Crate Dependency Graph

```
pokeplanner-api-rest ──┐
pokeplanner-api-grpc ──┼──► pokeplanner-service ──┬──► pokeplanner-storage ──► pokeplanner-core
pokeplanner-cli ───────┘                          └──► pokeplanner-pokeapi ──► pokeplanner-core
```

All crates depend on `pokeplanner-core` for shared types.
