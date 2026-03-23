# Observability

PokePlanner uses a three-pillar observability approach — structured logging, distributed tracing, and metrics — built on the `tracing` + OpenTelemetry ecosystem. Yes, a pokemon team planner has distributed tracing. The stack is deliberately over-built for the problem size because setting it up was educational, and this document exists so it doesn't go to waste.

## Pipeline

```
┌──────────────────────────────────────────────────┐
│  PokePlanner (REST / gRPC / CLI)                 │
│                                                  │
│  tracing spans + structured logs + OTel metrics  │
└───────────────┬──────────────────────────────────┘
                │ OTLP gRPC (batch)
                ▼
       ┌────────────────┐
       │  OTel Collector │
       └───┬────────┬───┘
           │        │
           ▼        ▼
     ┌──────────┐ ┌────────┐
     │Prometheus│ │ Jaeger │
     │(metrics) │ │(traces)│
     └──────────┘ └────────┘
```

- **Servers** (REST, gRPC) export traces and metrics via OTLP when `--otlp-endpoint` is set. Zero overhead when absent.
- **CLI** uses a simple `fmt` subscriber. No OTEL export (short-lived process).
- Logs always go to stdout (text or JSON format).

## Metrics Reference

All metrics are defined in `crates/pokeplanner-telemetry/src/metrics.rs` and created from an OpenTelemetry `Meter` named `"pokeplanner"`. Instruments are no-ops when the global meter provider is not configured.

### HTTP/gRPC Request Metrics

| Metric Name | OTel Type | Unit | Recorded In | Description |
|---|---|---|---|---|
| `http.server.request.count` | Counter\<u64\> | — | REST middleware (`api-rest/src/lib.rs`), gRPC handler (`api-grpc/src/main.rs`) | Total inbound requests. Incremented once per request/RPC call. |
| `http.server.request.duration` | Histogram\<f64\> | seconds | REST middleware, gRPC handler | Wall-clock time from request receipt to response send. |

### PokeAPI Client Metrics

| Metric Name | OTel Type | Unit | Recorded In | Description |
|---|---|---|---|---|
| `pokeapi.request.count` | Counter\<u64\> | — | `pokeapi/src/client.rs` `fetch()` | Total HTTP requests sent to the upstream PokeAPI. Only counts actual network calls (cache misses). |
| `pokeapi.request.duration` | Histogram\<f64\> | seconds | `pokeapi/src/client.rs` `fetch()` | Latency of upstream PokeAPI HTTP calls. Measured from after rate-limiter wait to response parse completion. |
| `pokeapi.cache.hit` | Counter\<u64\> | — | `pokeapi/src/client.rs` `fetch()` | Disk cache hits. A high ratio indicates warm cache and minimal upstream load. |
| `pokeapi.cache.miss` | Counter\<u64\> | — | `pokeapi/src/client.rs` `fetch()` | Disk cache misses. Each miss triggers an upstream HTTP request subject to rate limiting. |

### Job Metrics

| Metric Name | OTel Type | Unit | Recorded In | Description |
|---|---|---|---|---|
| `job.submitted` | Counter\<u64\> | — | `service/src/lib.rs` `submit_team_plan()` | Team planning jobs entering the system. |
| `job.completed` | Counter\<u64\> | — | `service/src/lib.rs` `run_team_plan_job()` | Jobs finishing successfully. |
| `job.failed` | Counter\<u64\> | — | `service/src/lib.rs` `fail_job()` | Jobs that errored out. Common causes: empty candidate pool, upstream fetch failure. |
| `job.duration` | Histogram\<f64\> | seconds | `service/src/lib.rs` `run_team_plan_job()`, `fail_job()` | Wall-clock time from job start to completion or failure. |

### Team Planner Metrics

| Metric Name | OTel Type | Unit | Recorded In | Description |
|---|---|---|---|---|
| `team.candidate_pool_size` | Histogram\<u64\> | — | `service/src/lib.rs` `run_team_plan_job()` | Number of pokemon remaining after BST/exclusion filtering. Determines algorithm selection: ≤25 triggers exact solver, >25 triggers beam search. |
| `team.plans_generated` | Counter\<u64\> | — | `service/src/lib.rs` `run_team_plan_job()` | Total team plans produced. Each completed job generates `top_k` plans (default 5). |
| `move_selection.fallback` | Counter\<u64\> | — | `service/src/lib.rs` `fetch_learnset_and_select()` | Learnset version group fallback events. Incremented when a pokemon's moves are sourced from a different VG than requested. High rates suggest requested VGs lack learnset data. |

### Notes

- All metrics are currently recorded with empty attributes (`&[]`). There is no dimension breakdown by endpoint, status code, game, or pokemon.
- Prometheus translation: OTel dot-separated names become underscores, counters get `_total` suffix, histograms get `_bucket`/`_sum`/`_count`, and the unit `s` appends `_seconds`. Example: `http.server.request.duration` → `http_server_request_duration_seconds_bucket`.

## Tracing Architecture

### Span Hierarchy

```
HTTP request (tower-http TraceLayer)
└── team_plan_job (job_id=<uuid>)            # info_span, service/lib.rs
    └── plan_teams (candidate_count, top_k)  # info_span, team_planner.rs
```

- **TraceLayer** (tower-http): Automatically creates a span for every HTTP/gRPC request. Captures method, path/RPC, response status, and latency. Applied via `.layer(TraceLayer::new_for_http())` (REST) and `.layer(TraceLayer::new_for_grpc())` (gRPC).
- **Job spans**: `info_span!("team_plan_job", %job_id)` wraps each spawned job task via `.instrument(span)`, propagating trace context across `tokio::spawn` boundaries.
- **Planner spans**: `info_span!("plan_teams", candidate_count, top_k)` captures algorithm-level context.

### OTEL Export

- **Exporter**: OTLP gRPC via `opentelemetry-otlp` with `tonic` transport.
- **Batching**: `SdkTracerProvider` with `with_batch_exporter()`. Spans are batched and flushed periodically.
- **Tracer name**: `"pokeplanner"`.
- **Shutdown**: `TelemetryGuard` returned from `init_server_telemetry()` calls `provider.shutdown()` on drop, flushing pending spans. Must be held until server shutdown.
- **Graceful degradation**: If OTEL initialization fails, an error is logged to stderr and the server continues without trace export.

### CLI

The CLI (`pokeplanner-cli`) uses `init_cli_telemetry(verbosity)` — a simple `fmt` subscriber with no OTEL. Short-lived CLI invocations don't benefit from distributed tracing.

## Logging

### Levels

| Level | What Gets Logged |
|---|---|
| `warn` | Fetch failures with fallback, learnset unavailable, cache write errors, cache corruption |
| `info` | Server start/shutdown, job lifecycle (submitted/completed), learnset fallback VG selection, algorithm selection (exact vs beam) |
| `debug` | Per-resource cache hit/miss with URL, filtering decisions (candidate counts), move rejection reasons (recoil, self-debuff, wrong damage class), API call details |

### Formats

- **Text** (default): Human-readable `tracing_subscriber::fmt` output. Good for development.
- **JSON**: Machine-parseable structured logs via `fmt::layer().json()`. Each log line is a JSON object with `timestamp`, `level`, `target`, `span`, `fields`. Good for log aggregation (Loki, CloudWatch, etc.).

### Filtering

- `--log-level <filter>` sets the base level (default: `info`). Supports target-level filters like `pokeplanner=debug,info`.
- `RUST_LOG` environment variable overrides `--log-level` when set. Uses `EnvFilter` syntax.
- CLI uses verbosity flags: default → `warn`, `-v` → `info`, `-vv` → `debug`.

## Correlation Model

### By Job ID

The primary correlation key is `job_id` (UUID). It appears as a structured field in the `team_plan_job` span, which means:

- **In Jaeger**: Search for traces by tag `job_id=<uuid>`. All spans within the job (planning, move selection, API calls) appear as children.
- **In JSON logs**: Filter by `"job_id":"<uuid>"` in the span context. All log events within the instrumented task include the span's fields.

### By Trace ID

OTEL assigns a unique trace ID to each incoming request. TraceLayer propagates W3C Trace Context headers, so:

- An HTTP request → team plan submission → spawned job task all share the same trace ID.
- Useful for correlating the initial API call with the background job it spawned.

### Current Gaps

- No explicit `request_id` header — correlation relies on OTEL trace IDs (only available when OTEL is enabled).
- No attributes on metrics — impossible to break down request duration by endpoint or job duration by game.

## Configuration Reference

### Server Flags (REST and gRPC)

| Flag | Env Var | Default | Description |
|---|---|---|---|
| `--otlp-endpoint <url>` | `OTEL_EXPORTER_OTLP_ENDPOINT` | (disabled) | OTLP gRPC endpoint for trace export. OTEL disabled when absent. |
| `--log-format text\|json` | — | `text` | Stdout log format. |
| `--log-level <filter>` | `RUST_LOG` (overrides) | `info` | Base log level. Supports `EnvFilter` syntax. |

### CLI Flags

| Flag | Default | Description |
|---|---|---|
| `-v` | — | Enable `info` level logging. |
| `-vv` | — | Enable `debug` level logging. |

### Examples

```bash
# REST server with OTEL export and JSON logs
cargo run -p pokeplanner-api-rest -- \
  --otlp-endpoint http://localhost:4317 \
  --log-format json \
  --log-level debug

# gRPC server with default settings (no OTEL, text logs, info level)
cargo run -p pokeplanner-api-grpc

# CLI with debug output
cargo run -p pokeplanner-cli -- -vv plan-team --game red-blue

# Override log level via environment
RUST_LOG=pokeplanner=debug,info cargo run -p pokeplanner-api-rest
```
