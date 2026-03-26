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
                │ + Move Selector │
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
1. Client submits a job via `POST /jobs` (REST) or `SubmitJob`/`PlanTeam` (gRPC) or CLI
2. Service creates a `Pending` job with a `JobKind` (Generic or TeamPlan), persists it, and returns the job ID immediately
3. A background task picks up the job, transitions it through `Running` -> `Completed`/`Failed`. The outer function (`run_team_plan_job`) handles all state transitions via a single `match` on the inner function's `Result`, guaranteeing a terminal state on every exit path. The inner function (`execute_team_plan`) uses `?` for error propagation — adding new fallible steps doesn't require manual `fail_job` calls.
4. Job `progress` field is updated during long operations (e.g., "Fetching pokemon 47/312")
5. Client polls for status via `GET /jobs/{id}` or `GetJob` or CLI `get-job`

## Team Planning Flow

1. User selects a source: game (version-group), pokedex, or custom pokemon list
2. Service fetches candidate pokemon via PokeAPI (cached aggressively, 1-year TTL)
3. Filters reduce candidates: min BST, exclude by form/species, exclude variant types (e.g., mega, gmax, alola)
4. **Hybrid algorithm** selects optimal teams:
   - N ≤ 25: exact brute-force (provably optimal)
   - N > 25: greedy beam search (beam width 50, high-quality heuristic)
5. Score function: 40% offensive type coverage + 30% defensive score + 30% normalized BST
6. Returns top-K teams with type coverage analysis
7. **Move selection phase** (post-hoc): If a learnset version group is available, recommends 4 optimal moves per team member via `MoveSelector`. Uses a three-tier fallback chain per pokemon: (a) try requested version groups, (b) try sibling VGs in the same generation, (c) fetch all VG data and pick the most recent generation. When fallback is used, `TeamMember.learnset_source_vg` records which VG provided the data. Errors are non-fatal — members get `recommended_moves: None` on failure.
8. **Move coverage**: After move selection, the service sets `move_coverage: MoveCoverage` — a three-state enum: `NotAttempted` (move selection was skipped, e.g. Custom source), `Unavailable { version_groups }` (attempted but no learnset data found for any team member even after fallback), or `Available { types }` (the set of types hit SE by the team's recommended moves). The CLI displays a percentage summary for `Available`, a diagnostic note for `Unavailable`, and nothing for `NotAttempted`.

## PokeAPI Navigation Chain

```
version-group/{name} → pokedexes[]
pokedex/{name} → pokemon_entries[] (species names)
pokemon-species/{name} → varieties[] (forms: base, mega, regional)
pokemon/{form_name} → stats[], types[]
move/{name} → type, power, accuracy, pp, damage_class, meta, stat_changes
```

### Move metadata semantics

The `/move/{name}` endpoint returns two fields used for move safety filtering:

- **`meta.drain`** (i32): percentage of damage drained as HP. Negative = recoil (user loses HP, e.g. Flare Blitz: -33), positive = HP drain (e.g. Giga Drain: 50), 0 = neither.
- **`meta.stat_chance`** (i32): probability that `stat_changes` apply. **0 means guaranteed** (not "never") — this is PokeAPI's convention. Values 1–99 are probabilities; ≥100 is also guaranteed. For example, Overheat has `stat_chance: 0` with `stat_changes: [{change: -2, stat: "special-attack"}]`, meaning the SpAtk drop always occurs.
- **`stat_changes`** (array): top-level array of `{change: i32, stat: NamedApiResource}`. Only negative entries (debuffs) with guaranteed application are captured in the core `Move.self_stat_changes` field.

## Move Selection Algorithm

After the team planner selects a team composition, the `MoveSelector` recommends 4 optimal moves per team member. This is a post-hoc step — moves don't influence team scoring, keeping the planner fast.

### Filtering criteria
1. **Damaging only**: Moves must have `power > 0` (status moves excluded)
2. **Uniform damage class**: All moves match the pokemon's dominant offensive stat — physical if Attack ≥ Special Attack, special otherwise
3. **No recoil**: Moves with `drain < 0` are excluded (e.g. Flare Blitz, Brave Bird)
4. **No self-debuffs**: Moves with non-empty `self_stat_changes` are excluded (e.g. Overheat's SpAtk -2)
5. **Deduplication**: Same move learned via multiple methods (level-up + TM) appears once

### 2 STAB + 2 Coverage allocation
- **STAB moves** (2 slots): Moves matching the pokemon's own type(s). For dual-types, prefer one move of each type. Falls back to 2 of the same type if only one type has eligible STAB moves.
- **Coverage moves** (2 slots): Non-STAB moves selected by greedy set-cover over the pokemon's weaknesses.

### Greedy set-cover for coverage moves
1. For each candidate, compute which uncovered weakness types it hits super-effectively (≥2.0x)
2. Pick the move covering the most uncovered weaknesses (break ties by power)
3. Mark those weaknesses as covered
4. Repeat for remaining slots

### Mirror-match fallback
If all weaknesses are covered (or no coverage moves hit any weakness), remaining slots are filled with moves that are super-effective against the pokemon's own type(s). This helps in mirror matchups. If no mirror coverage is available, the highest-power remaining move is selected.

## Caching Strategy

Two layers of caching in `data/cache/`:
- **Raw API responses**: `pokemon/`, `species/`, `pokedex/`, `version-group/`, `type/` — individual JSON files per resource
- **Aggregated results**: `game-pokemon/`, `type-chart/` — pre-computed for fast repeated access

All caches use 1-year TTL. Bypassed via `--no-cache` (CLI), `?no_cache=true` (REST), or `no_cache: true` (gRPC).

The CLI provides a `cache` subcommand for cache management:
- `cache stats` — inspect cache entry counts, sizes, and location
- `cache populate` — pre-fetch data with reduced concurrency (3 concurrent, 5 req/s) to be gentle on the API
- `cache clear` — selectively or fully remove cached data (by game, pokedex, pokemon, type chart, or all/stale)

## Rate Limiting

PokeAPI is a free, no-auth public API. We are responsible consumers:

- **Default rate: 20 requests/second** with a burst allowance of 5. This is conservative — PokeAPI does not publish a hard limit, but sits behind Cloudflare which can throttle or block aggressive clients.
- **Configurable** via `PokeApiClientConfig` — binaries can adjust `requests_per_second`, `burst_size`, and `concurrent_requests`.
- **Single shared rate limiter**: All concurrent jobs and API handlers share one `Arc<PokeApiHttpClient>`, so the rate limit is global per process, not per-request or per-job. Two concurrent jobs each get roughly half the budget.
- **Concurrency cap**: Mass-fetch operations use `BufferedUnordered(N)` — at most N HTTP requests in flight at once per fetch operation (default 10, configurable via `concurrent_requests`). Combined with the rate limiter, this prevents connection storms. The `cache populate` CLI uses lower values (3 concurrent, 5 req/s) to be gentle on the API.
- **Aggressive caching eliminates repeat calls**: After the first cold-cache fetch, all subsequent requests for the same data are served from disk. The rate limiter only matters for cold-cache scenarios.

### Expected cold-cache times (national dex, 1028 species)

| Scenario | Requests | Time at 20 req/s |
|----------|----------|-------------------|
| Default forms only | ~2,057 | ~1.7 minutes |
| With mega/regional variants | ~2,430 | ~2 minutes |
| With movesets (future) | ~4,400 | ~3.5 minutes |

These are one-time costs. Subsequent calls are instant from cache.

## Observability

> For the complete observability reference (metrics catalog, tracing architecture, correlation model), see [OBSERVABILITY.md](OBSERVABILITY.md).
> For operational runbooks, see [ops/RUNBOOK.md](../ops/RUNBOOK.md). For Grafana dashboards, see [dashboards/](../dashboards/).

PokePlanner uses a three-pillar observability approach: structured logging, distributed tracing, and metrics — all built on the `tracing` + OpenTelemetry ecosystem.

### Telemetry initialization (`pokeplanner-telemetry`)

A shared crate centralizes subscriber setup across all three binaries:

- **Servers** (REST, gRPC): `init_server_telemetry(config)` builds a layered subscriber — `EnvFilter` + `fmt` (text or JSON) + optional OTEL trace layer. Returns a `TelemetryGuard` for graceful shutdown. OTEL export is gated behind `--otlp-endpoint` (or `OTEL_EXPORTER_OTLP_ENDPOINT` env var) — zero overhead when absent.
- **CLI**: `init_cli_telemetry(verbosity)` builds a simple fmt subscriber. `-v` enables info, `-vv` enables debug. No OTEL (CLI is short-lived).

### Configuration (REST/gRPC servers)

| Flag | Default | Description |
|------|---------|-------------|
| `--otlp-endpoint <url>` | (disabled) | OTLP gRPC endpoint for trace export |
| `--log-format text\|json` | `text` | Stdout log format |
| `--log-level <filter>` | `info` | Base log level (overridden by `RUST_LOG`) |

### Instrumentation points

- **HTTP/gRPC requests**: `tower-http::TraceLayer` provides automatic per-request spans with method, path/RPC, status, and latency
- **Job orchestration**: Team plan jobs run in an `info_span!("team_plan_job", %job_id)` with child spans per phase
- **Team planner**: `plan_teams` logs algorithm selection (exact/beam), candidate count, top_k
- **Move selector**: `select_moves` logs filtering decisions at debug level (rejection reasons: non-damaging, wrong class, recoil, self-debuff)
- **PokeAPI client**: `fetch()` logs cache hit/miss, URL, and elapsed time at debug level

### Metrics (`Metrics` struct)

When OTEL is enabled, an optional `Metrics` struct is injected into `PokePlannerService`. Instruments include:

- HTTP request counter and duration histogram
- PokeAPI request counter, duration, cache hit/miss counters
- Job submitted/completed/failed counters and duration histogram
- Team planner candidate pool size and plans generated counter

All metrics are no-ops when OTEL is not configured.

## Crate Dependency Graph

```
pokeplanner-api-rest ──┐
pokeplanner-api-grpc ──┼──► pokeplanner-service ──┬──► pokeplanner-storage ──► pokeplanner-core
pokeplanner-cli ───────┘          │               └──► pokeplanner-pokeapi ──► pokeplanner-core
                                  │
                    pokeplanner-telemetry (shared by all binaries + service)
```

All crates depend on `pokeplanner-core` for shared types.
