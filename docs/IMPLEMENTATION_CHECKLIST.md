# Implementation Checklist

## Phase 1: Foundation
- [x] 1.1 Core types expanded (PokemonType enum, BaseStats, Pokemon, team.rs)
- [x] 1.2 Job modifications (JobKind, JobProgress, structured JobResult)
- [x] 1.3 PokeAPI crate — types.rs (response deserialization)
- [x] 1.3 PokeAPI crate — cache.rs (DiskCache with 1-year TTL)
- [x] 1.3 PokeAPI crate — traits.rs (PokeApiClient trait)
- [x] 1.3 PokeAPI crate — client.rs (HTTP + cache + rate limit)
- [x] 1.4 Intermediary caching (game-pokemon, type-chart aggregates)

## Phase 2: Team Planning Algorithm
- [x] 2.1 Type effectiveness matrix (TypeChart, fallback, scoring methods)
- [x] 2.2 Hybrid team planner (exact brute-force N≤25, beam search N>25)
- [x] 2.3 Service layer integration (PokePlannerService<S, P> with new methods)

## Phase 3: API Surface Integration
- [x] 3.1 REST endpoints (version-groups, game-pokemon, pokemon, teams/plan, teams/analyze)
- [x] 3.2 gRPC handlers updated for new service signature
- [x] 3.3 CLI commands (list-games, game-pokemon, pokemon, plan-team, analyze-team)

## Phase 4: Documentation
- [x] 4.1 ARCHITECTURE.md updated
- [x] 4.2 DEPENDENCIES.md updated (reqwest, governor, futures)
- [x] 4.3 STRUCTURE.md updated (pokeplanner-pokeapi, new files, cache dirs)
- [x] 4.4 IMPLEMENTATION_CHECKLIST.md created
- [x] 4.5 CLAUDE.md updated

## Future Work
- [x] gRPC proto messages for new RPCs (PlanTeam, GetGamePokemon, etc.)
- [x] Integration tests with mocked HTTP responses
- [ ] gRPC integration tests (make GrpcHandler generic, tonic in-process transport)
- [ ] Stale job recovery on startup (Running → Failed for interrupted jobs)
