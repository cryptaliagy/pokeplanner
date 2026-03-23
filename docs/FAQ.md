# FAQ

## Why does this project have a CLI, a REST API, *and* a gRPC API?

Because it was fun to build all three. PokePlanner is a learning project, and one of the goals was to see what it takes to serve the same Rust business logic through completely different interfaces. None of this needs three separate APIs -- a CLI alone would do the job just fine.

That said, each one teaches something different:

- **CLI** -- the quickest way to actually use the thing. Good for poking around, scripting, and sanity-checking during development.
- **REST API** -- the obvious choice if a web frontend ever materializes. Axum makes it almost too easy to bolt on.
- **gRPC API** -- the excuse to learn Protobuf and Tonic. Strongly-typed service contracts, binary serialization, the whole deal. Overkill for a Pokemon planner? Absolutely.

The nice part is that all three are thin adapters over the same `PokePlannerService`, so adding a feature or fixing a bug only happens once in the service layer. The interfaces just translate requests in and responses out.

## How does the team planning algorithm work?

The planner scores teams on three components: **offensive coverage** (40%), **defensive resilience** (30%), and **base stat total** (30%). For the full math, see [COST_FUNCTION.md](COST_FUNCTION.md).

Two algorithms are used depending on pool size:

- **N <= 25 candidates**: exact brute-force search over all C(N, 6) combinations. Provably optimal.
- **N > 25 candidates**: greedy beam search with beam width 50. Fast approximation.

The planner returns `top_k` results (default 5), ranked by score.

## How does move selection work?

After the planner picks teams, PokePlanner optionally selects 4 recommended moves for each team member:

1. **Filter** to damaging moves matching the Pokemon's dominant offensive stat (physical or special)
2. **Exclude** recoil moves (drain < 0) and moves with guaranteed self-debuffs
3. **Pick 2 STAB moves** (one per type for dual-types when possible)
4. **Pick 2 coverage moves** using greedy set-cover over the team's weaknesses, with mirror-match fallback

Move data comes from the PokeAPI learnset for the requested version group. If that VG doesn't have data, a fallback chain tries: same-generation VGs, then all VGs picking the most recent.

## Where does the data come from?

All Pokemon data comes from [PokeAPI v2](https://pokeapi.co), a free and open RESTful API. PokePlanner never stores its own Pokemon data -- it fetches and caches PokeAPI responses.

The navigation chain is: version-group -> pokedexes -> species -> varieties (forms) -> stats/types. Megas, regional forms, and Gigantamax are non-default varieties discovered via the species endpoint.

## How does caching work?

See [CACHING.md](CACHING.md) for the full breakdown. Short version: every PokeAPI response is cached to disk with a 1-year TTL. Rate limiting (20 req/s sustained, burst 5) keeps upstream load reasonable even on cold-cache fetches. The cache can be pre-warmed via `pokeplanner cache populate`.

## What is the unusable list?

A local persistence file (`~/.pokeplanner/unusable.json`) that lets you mark Pokemon you never want in team plans. Useful for personal bans, nuzlocke rules, or excluding legendaries.

```bash
pokeplanner unusable add magikarp,metapod
pokeplanner unusable list
pokeplanner unusable remove magikarp
pokeplanner unusable clear
```

Unusable Pokemon are automatically excluded from the candidate pool during team planning.

## Does this project have observability? For a Pokemon planner?

Yes. The observability stack is deliberately over-built for the problem size. There are 13 OpenTelemetry metrics, distributed tracing, structured JSON logging, and importable Grafana dashboards.

Was all of that necessary? No. Was it educational to build? Very much so. See [OBSERVABILITY.md](OBSERVABILITY.md) for the full reference and [ops/RUNBOOK.md](../ops/RUNBOOK.md) for the diagnostic playbook.

## How do I contribute or report issues?

This is a personal learning project, but issues and feedback are welcome. Open a GitHub issue or submit a pull request.

Before submitting code, run `just ci` (or at minimum `just format lint check test`) to ensure all checks pass. See the [justfile](../justfile) for available commands.
