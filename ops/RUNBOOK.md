# PokePlanner Operational Runbook

Diagnostic playbook for production incidents. Each scenario describes symptoms, what to check, relevant queries, and resolution steps.

For the full observability reference, see [docs/OBSERVABILITY.md](../docs/OBSERVABILITY.md).
For dashboard import instructions, see [dashboards/README.md](../dashboards/README.md).

---

## Scenario 1: High Request Latency

### Symptoms

- Slow API responses reported by users or monitoring
- p95/p99 latency rising on the Overview dashboard

### Investigation

1. **Open**: Overview dashboard > **"Request Duration (p50 / p95 / p99)"** panel

   This shows whether latency is elevated across all percentiles (systemic issue) or only at the tail (specific slow requests).

   ```promql
   histogram_quantile(0.95, rate(http_server_request_duration_seconds_bucket[5m]))
   ```

2. **Check upstream**: PokeAPI dashboard > **"PokeAPI Request Duration (p50 / p95 / p99)"** panel

   If PokeAPI latency correlates with service latency, the bottleneck is upstream.

   ```promql
   histogram_quantile(0.95, rate(pokeapi_request_duration_seconds_bucket[5m]))
   ```

3. **Check cache**: PokeAPI dashboard > **"Cache Hit Ratio"** panel

   Low cache hit ratio means more upstream calls and higher latency. Expected >95% in steady state.

   ```promql
   rate(pokeapi_cache_hit_total[5m]) / (rate(pokeapi_cache_hit_total[5m]) + rate(pokeapi_cache_miss_total[5m]))
   ```

4. **Trace slow requests**: Search Jaeger for traces with duration > threshold. Look for long spans in the `team_plan_job` span tree.

### Log queries (JSON format)

```bash
# Find slow requests (look for high elapsed_ms values)
grep '"elapsed_ms"' logs.json | jq 'select(.fields.elapsed_ms > 5000)'

# Find upstream API calls
grep '"fetching from API"' logs.json
```

### Resolution

| Cause | Fix |
|---|---|
| Cold cache | Pre-warm with `pokeplanner cache populate all` |
| PokeAPI degradation | Check [pokeapi.co status](https://pokeapi.co/), wait for recovery |
| Large candidate pool (beam search) | Increase `min_bst` filter or narrow game selection |
| Rate limiter saturation | Cache is cold and too many concurrent requests. Pre-warm cache. |

---

## Scenario 2: Job Failures

### Symptoms

- `job.failed` counter incrementing
- Users reporting failed team plan requests
- Overview dashboard > **"Job Failure Rate (%)"** panel showing elevated values

### Investigation

1. **Open**: Overview dashboard > **"Job Throughput"** panel

   Shows the relationship between submitted, completed, and failed jobs over time. A spike in failures without a corresponding spike in submissions suggests a systemic issue.

2. **Open**: Jobs dashboard > **"Jobs Completed vs Failed"** panel

   Focused view on success vs failure rates.

3. **Get the job ID** from the API response or logs:

   ```bash
   # REST: submit and get job ID
   curl -s -X POST http://localhost:3000/teams/plan -H 'Content-Type: application/json' \
     -d '{"source":{"Game":{"version_groups":["red-blue"]}}}' | jq .job_id

   # Check job status
   curl -s http://localhost:3000/jobs/<job-id> | jq .
   ```

4. **Trace by job_id** in Jaeger: search for tag `job_id=<uuid>`. The span tree shows which phase failed.

5. **Search logs**:

   ```bash
   # JSON logs
   grep '<job-id>' logs.json | jq .

   # Text logs
   grep '<job-id>' logs.txt
   ```

### Common failure causes

| Error Message | Cause | Resolution |
|---|---|---|
| "No candidates remaining after filtering" | `min_bst` too high, or all pokemon excluded | Lower `min_bst`, check exclusion lists |
| "Failed to fetch type chart" | PokeAPI unreachable or rate-limited | Check PokeAPI status, pre-warm type chart cache |
| "Failed to fetch species" (in logs) | Individual pokemon fetch errors | Usually transient; retry the job |
| "Learnset fetch failed for X in Y" | Move data unavailable for that VG | Expected for some pokemon/VG combos; the fallback chain handles this |

### Metrics to monitor during incident

```promql
# Failure rate as percentage
rate(job_failed_total[5m]) / rate(job_submitted_total[5m]) * 100

# Are failures correlating with upstream issues?
rate(pokeapi_request_count_total[5m])
```

---

## Scenario 3: PokeAPI Degradation

### Symptoms

- Slow cold-cache operations
- HTTP errors in logs (`PokeAPI returned status 429/503`)
- Cache miss rate elevated

### Investigation

1. **Open**: PokeAPI dashboard > **"PokeAPI Request Rate"** panel

   Shows how many upstream calls we're making. After cache warm-up, this should be near zero. Sustained traffic indicates cold cache or `no_cache` usage.

2. **Open**: PokeAPI dashboard > **"PokeAPI Request Duration (p50 / p95 / p99)"** panel

   Latency >2s suggests Cloudflare throttling or PokeAPI server issues.

3. **Open**: PokeAPI dashboard > **"Cache Hits vs Misses"** panel

   Bursts of misses correspond to first-time game/pokedex fetches or cache clears. Sustained misses suggest cache bypass (`no_cache=true` in requests).

4. **Check rate limiter**: The client uses `governor` with default 20 req/s sustained, burst 5. Under heavy load with cold cache, the rate limiter queues requests, which adds latency.

### Log queries

```bash
# Find PokeAPI errors
grep 'PokeAPI returned status' logs.txt

# Find cache misses (debug level)
grep '"cache_hit":false' logs.json

# Count cache hit ratio from logs
grep -c '"cache_hit":true' logs.json
grep -c '"cache_hit":false' logs.json
```

### Resolution

| Cause | Fix |
|---|---|
| Cold cache | Pre-warm: `pokeplanner cache populate all` (uses lower concurrency: 3 req, 5 rps) |
| Cache expired | Default TTL is 1 year. Clear stale: `pokeplanner cache clear stale` |
| `no_cache` abuse | Check if clients are sending `no_cache=true` on every request |
| PokeAPI outage | Check [pokeapi.co](https://pokeapi.co/). Wait for recovery. Cached data continues to work. |
| Cloudflare rate limiting (429) | Reduce concurrency. Cache populate commands use conservative limits. |

---

## Scenario 4: Cache Issues

### Symptoms

- Unexpected cache misses (high upstream API traffic)
- Stale data being served
- Disk space growth in cache directory

### Investigation

1. **Check cache stats** via CLI:

   ```bash
   pokeplanner cache stats
   ```

   Shows entry counts per category, total size, and cache directory location.

2. **Check disk space**:

   ```bash
   du -sh ~/.pokeplanner/cache/
   du -sh ~/.pokeplanner/cache/*/
   ```

3. **Check for corruption**: Cache corruption is auto-detected and logged as warnings. Corrupted entries are deleted and treated as misses.

   ```bash
   grep 'cache' logs.txt | grep -i 'corrupt\|error\|failed'
   ```

### Resolution

| Issue | Fix |
|---|---|
| Stale data | `pokeplanner cache clear stale` (removes expired entries only) |
| Corruption | `pokeplanner cache clear all` (nuclear option, requires re-warming) |
| Single game stale | `pokeplanner cache clear game <name>` |
| Single pokemon stale | `pokeplanner cache clear pokemon <name>` |
| Disk space | `pokeplanner cache clear stale`, then check size again |
| Full re-warm | `pokeplanner cache populate all --include-variants` |

---

## Scenario 5: Move Selection Fallbacks

### Symptoms

- Jobs dashboard > **"Move Selection Fallbacks"** panel showing elevated rates
- `learnset_source_vg` field populated in team plan results (indicates a different VG was used)

### Investigation

1. **Open**: Jobs dashboard > **"Move Selection Fallbacks"** panel

   ```promql
   rate(move_selection_fallback_total[5m])
   ```

   This tracks how often the learnset fallback chain is triggered. Some fallback is normal and expected — many pokemon don't have learnset data in every version group.

2. **Check which VGs are falling back**: Search logs for fallback messages:

   ```bash
   grep 'fallback' logs.txt | grep 'learnset'
   # Example output:
   # Using same-gen fallback gold-silver for bulbasaur learnset
   # Using best-available fallback sword-shield for pikachu learnset
   ```

3. **Check if moves are actually unavailable**: Look for the "No learnset data found" warning:

   ```bash
   grep 'move coverage unavailable' logs.txt
   ```

### Resolution

This is usually not a problem — the fallback chain is working as designed. It exists because PokeAPI's learnset data is incomplete for some version groups.

| Situation | Action |
|---|---|
| Fallbacks are expected | No action needed. The fallback chain picks the best available data. |
| Want more accurate data | Use `--learnset-game` to specify a VG known to have good data |
| Fallbacks causing wrong moves | Report as a bug — the fallback chain should prefer same-generation VGs |

---

## Quick Reference: Useful PromQL

```promql
# Service request rate
rate(http_server_request_count_total[5m])

# Request latency p95
histogram_quantile(0.95, rate(http_server_request_duration_seconds_bucket[5m]))

# Job failure rate
rate(job_failed_total[5m]) / rate(job_submitted_total[5m]) * 100

# Cache hit ratio
rate(pokeapi_cache_hit_total[5m]) / (rate(pokeapi_cache_hit_total[5m]) + rate(pokeapi_cache_miss_total[5m]))

# PokeAPI upstream latency p95
histogram_quantile(0.95, rate(pokeapi_request_duration_seconds_bucket[5m]))

# Average candidate pool size
rate(team_candidate_pool_size_sum[5m]) / rate(team_candidate_pool_size_count[5m])

# Move fallback rate
rate(move_selection_fallback_total[5m])
```
