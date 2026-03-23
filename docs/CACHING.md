# Caching

PokePlanner aggressively caches PokeAPI responses to avoid hammering the upstream service. Understanding the caching layer matters if you're self-hosting or developing -- it is the primary mechanism that keeps request volume low.

## How It Works

Every PokeAPI response is cached to disk the first time it is fetched. Subsequent requests for the same resource are served from the local cache without making any network call.

- **Location**: `~/.pokeplanner/cache/` (overridable with `--cache-dir`)
- **Layout**: one JSON file per resource, organized by category -- e.g., `cache/pokemon/charizard.json`, `cache/type-chart/current.json`, `cache/game-pokemon/red-blue-variants-true.json`
- **TTL**: 1 year. PokeAPI data changes extremely rarely, so a long TTL is safe.
- **Corruption handling**: if a cached file can't be deserialized, it is logged, deleted, and treated as a cache miss.

### Aggregated Caches

Some operations aggregate multiple API responses into a single cached result:

- `game-pokemon/{game}-variants-{bool}.json` -- all Pokemon for a game, pre-resolved from species to forms
- `type-chart/current.json` -- the full 18x18 type effectiveness matrix

These save hundreds of API calls on subsequent requests for the same game.

## Rate Limiting

Even with caching, the first fetch of a new game's Pokemon pool can issue hundreds of requests (one per species + one per form). To stay respectful of PokeAPI's infrastructure:

- **Rate limiter**: a `governor`-based token bucket shared across all concurrent jobs and API handlers -- default **20 requests/second** with a burst allowance of **5**
- **Concurrency**: outbound fetches run via `BufferedUnordered` with a cap of **10 concurrent requests**
- **Bulk population mode**: `cache populate all` uses a gentler profile -- **3 concurrent requests** at **5 requests/second** -- designed for pre-warming the entire cache without spiking load

## Cache Management CLI

```bash
pokeplanner cache stats                     # Show entry counts, sizes, and location

pokeplanner cache populate game red-blue    # Pre-fetch all Pokemon for a game
pokeplanner cache populate pokedex national # Pre-fetch a specific pokedex
pokeplanner cache populate type-chart       # Pre-fetch the type effectiveness chart
pokeplanner cache populate all              # Pre-fetch everything (gentle rate limit)

pokeplanner cache clear stale               # Remove only expired entries
pokeplanner cache clear game red-blue       # Remove cached data for a specific game
pokeplanner cache clear pokemon charizard   # Remove cached data for a specific Pokemon
pokeplanner cache clear type-chart          # Remove the cached type chart
pokeplanner cache clear all                 # Remove all cached data
```

## Bypass

Pass `--no-cache` (CLI), `?no_cache=true` (REST query param), or `no_cache: true` (gRPC field) to skip the cache and fetch fresh data. The fresh response is still written back to cache.

## Observability

Cache hit/miss events are tracked via two metrics (`pokeapi.cache.hit` and `pokeapi.cache.miss`) and logged at `debug` level with the resource URL and elapsed time. See [OBSERVABILITY.md](OBSERVABILITY.md) for details.
