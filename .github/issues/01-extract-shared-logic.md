# Extract shared sort, coverage, and type-name logic to eliminate duplication

## TL;DR

Three pieces of logic are duplicated between the service and CLI crates. Extract them to shared locations to prevent divergence.

## Problem

1. **Sort logic duplicated** — `sort_pokemon()` at `crates/pokeplanner-service/src/lib.rs:488-506` is duplicated nearly identically in the CLI's pokemon search handler at `crates/pokeplanner-cli/src/main.rs:872-889`. Both contain identical 9-field match arms. If a new `SortField` variant is added, both must be updated.

2. **Coverage logic duplicated** — `analyze_team()` at `crates/pokeplanner-service/src/lib.rs:412-455` manually computes offensive coverage by iterating `PokemonType::ALL` and calling `type_chart.effectiveness()`, while `TeamPlanner` already computes the same thing via `team_offensive_coverage()`, `shared_weaknesses()`, and `uncovered_types()`. Both paths can diverge if scoring changes.

3. **`type_name()` duplicates `Display`** — `crates/pokeplanner-cli/src/main.rs:1435-1456` is an 18-arm match that maps `PokemonType` to `&str`, producing identical output to the `Display` impl at `crates/pokeplanner-core/src/model.rs:54-62`. This is dead-weight code that could silently go stale if a 19th type were added.

## Acceptance Criteria

- [ ] `sort_pokemon()` exists in exactly one place and is used by both service and CLI
- [ ] `analyze_team()` delegates to `TypeChart` methods rather than reimplementing coverage iteration
- [ ] `type_name()` is removed from the CLI; callers use `Display` or `to_string()` instead
- [ ] `cargo test` passes with no regressions
- [ ] No new public API surface beyond what's necessary

## Implementation Guidance

### Sort logic extraction

Move `sort_pokemon()` and `filter_sort_limit()` from `crates/pokeplanner-service/src/lib.rs:469-506` into `crates/pokeplanner-core/src/team.rs` (where `SortField` and `SortOrder` already live). Then:

- In `crates/pokeplanner-service/src/lib.rs`: replace the local functions with imports from core
- In `crates/pokeplanner-cli/src/main.rs:870-891`: replace the inline sort with a call to the shared function

### Coverage logic consolidation

In `crates/pokeplanner-service/src/lib.rs:412-455`, the `analyze_team()` method manually iterates types to compute offensive coverage (lines 433-443). Replace this with calls to the existing `TypeChart` methods that `TeamPlanner` already uses:

- `type_chart.team_offensive_coverage(&team_types)` — already returns the coverage score
- `type_chart.shared_weaknesses(&team_types)` — already called at line 445
- `type_chart.uncovered_types(&team_types)` — already called at line 446

The manual `PokemonType::ALL.iter().filter(...)` block (lines 433-443) should be replaced. You may need to add a method to `TypeChart` that returns the list of super-effectively-covered types (not just the score), since `team_offensive_coverage` currently returns only an `f64`.

### Remove `type_name()`

Delete `type_name()` at `crates/pokeplanner-cli/src/main.rs:1435-1456`. Grep for all call sites in that file and replace with `t.to_string()` or `format!("{t}")`. The `Display` impl on `PokemonType` produces identical lowercase output via serde.

## Things to Note

- The `CliSortField` / `CliSortOrder` enums and their `From` impls in the CLI (`main.rs:381-416`) are **not** duplication — they're Clap `ValueEnum` wrappers for CLI argument parsing. Keep them.
- The `filter_sort_limit` free function in the service crate is only used by `get_game_pokemon` and `get_pokedex_pokemon`. Moving it to core means the service depends on core for this logic, which is already the case for the types it operates on.
- This ticket is a pure refactor with no behavior changes. All existing tests should pass without modification.
