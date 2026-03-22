# Team Planning Cost Function

This document describes the scoring function used by the team planner to evaluate and rank teams of 6 Pokemon.

## Overview

The planner assigns each candidate team a scalar score in `[0.0, 1.0]`. Higher scores are better. The score is a weighted sum of three components:

```
score = 0.4 × offensive_coverage + 0.3 × defensive_score + 0.3 × bst_score
```

| Component | Weight | Measures |
|-----------|--------|----------|
| Offensive coverage | 40% | How many of the 18 types the team can hit super-effectively |
| Defensive score | 30% | How few shared type weaknesses the team has |
| BST score | 30% | Raw statistical power of the team |

## Component Definitions

### 1. Offensive Coverage (40%)

**Question answered:** "How many different types can this team threaten super-effectively?"

```
offensive_coverage = |{T ∈ 18 types : ∃ pokemon P in team, ∃ type A in P.types, effectiveness(A, T) ≥ 2.0}| / 18
```

For each of the 18 types, we check whether **any** team member has a STAB type that is super-effective against it. The score is the fraction of types covered.

**Range:** `[0.0, 1.0]`. A score of 1.0 means the team has at least one super-effective STAB attacker for every type. A typical well-composed team scores 0.7–0.9.

**Key assumptions:**
- Only considers STAB types (the 1–2 types a Pokemon *is*), not its full move coverage. A Water-type Pokemon can learn Ice Beam, but only its Water STAB is counted.
- Uses the standard Gen 6+ type chart. No ability interactions (e.g., Mold Breaker bypassing immunities).
- Each type is equally weighted — covering Fairy is worth the same as covering Normal.

### 2. Defensive Score (30%)

**Question answered:** "How vulnerable is this team to being swept by a single attacking type?"

```
For each attacking type A in 18 types:
    weak_count = |{P in team : effectiveness_against_pokemon(A, P.types) ≥ 2.0}|

bad_types = |{A : weak_count(A) ≥ 3}|

defensive_score = 1.0 - (bad_types / 18)
```

For each attacking type, we count how many team members are weak to it (taking dual-type interactions into account — a Water/Ground Pokemon takes 4× from Grass but 0× from Electric). An attacking type is "bad" if 3 or more team members are weak to it. The score penalizes teams with many such shared weaknesses.

**Range:** `[0.0, 1.0]`. A score of 1.0 means no attacking type hits 3+ team members super-effectively. A typical team scores 0.8–1.0.

**Dual-type calculation:** Effectiveness against a dual-type Pokemon multiplies the individual type multipliers:
```
effectiveness_against_pokemon(A, [T1, T2]) = effectiveness(A, T1) × effectiveness(A, T2)
```

This correctly handles:
- 4× weaknesses (e.g., Grass → Water/Ground: 2.0 × 2.0 = 4.0)
- Immunities through dual-typing (e.g., Electric → Water/Ground: 2.0 × 0.0 = 0.0)
- Neutral combined matchups (e.g., Fire → Water/Grass: 0.5 × 2.0 = 1.0)

**Threshold of 3:** The threshold of 3 was chosen because in a 6-Pokemon team, having half or more members weak to a single type represents a significant structural vulnerability. An opponent with that type can reliably threaten the majority of the team.

### 3. BST Score (30%)

**Question answered:** "How statistically strong are these Pokemon in aggregate?"

```
bst_score = sum(P.bst for P in team) / (team_size × 780)
```

The total base stat total (BST) of the team, normalized by the theoretical maximum. The normalizer is `team_size × 780` where 780 is the BST of Mega Rayquaza / Mega Mewtwo (the highest BST in the games).

**Range:** `[0.0, 1.0]`. Most fully-evolved Pokemon have BST 400–600. A team of six 500-BST Pokemon scores `3000 / 4680 ≈ 0.64`. A team of six pseudo-legendaries (BST 600) scores `3600 / 4680 ≈ 0.77`.

## Weight Rationale

The weights (0.4 / 0.3 / 0.3) reflect these design priorities:

1. **Offensive coverage gets the most weight (40%)** because in competitive and in-game Pokemon, the ability to hit opponents super-effectively is the single most impactful team-building factor. A team that cannot threaten common types will struggle regardless of stats.

2. **Defensive resilience and raw stats are equally weighted (30% each)** because:
   - Defensive resilience prevents catastrophic weaknesses but is less granular (most well-typed teams already have decent defensive spread)
   - BST matters for actual battles but has diminishing returns — the difference between 500 and 520 BST is rarely decisive

## Algorithm Selection

The planner uses two algorithms depending on the number of candidates:

| Candidates (N) | Algorithm | Complexity | Guarantee |
|-----------------|-----------|------------|-----------|
| N ≤ 25 | Exact enumeration | O(C(N,6) × scoring) ≈ O(177K × scoring) | Provably optimal |
| N > 25 | Greedy beam search (width 50) | O(6 × 50 × N × scoring) | High-quality heuristic |

The beam search builds teams incrementally, keeping the best 50 partial teams at each step. This means it can miss globally optimal teams that require a "bad" early pick to enable a synergistic later pick, but in practice it produces excellent results because Pokemon type coverage is largely additive.

## Alternatives Considered

### Alternative A: Move-Based Offensive Coverage

Instead of using a Pokemon's STAB types, score based on the actual moves it can learn:

```
offensive_coverage = |{T : ∃ P in team, ∃ move M in P.learnset, effectiveness(M.type, T) ≥ 2.0}| / 18
```

**Pros:** Much more accurate. A Starmie (Water/Psychic) can learn Thunderbolt, Ice Beam, and Psychic, giving it far broader coverage than its STAB types suggest.

**Cons:**
- Requires fetching move data for every Pokemon (massive API load increase)
- Learnsets vary by generation/game — adds complexity
- Scoring becomes moveset-dependent, creating a chicken-and-egg problem (optimal team depends on moves, optimal moves depend on team)
- Significantly higher implementation complexity

**Verdict:** Excellent future enhancement. Current STAB-only approach is a reasonable v1 approximation.

### Alternative B: Speed Tier Awareness

Add a component that rewards teams with diverse speed tiers:

```
speed_diversity = stddev(P.stats.speed for P in team) / max_speed
```

**Pros:** A team with all slow Pokemon loses initiative; speed diversity means some members can outrun threats.

**Cons:**
- Speed's value is highly context-dependent (Trick Room inverts it, priority moves bypass it)
- Adding a fourth scoring component dilutes the existing three
- Difficult to weight meaningfully without metagame context

**Verdict:** Interesting but too context-dependent for a general planner.

### Alternative C: Resistance-Based Defensive Scoring

Instead of counting weaknesses, count resistances:

```
For each type T:
    resist_count = |{P in team : effectiveness_against_pokemon(T, P.types) ≤ 0.5}|

resistance_score = sum(resist_count for T in 18 types) / (18 × team_size)
```

**Pros:** Rewards teams that can wall many types, not just avoid weaknesses.

**Cons:**
- Can lead to Steel-heavy teams that resist 10+ types but share Ground/Fire/Fighting weaknesses
- Weaknesses are more impactful than resistances in practice (losing a Pokemon vs. taking reduced damage)
- Current approach already implicitly rewards resistance diversity by penalizing shared weaknesses

**Verdict:** Could complement the current approach as an additional sub-component. Not a replacement.

### Alternative D: Type Uniqueness Scoring

Penalize teams where multiple Pokemon share types:

```
unique_types = |union(P.types for P in team)|
type_uniqueness = unique_types / (team_size × 2)
```

**Pros:** Directly targets the "non-overlapping type spread" goal. Simple to compute.

**Cons:**
- Too blunt — two Fire/Flying Pokemon are redundant, but a Fire/Fighting and Fire/Psychic share Fire while contributing different secondary coverage
- Overlaps heavily with the offensive coverage score (more unique types → more coverage naturally)
- Penalizes dual-types that share one type even when they serve different roles

**Verdict:** Too simplistic. The offensive/defensive scoring already captures type diversity more nuancefully.

### Alternative E: Genetic Algorithm

Replace beam search with a genetic algorithm:
- Population of random teams
- Crossover: swap members between high-scoring teams
- Mutation: replace random team member with a random candidate
- Iterate for N generations

**Pros:** Better at escaping local optima than greedy beam search. Can find synergistic teams where individual members look weak but the whole is strong.

**Cons:**
- Non-deterministic results (different runs → different teams)
- Harder to tune (population size, mutation rate, generations)
- Overkill for the additive nature of Pokemon type coverage
- Beam search already produces near-optimal results for this problem

**Verdict:** Worthwhile for competitive team building with move/ability/item interactions. Overkill for type+BST optimization.

### Alternative F: Integer Linear Programming (ILP)

Formulate team selection as an ILP:
- Binary variables: x_i ∈ {0,1} for each candidate Pokemon
- Constraint: sum(x_i) = 6
- Objective: maximize coverage + stats

**Pros:** Provably optimal for any N. Can handle complex constraints (e.g., "no more than 2 Pokemon of the same type").

**Cons:**
- Requires an ILP solver dependency (e.g., `good_lp`, `highs`)
- Offensive coverage is not linear (it's a set coverage problem)
- Linearizing set coverage requires auxiliary variables, making the formulation complex
- The beam search is already fast and effective for this problem

**Verdict:** Theoretically elegant but the non-linear nature of type coverage makes formulation awkward. Better for problems with linear objectives.

## Current Limitations

1. **STAB-only coverage:** Does not account for move learnsets — a Pokemon's actual offensive potential may be much broader than its STAB types.
2. **No ability interactions:** Abilities like Levitate (Ground immunity), Flash Fire (Fire immunity), or Dry Skin (Water absorb) fundamentally change type matchups but are not modeled.
3. **Equal type weighting:** All types are weighted equally, but in practice some types (Ground, Fairy, Steel) are more relevant than others in the metagame.
4. **No role differentiation:** The planner doesn't distinguish between physical attackers, special attackers, walls, or support Pokemon. A team of 6 sweepers with great coverage may still lose to a single wall.
5. **Generation-agnostic type chart:** Uses the current (Gen 6+) type chart. Historical type charts (pre-Fairy, pre-Dark/Steel) are not considered even when planning for older games.
