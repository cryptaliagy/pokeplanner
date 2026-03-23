# Grafana Dashboards

Pre-built Grafana dashboards for monitoring PokePlanner.

## Prerequisites

These dashboards assume the standard OTEL-to-Prometheus pipeline:

```
PokePlanner --OTLP gRPC--> OTel Collector --remote_write--> Prometheus
```

Start the server with OTEL export enabled:

```bash
cargo run -p pokeplanner-api-rest -- --otlp-endpoint http://localhost:4317
```

## Importing

1. Open Grafana UI
2. Go to **Dashboards > Import**
3. Upload the JSON file (or paste its contents)
4. Select your Prometheus datasource when prompted

Each dashboard uses a `${DS_PROMETHEUS}` template variable. On import, Grafana will ask you to bind it to a datasource.

## Available Dashboards

| File | Description |
|---|---|
| `overview.json` | Service health: request rate, latency, job throughput, failure rate |
| `pokeapi.json` | Upstream dependency: PokeAPI request rate, latency, cache hit ratio |
| `jobs.json` | Job processing: submit/complete/fail rates, duration, candidate pool, move fallbacks |

## Metric Name Mapping

OpenTelemetry uses dot-separated metric names. Prometheus converts them:

| OTel Name | Prometheus Name |
|---|---|
| `http.server.request.count` | `http_server_request_count_total` |
| `http.server.request.duration` | `http_server_request_duration_seconds_bucket` |
| `pokeapi.request.count` | `pokeapi_request_count_total` |
| `pokeapi.request.duration` | `pokeapi_request_duration_seconds_bucket` |
| `pokeapi.cache.hit` | `pokeapi_cache_hit_total` |
| `pokeapi.cache.miss` | `pokeapi_cache_miss_total` |
| `job.submitted` | `job_submitted_total` |
| `job.completed` | `job_completed_total` |
| `job.failed` | `job_failed_total` |
| `job.duration` | `job_duration_seconds_bucket` |
| `team.candidate_pool_size` | `team_candidate_pool_size_bucket` |
| `team.plans_generated` | `team_plans_generated_total` |
| `move_selection.fallback` | `move_selection_fallback_total` |

See `docs/OBSERVABILITY.md` for complete metric documentation.
