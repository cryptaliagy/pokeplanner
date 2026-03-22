# Split CLI monolith into modules

## TL;DR

`crates/pokeplanner-cli/src/main.rs` is 1,775 lines containing argument parsing, business logic orchestration, and display formatting all in one file. Split it into focused modules.

## Problem

The CLI's `main.rs` handles everything:
- Clap struct definitions and argument parsing (~400 lines)
- Subcommand dispatch and business logic orchestration (~900 lines)
- Display formatting, coloring, stat bars, and table rendering (~400 lines)
- Utility functions like `format_bytes()`, `type_name()`, `stat_bar()` (~75 lines)

This makes the file hard to navigate, hard to review in PRs, and hard to test display logic in isolation. Adding a new subcommand means touching a file that's already near the practical readability limit.

## Acceptance Criteria

- [ ] `main.rs` is reduced to Clap parsing + dispatch (under ~200 lines)
- [ ] Display/formatting helpers are in a dedicated module (e.g., `display.rs`)
- [ ] Subcommand handlers are organized into modules (e.g., `commands/pokemon.rs`, `commands/team.rs`, `commands/cache.rs`, `commands/moves.rs`)
- [ ] No functional changes — all CLI behavior is identical
- [ ] `cargo test` passes (including any inline tests in the CLI crate)

## Implementation Guidance

### Suggested module structure

```
crates/pokeplanner-cli/src/
├── main.rs              # Clap structs, parse, dispatch (keep thin)
├── display.rs           # stat_bar(), color_type(), colored_type_list(),
│                        #   print_pokemon_list(), print_pokemon_detail(),
│                        #   print_team_plans(), print_move_detail(),
│                        #   print_learnset(), format_bytes()
├── commands/
│   ├── mod.rs
│   ├── pokemon.rs       # show, search subcommands
│   ├── team.rs          # plan-team, analyze-team
│   ├── cache.rs         # cache stats/populate/clear
│   ├── moves.rs         # moves show/search
│   ├── games.rs         # list-games, game-pokemon, pokedex-pokemon
│   └── unusable_cmd.rs  # unusable add/remove/list/clear
└── unusable.rs          # UnusableStore (already separate — keep as-is)
```

### Step-by-step

1. **Extract `display.rs`** first — move all functions from the "Display helpers" section (starting around line 1431) into `display.rs`. These are pure functions with no service dependencies. Key functions:
   - `type_name()` → delete entirely (use `to_string()` from `Display` impl — if the shared-logic ticket is done first, this is already gone)
   - `color_type()` (line 1458)
   - `colored_type_list()` (line 1482)
   - `stat_bar()` (line 1491)
   - `print_pokemon_list()`, `print_pokemon_detail()`, `print_team_plans()`, `print_move_detail()`, `print_learnset()` — these are larger display functions scattered through the file

2. **Extract command handlers** — each match arm in the main `match cli.command { ... }` block becomes a function in the appropriate `commands/` module. The function signature should take the parsed args + a reference to the service + the unusable store where needed.

3. **Keep `main.rs` thin** — it should contain the `Cli` struct, `Commands` enum, and a `main()` that parses args, creates the service, and dispatches to command modules.

### What stays in `main.rs`

- `Cli`, `Commands`, and all Clap `#[derive]` structs (these define the CLI interface and should be visible at the top level)
- `default_data_dir()`
- `main()` with the dispatch match

## Things to Note

- The `PokemonSearch` subcommand handler (around lines 600-900) is the largest single block. It has its own helper closure `parse_stat_filter()` (lines 614-643) that should move with it.
- `print_pokemon_list()` and `print_team_plans()` reference `UnusableStore` — the display module will need to accept it as a parameter or reference.
- The `make_populate_client()` helper (used by cache populate commands) creates a separate `PokeApiHttpClient` with lower concurrency settings. Keep it with the cache command module.
- `unusable.rs` is already a separate module and well-structured. No changes needed there.
- Consider whether the Clap structs should also move to a `cli.rs` or `args.rs` module. This is optional — keeping them in `main.rs` is fine if the command handlers are extracted.
