# Move-Based Offensive Capability Assessment — Implementation Plan

## Goal

Refactor the team planning scoring function to incorporate move-based analysis. Each team member gets 4 recommended moves that:
1. Have no negative self-effects (no recoil, no self-stat-drops)
2. Are as strong as possible
3. Match the pokemon's better offensive stat (all physical if Atk > SpA, all special otherwise)
4. Follow a STAB + coverage allocation: 2 STAB moves, 2 coverage moves that mitigate weaknesses or mirror matchups
5. Are displayed alongside the team plan output with their offensive type coverages

## PokeAPI Data Needed

The `/move/{name}` endpoint provides two fields not currently captured:
- **`meta.drain`** (i32): negative = recoil %, positive = drain %
- **`stat_changes`** (array): `[{change: i32, stat: {name: "special-attack"}}]` — negative `change` on the user's stats = self-debuff (e.g., Overheat: `stat_changes: [{change: -2, stat: "special-attack"}]`)

We also need the `stat_chance` from `meta` — if `stat_chance < 100` and `stat_changes` has negative entries, the debuff isn't guaranteed and may be acceptable. But for simplicity in v1, any move with 100% chance of self-debuff OR any recoil is excluded.

## Phase 1: Core Data Model Changes

### 1.1 Extend `Move` struct (`pokeplanner-core/src/model.rs`)

Add fields to represent recoil/drain and self-stat changes:

```rust
pub struct Move {
    // ... existing fields ...
    /// Negative = recoil %, positive = drain/heal %.  0 = neither.
    pub drain: i32,
    /// Stat changes the move causes to the USER (not target).
    /// Only populated for moves with 100% stat_chance.
    /// Negative values = stat drops (e.g., Overheat -2 SpA).
    pub self_stat_changes: Vec<MoveStatChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveStatChange {
    pub stat: String,    // "attack", "special-attack", "defense", etc.
    pub change: i32,     // positive = boost, negative = drop
}
```

### 1.2 Add `RecommendedMoves` to `TeamMember` (`pokeplanner-core/src/team.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedMove {
    pub move_name: String,
    pub move_type: PokemonType,
    pub power: u32,
    pub damage_class: String,   // "physical" or "special"
    pub role: MoveRole,         // Why this move was selected
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveRole {
    Stab,                       // Same-type attack bonus
    WeaknessCoverage(PokemonType), // Covers a specific weakness type
    MirrorCoverage,             // Effective in mirror matches
}

pub struct TeamMember {
    // ... existing fields ...
    pub recommended_moves: Option<Vec<RecommendedMove>>,
}
```

Making it `Option` keeps backward compatibility — plans generated without move data still work.

## Phase 2: PokeAPI Response & Client Changes

### 2.1 Extend `MoveResponse` (`pokeplanner-pokeapi/src/types.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveMeta {
    pub drain: i32,
    pub healing: i32,
    pub crit_rate: i32,
    pub ailment_chance: i32,
    pub flinch_chance: i32,
    pub stat_chance: i32,
    // min/max hits/turns omitted for now
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveStatChangeResponse {
    pub change: i32,
    pub stat: NamedApiResource,
}

pub struct MoveResponse {
    // ... existing fields ...
    #[serde(default)]
    pub meta: Option<MoveMeta>,
    #[serde(default)]
    pub stat_changes: Vec<MoveStatChangeResponse>,
}
```

### 2.2 Update `get_move()` in client.rs

Map the new response fields into the core `Move`:
- `drain` ← `resp.meta.map(|m| m.drain).unwrap_or(0)`
- `self_stat_changes` ← filter `stat_changes` where `change < 0` AND `meta.stat_chance >= 100` (or `stat_chance == 0` which means "always applies")
  - Note: PokeAPI uses `stat_chance: 0` to mean "always applies" for moves where the stat change is a guaranteed side effect. `stat_chance > 0 && < 100` means a probability.

## Phase 3: Move Selection Algorithm (`pokeplanner-service`)

### 3.1 New module: `move_selector.rs`

Core logic for selecting 4 moves per pokemon:

```rust
pub struct MoveSelector<'a> {
    type_chart: &'a TypeChart,
}

pub struct MoveRecommendation {
    pub moves: Vec<RecommendedMove>,
    pub move_type_coverage: Vec<PokemonType>,  // types these moves hit SE
}

impl MoveSelector {
    pub fn select_moves(
        &self,
        pokemon: &Pokemon,
        learnset: &[DetailedLearnsetEntry],
        weaknesses: &[PokemonType],
    ) -> MoveRecommendation;
}
```

**Algorithm:**

1. **Filter eligible moves**: Only damaging moves (`power.is_some() && power > 0`) with `damage_class` matching the pokemon's preferred class (`"physical"` if `stats.attack >= stats.special_attack`, else `"special"`). Exclude moves with `drain < 0` (recoil) or any negative `self_stat_changes`.

2. **Select 2 STAB moves**: From eligible moves matching the pokemon's type(s), pick the 2 highest-power moves. If the pokemon is dual-type, prefer one of each type. If only one STAB type has eligible moves, take the best 2 of that type. If fewer than 2 STAB moves exist, fill remaining slots with coverage moves.

3. **Select 2 coverage moves**: These address the pokemon's defensive weaknesses.
   - For each weakness type, find moves that are super-effective against that type (i.e., a move whose type hits the weakness type for ≥2.0x damage)
   - Rank coverage candidates by: (a) number of weakness types they cover, (b) power
   - Greedy selection: pick the move covering the most uncovered weaknesses, then pick the next best for remaining gaps
   - If all weaknesses are already covered by one move, the second coverage slot targets mirror matches (move effective against the pokemon's own type(s))
   - For mono-type pokemon with few weaknesses (like Normal with only Fighting weakness): one move for weakness coverage, one for mirror coverage

4. **Fallback**: If fewer than 4 eligible moves exist, return what's available (some pokemon have very limited movepools).

### 3.2 Integrate into team plan job flow

In `run_team_plan_job` (service `lib.rs`), after team selection:

1. Add a new phase: **"Selecting recommended moves"** (step 3/4, shift "Complete" to 4/4)
2. For each team member, fetch their learnset for the relevant version group(s)
3. Run `MoveSelector::select_moves()` for each member
4. Attach results to `TeamMember::recommended_moves`

This happens AFTER team selection (scoring remains type-based for now) to avoid making the planning loop dependent on learnset fetches — that would be prohibitively slow for beam search over hundreds of candidates.

### 3.3 Version group context

The `TeamPlanRequest` already has `source` which contains version group(s). For `TeamSource::Game`, use the first version group for learnset lookups. For `Pokedex` or `Custom`, learnset game context would need to be optional (skip move recommendations or require a `--learnset-game` flag).

Add to `TeamPlanRequest`:
```rust
/// Version group for move recommendations. Defaults to first game in source.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub learnset_version_group: Option<String>,
```

## Phase 4: Scoring Refinement

### 4.1 Move-aware offensive coverage (future enhancement)

The current scoring uses pokemon types for offensive coverage. With move data, we could score based on what the team's moves _actually hit_ rather than what their types could theoretically hit. This is a follow-up because:
- It would require learnset fetches during scoring (slow for beam search)
- The type-based heuristic is a reasonable proxy for now

### 4.2 Stat-form coherence bonus (immediate)

Add a minor scoring bonus for pokemon whose higher offensive stat matches their type's typical damage class. This doesn't require move data and rewards pokemon that will use their moves more effectively:
- A Fire pokemon with high SpA naturally uses special Fire moves well
- A Fighting pokemon with high Atk naturally uses physical Fighting moves well

This is a lightweight proxy that can be added to `score_team_refs` without learnset data.

## Phase 5: CLI Display Changes

### 5.1 Extend `print_team_plans`

After the stats row and weakness row for each member, add a moves section:

```
  charizard              Fire/Flying   534   78   84   78  109   85  100
                         4x: Rock  2x: Electric, Water
                         Moves: fire-blast (Fire), air-slash (Flying),
                                dragon-pulse (Dragon→Rock,Dragon), solar-beam (Grass→Water,Rock,Ground)
```

The `→Type,Type` notation shows which weakness/coverage types each coverage move addresses.

### 5.2 Team move coverage summary

After the per-member section, add a "Move coverage" line showing what types the team's actual moves can hit SE, complementing the existing type-based coverage:

```
  Move coverage: 94% (17/18 types hit SE by recommended moves)
    Not covered by moves: Steel
```

## Phase 6: Documentation & Tests

### 6.1 Update docs
- `ARCHITECTURE.md`: Add move selection algorithm section
- `IMPLEMENTATION_CHECKLIST.md`: Add Phase 5 items
- `CLAUDE.md`: Update API endpoints, team types, scoring description

### 6.2 Tests
- Unit tests in `move_selector.rs`: test STAB selection, coverage selection, recoil exclusion, stat-drop exclusion, fallback for limited movepools, mono-type vs dual-type, physical vs special preference
- Update existing `team_planner.rs` tests to verify `recommended_moves` is populated
- Integration test with mock learnset data flowing through the full plan

## Implementation Order

1. **Phase 1** — Core types (Move fields, RecommendedMove, MoveRole, TeamMember extension)
2. **Phase 2** — PokeAPI response types and client mapping
3. **Phase 3.1** — `move_selector.rs` with unit tests
4. **Phase 3.2-3.3** — Service integration (learnset fetch in plan job, version group plumbing)
5. **Phase 5** — CLI display
6. **Phase 4.2** — Optional stat-form coherence scoring bonus
7. **Phase 6** — Docs and remaining tests

## Key Design Decisions

1. **Post-hoc move selection**: Moves are recommended AFTER team composition is chosen, not during scoring. This keeps the planner fast (no learnset I/O in the inner loop) while still providing actionable guidance.

2. **Uniform damage class**: All 4 moves match the pokemon's higher stat. Mixed attackers exist but are uncommon enough that optimizing for the dominant stat is the right default. A future flag could allow mixed sets.

3. **Recoil/debuff filtering via API metadata**: Using `meta.drain` and `stat_changes` + `stat_chance` from PokeAPI rather than text-parsing `effect` strings. This is structured and reliable.

4. **Weakness-first coverage**: The 2 coverage moves prioritize mitigating the pokemon's own weaknesses over general type coverage. This makes each pokemon more self-sufficient and reduces team dependency. The mirror-match fallback handles cases where weaknesses are already well-covered.

5. **Backward compatibility**: `recommended_moves: Option<Vec<...>>` means old serialized plans still deserialize, and plans without move data still display correctly.
