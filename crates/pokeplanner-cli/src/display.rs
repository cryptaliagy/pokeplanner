use colored::Colorize;
use pokeplanner_core::{MoveCoverage, MoveRole, PokemonType, RecommendedMove};

use crate::unusable::UnusableStore;

pub fn color_type(t: &PokemonType) -> colored::ColoredString {
    let name = format!("{t}");
    match t {
        PokemonType::Fire => name.red(),
        PokemonType::Water => name.blue(),
        PokemonType::Grass => name.green(),
        PokemonType::Electric => name.yellow(),
        PokemonType::Ice => name.cyan(),
        PokemonType::Fighting => name.truecolor(194, 46, 27),
        PokemonType::Poison => name.purple(),
        PokemonType::Ground => name.truecolor(226, 191, 101),
        PokemonType::Flying => name.truecolor(169, 143, 243),
        PokemonType::Psychic => name.truecolor(249, 85, 135),
        PokemonType::Bug => name.truecolor(166, 185, 26),
        PokemonType::Rock => name.truecolor(182, 161, 54),
        PokemonType::Ghost => name.truecolor(115, 87, 151),
        PokemonType::Dragon => name.truecolor(111, 53, 252),
        PokemonType::Dark => name.truecolor(112, 87, 70),
        PokemonType::Steel => name.truecolor(183, 183, 206),
        PokemonType::Fairy => name.truecolor(214, 133, 173),
        PokemonType::Normal => name.white(),
    }
}

pub fn colored_type_list(types: &[PokemonType]) -> String {
    types
        .iter()
        .map(|t| format!("{}", color_type(t)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Strip ANSI escape codes from a string (for measuring plain text width).
pub fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Format a list of recommended moves with role annotations.
/// Returns a Vec of formatted move strings (with ANSI colors).
pub fn format_recommended_moves(moves: &[RecommendedMove]) -> Vec<String> {
    moves
        .iter()
        .map(|m| {
            let role_str = match &m.role {
                MoveRole::Stab => "STAB".to_string(),
                MoveRole::WeaknessCoverage(types) => {
                    let type_names: Vec<String> = types.iter().map(capitalize_type).collect();
                    format!("->{}", type_names.join(","))
                }
                MoveRole::MirrorCoverage => "mirror".to_string(),
            };
            format!(
                "{} ({}, {})",
                m.move_name,
                color_type(&m.move_type),
                role_str
            )
        })
        .collect()
}

/// Capitalize the first letter of a type name for display in move annotations.
pub fn capitalize_type(t: &PokemonType) -> String {
    let s = t.to_string();
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Compute the types a pokemon hits super-effectively with its STAB types.
pub fn pokemon_offensive_strengths(
    types: &[PokemonType],
    chart: &pokeplanner_service::type_chart::TypeChart,
) -> Vec<PokemonType> {
    PokemonType::ALL
        .iter()
        .filter(|&&target| {
            types
                .iter()
                .any(|&atk| chart.effectiveness(atk, target) >= 2.0)
        })
        .copied()
        .collect()
}

/// Compute the types a pokemon resists (takes ≤0.5x damage from).
pub fn pokemon_resistances(
    types: &[PokemonType],
    chart: &pokeplanner_service::type_chart::TypeChart,
) -> Vec<PokemonType> {
    PokemonType::ALL
        .iter()
        .filter(|&&atk| chart.effectiveness_against_pokemon(atk, types) <= 0.5)
        .copied()
        .collect()
}

/// Compute the types a pokemon is immune to (takes 0x damage from).
pub fn pokemon_immunities(
    types: &[PokemonType],
    chart: &pokeplanner_service::type_chart::TypeChart,
) -> Vec<PokemonType> {
    PokemonType::ALL
        .iter()
        .filter(|&&atk| chart.effectiveness_against_pokemon(atk, types) == 0.0)
        .copied()
        .collect()
}

/// Render a stat bar: filled portion + dimmed remainder, fixed width.
pub fn stat_bar(value: u32, max: u32, width: usize) -> String {
    let filled = ((value as f64 / max as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;

    let bar_color = if value >= 130 {
        "green"
    } else if value >= 80 {
        "yellow"
    } else {
        "red"
    };

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);

    let colored_filled = match bar_color {
        "green" => filled_str.green(),
        "yellow" => filled_str.yellow(),
        _ => filled_str.red(),
    };

    format!("{colored_filled}{}", empty_str.dimmed())
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn print_pokemon_list(pokemon: &[pokeplanner_core::Pokemon], unusable: &UnusableStore) {
    println!("{}", format!("{} pokemon found:", pokemon.len()).bold());
    println!();
    for p in pokemon {
        let types_display: Vec<String> = p
            .types
            .iter()
            .map(|t| format!("{}", color_type(t)))
            .collect();
        let mut markers = String::new();
        if !p.is_default_form {
            markers.push_str(&" *".dimmed().to_string());
        }
        if unusable.is_unusable(&p.form_name) {
            markers.push_str(&" [unusable]".red().to_string());
        }
        println!(
            "  #{:>4} {:<25} {:<20} BST: {}{}",
            p.pokedex_number.to_string().dimmed(),
            p.form_name,
            types_display.join("/"),
            p.bst().to_string().bold(),
            markers,
        );
    }
}

pub fn print_pokemon_detail(p: &pokeplanner_core::Pokemon, unusable: &UnusableStore) {
    let chart = pokeplanner_service::type_chart::TypeChart::fallback();

    let types_display: Vec<String> = p
        .types
        .iter()
        .map(|t| format!("{}", color_type(t)))
        .collect();
    let mut tags = String::new();
    if !p.is_default_form {
        tags.push_str(&" (variant)".dimmed().to_string());
    }
    if unusable.is_unusable(&p.form_name) {
        tags.push_str(
            &" (unusable — excluded from team planning)"
                .red()
                .to_string(),
        );
    }

    println!();
    println!("  {} {}", p.form_name.bold(), tags);
    println!("  #{} {}", p.pokedex_number, types_display.join(" / "));
    println!();

    // Stats with bars (max single stat is 255 for bar scaling)
    let max = 255;
    let bar_w = 20;
    let stats = [
        ("HP ", p.stats.hp),
        ("Atk", p.stats.attack),
        ("Def", p.stats.defense),
        ("SpA", p.stats.special_attack),
        ("SpD", p.stats.special_defense),
        ("Spe", p.stats.speed),
    ];

    for (label, val) in &stats {
        println!(
            "  {} {:>3}  {}",
            label.dimmed(),
            val.to_string().bold(),
            stat_bar(*val, max, bar_w),
        );
    }
    println!("  {} {}", "BST".dimmed(), p.bst().to_string().bold(),);
    println!();

    // Type effectiveness
    let (weak_2x, weak_4x) = chart.pokemon_weaknesses(&p.types);
    let strengths = pokemon_offensive_strengths(&p.types, &chart);
    let resistances = pokemon_resistances(&p.types, &chart);
    let immunities = pokemon_immunities(&p.types, &chart);

    if !weak_4x.is_empty() {
        println!(
            "  {} {}",
            "4x weak to:".red().bold(),
            colored_type_list(&weak_4x)
        );
    }
    if !weak_2x.is_empty() {
        println!(
            "  {} {}",
            "2x weak to:".yellow(),
            colored_type_list(&weak_2x)
        );
    }
    if !resistances.is_empty() {
        println!(
            "  {} {}",
            "Resists:".green(),
            colored_type_list(&resistances)
        );
    }
    if !immunities.is_empty() {
        println!(
            "  {} {}",
            "Immune to:".cyan().bold(),
            colored_type_list(&immunities)
        );
    }
    if !strengths.is_empty() {
        println!(
            "  {} {}",
            "Strong vs:".green().bold(),
            colored_type_list(&strengths)
        );
    }
    println!();
}

pub fn print_team_plans(plans: &[pokeplanner_core::TeamPlan]) {
    if plans.is_empty() {
        println!("No team plans generated.");
        return;
    }

    let chart = pokeplanner_service::type_chart::TypeChart::fallback();

    for (i, plan) in plans.iter().enumerate() {
        let rank = i + 1;
        println!();
        println!(
            "{}",
            format!(
                "=== Team #{rank} (score: {:.3}, BST: {}) ===",
                plan.score, plan.total_bst
            )
            .bold()
        );
        println!();

        // Header — pad plain text first, then apply bold
        println!(
            "  {}  {}  {}  {}  {}  {}  {}  {}  {}",
            format!("{:<22}", "Pokemon").bold(),
            format!("{:<18}", "Types").bold(),
            format!("{:>5}", "BST").bold(),
            format!("{:>3}", "HP").bold(),
            format!("{:>3}", "Atk").bold(),
            format!("{:>3}", "Def").bold(),
            format!("{:>3}", "SpA").bold(),
            format!("{:>3}", "SpD").bold(),
            format!("{:>3}", "Spe").bold(),
        );
        println!("  {}", "-".repeat(78).dimmed());

        for member in &plan.team {
            let p = &member.pokemon;

            // Build the types string: pad the plain text width, then colorize
            let types_plain: Vec<String> = p.types.iter().map(|t| t.to_string()).collect();
            let types_plain_joined = types_plain.join("/");
            let types_colored: Vec<String> = p
                .types
                .iter()
                .map(|t| format!("{}", color_type(t)))
                .collect();
            let types_colored_joined = types_colored.join("/");
            // Compute how many pad chars we need to reach column width 18
            let pad_needed = 18usize.saturating_sub(types_plain_joined.len());
            let types_padded = format!("{types_colored_joined}{}", " ".repeat(pad_needed));

            let name_display = if p.is_default_form {
                p.form_name.clone()
            } else {
                format!("{} *", p.form_name)
            };

            println!(
                "  {:<22}  {}  {:>5}  {:>3}  {:>3}  {:>3}  {:>3}  {:>3}  {:>3}",
                name_display,
                types_padded,
                p.bst().to_string().bold(),
                p.stats.hp,
                p.stats.attack,
                p.stats.defense,
                p.stats.special_attack,
                p.stats.special_defense,
                p.stats.speed,
            );

            // Per-pokemon weaknesses
            let mut weakness_parts = Vec::new();
            if !member.weaknesses_4x.is_empty() {
                let list = colored_type_list(&member.weaknesses_4x);
                weakness_parts.push(format!("{} {list}", "4x:".red().bold()));
            }
            if !member.weaknesses_2x.is_empty() {
                let list = colored_type_list(&member.weaknesses_2x);
                weakness_parts.push(format!("{} {list}", "2x:".yellow()));
            }
            if !weakness_parts.is_empty() {
                println!("  {:<25} {}", "", weakness_parts.join("  "));
            }

            // Per-pokemon offensive strengths (types hit SE by STAB)
            let strengths = pokemon_offensive_strengths(&p.types, &chart);
            if !strengths.is_empty() {
                println!(
                    "  {:<25} {} {}",
                    "",
                    "Strong vs:".green().bold(),
                    colored_type_list(&strengths)
                );
            }

            // Recommended moves (with role annotations and damage class)
            if let Some(ref moves) = member.recommended_moves {
                if !moves.is_empty() {
                    let formatted = format_recommended_moves(moves);
                    // First line includes "Moves (class):" prefix
                    let damage_class = &moves[0].damage_class;
                    let prefix = format!("{} ", format!("Moves ({damage_class}):").bold());
                    // Wrap moves across lines at ~80 chars
                    let indent = " ".repeat(27);
                    let max_line = 80usize.saturating_sub(27);
                    let mut lines: Vec<String> = Vec::new();
                    let mut current_line = String::new();
                    for (i, part) in formatted.iter().enumerate() {
                        let separator = if i > 0 { ", " } else { "" };
                        let plain_len = strip_ansi(&current_line).len()
                            + strip_ansi(part).len()
                            + separator.len();
                        if !current_line.is_empty() && plain_len > max_line {
                            lines.push(current_line);
                            current_line = part.clone();
                        } else {
                            current_line = format!("{current_line}{separator}{part}");
                        }
                    }
                    if !current_line.is_empty() {
                        lines.push(current_line);
                    }
                    for (j, line) in lines.iter().enumerate() {
                        if j == 0 {
                            println!("  {:<25} {prefix}{line}", "");
                        } else {
                            println!("  {indent}  {line}");
                        }
                    }
                    if let Some(ref source_vg) = member.learnset_source_vg {
                        println!(
                            "  {:<25} {}",
                            "",
                            format!("(moves from: {source_vg})").dimmed()
                        );
                    }
                }
            }
        }

        // Coverage summary
        let cov = &plan.type_coverage;
        println!();
        let pct = cov.coverage_score * 100.0;
        let pct_display = if pct >= 80.0 {
            format!("{pct:.0}%").green()
        } else if pct >= 50.0 {
            format!("{pct:.0}%").yellow()
        } else {
            format!("{pct:.0}%").red()
        };
        println!(
            "  {} {pct_display} ({}/18 types)",
            "Offensive coverage:".bold(),
            cov.offensive_coverage.len()
        );

        if !cov.offensive_coverage.is_empty() {
            println!(
                "    {} {}",
                "SE against:".dimmed(),
                colored_type_list(&cov.offensive_coverage)
            );
        }

        if !cov.uncovered_types.is_empty() {
            println!(
                "    {} {}",
                "No SE into:".dimmed(),
                colored_type_list(&cov.uncovered_types)
            );
        }

        if !cov.defensive_weaknesses.is_empty() {
            println!(
                "    {} {}",
                "Shared weaknesses:".dimmed(),
                colored_type_list(&cov.defensive_weaknesses)
            );
        }

        // Move coverage summary
        match &cov.move_coverage {
            MoveCoverage::Available { types: move_cov } => {
                let covered_count = move_cov.len();
                let total_types = PokemonType::ALL.len();
                let pct = (covered_count as f64 / total_types as f64) * 100.0;
                let pct_display = if pct >= 80.0 {
                    format!("{pct:.0}%").green()
                } else if pct >= 50.0 {
                    format!("{pct:.0}%").yellow()
                } else {
                    format!("{pct:.0}%").red()
                };
                println!(
                    "  {} {pct_display} ({covered_count}/{total_types} types hit SE by moves)",
                    "Move coverage:".bold(),
                );

                let uncovered_by_moves: Vec<PokemonType> = PokemonType::ALL
                    .iter()
                    .filter(|t| !move_cov.contains(t))
                    .copied()
                    .collect();
                if !uncovered_by_moves.is_empty() {
                    println!(
                        "    {} {}",
                        "Not covered by moves:".dimmed(),
                        colored_type_list(&uncovered_by_moves)
                    );
                }
            }
            MoveCoverage::Unavailable { version_groups } => {
                println!(
                    "  {} {}",
                    "Move coverage:".bold(),
                    format!(
                        "No learnset data available for {}",
                        version_groups.join(", ")
                    )
                    .dimmed()
                );
            }
            MoveCoverage::NotAttempted => {}
        }
    }
    println!();
}

pub fn print_move_detail(m: &pokeplanner_core::Move) {
    println!();
    println!("  {}", m.name.bold());
    print!("  Type: {} ", color_type(&m.move_type));
    print!("{}", format!("Class: {}", m.damage_class).dimmed());
    if m.priority != 0 {
        print!(" Priority: {:+}", m.priority);
    }
    println!();
    println!(
        "  Power: {} Accuracy: {} PP: {}",
        m.power.map(|p| p.to_string()).unwrap_or("-".into()),
        m.accuracy.map(|a| format!("{a}%")).unwrap_or("-".into()),
        m.pp.map(|p| p.to_string()).unwrap_or("-".into()),
    );
    if let Some(ref effect) = m.effect {
        println!();
        println!("  {}", effect.dimmed());
    }
    println!();
}

/// Deduplicate learnset entries by move name, keeping the first occurrence
/// (best learn method due to sorting order: level-up before machine).
pub fn dedup_learnset(
    entries: Vec<pokeplanner_core::DetailedLearnsetEntry>,
) -> Vec<pokeplanner_core::DetailedLearnsetEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|e| seen.insert(e.move_details.name.clone()))
        .collect()
}

pub fn print_learnset(entries: &[pokeplanner_core::DetailedLearnsetEntry]) {
    if entries.is_empty() {
        println!("  {}", "(no moves found)".dimmed());
        return;
    }

    println!();
    println!(
        "  {:<4} {:<22} {:<11} {:>5} {:>5} {:>4}  {}",
        "Lvl".bold(),
        "Move".bold(),
        "Type".bold(),
        "Pwr".bold(),
        "Acc".bold(),
        "PP".bold(),
        "Method".bold(),
    );
    println!("  {}", "-".repeat(72).dimmed());

    for entry in entries {
        let m = &entry.move_details;
        let lvl = if entry.level > 0 {
            format!("{:>3}", entry.level)
        } else {
            "  -".to_string()
        };
        let power = m.power.map(|p| format!("{p:>5}")).unwrap_or("    -".into());
        let acc = m
            .accuracy
            .map(|a| format!("{a:>4}%"))
            .unwrap_or("    -".into());
        let pp = m.pp.map(|p| format!("{p:>4}")).unwrap_or("   -".into());

        // Build the type string with color but pad to fixed width
        let type_plain = format!("{}", m.move_type);
        let type_colored = format!("{}", color_type(&m.move_type));
        let type_pad = 11usize.saturating_sub(type_plain.len());
        let type_display = format!("{type_colored}{}", " ".repeat(type_pad));

        let class_marker = match m.damage_class.as_str() {
            "physical" => "P",
            "special" => "S",
            "status" => "-",
            _ => "?",
        };

        println!(
            "  {lvl}  {:<22} {} {power} {acc} {pp}  {} {}",
            m.name,
            type_display,
            entry.learn_method,
            class_marker.dimmed(),
        );
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_recommended_moves_stab_and_coverage() {
        let moves = vec![
            RecommendedMove {
                move_name: "fire-blast".into(),
                move_type: PokemonType::Fire,
                power: 110,
                damage_class: "special".into(),
                role: MoveRole::Stab,
            },
            RecommendedMove {
                move_name: "air-slash".into(),
                move_type: PokemonType::Flying,
                power: 75,
                damage_class: "special".into(),
                role: MoveRole::Stab,
            },
            RecommendedMove {
                move_name: "dragon-pulse".into(),
                move_type: PokemonType::Dragon,
                power: 85,
                damage_class: "special".into(),
                role: MoveRole::WeaknessCoverage(vec![PokemonType::Rock, PokemonType::Dragon]),
            },
            RecommendedMove {
                move_name: "solar-beam".into(),
                move_type: PokemonType::Grass,
                power: 120,
                damage_class: "special".into(),
                role: MoveRole::WeaknessCoverage(vec![
                    PokemonType::Water,
                    PokemonType::Rock,
                    PokemonType::Ground,
                ]),
            },
        ];
        let output = format_recommended_moves(&moves);
        let plain: Vec<String> = output.iter().map(|s| strip_ansi(s)).collect();
        assert!(plain[0].contains("fire-blast"));
        assert!(plain[0].contains("STAB"));
        assert!(plain[1].contains("air-slash"));
        assert!(plain[1].contains("STAB"));
        assert!(plain[2].contains("dragon-pulse"));
        assert!(plain[2].contains("->Rock,Dragon"));
        assert!(plain[3].contains("solar-beam"));
        assert!(plain[3].contains("->Water,Rock,Ground"));
    }

    #[test]
    fn test_format_recommended_moves_none_graceful() {
        let output = format_recommended_moves(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_recommended_moves_mirror_coverage() {
        let moves = vec![RecommendedMove {
            move_name: "shadow-ball".into(),
            move_type: PokemonType::Ghost,
            power: 80,
            damage_class: "special".into(),
            role: MoveRole::MirrorCoverage,
        }];
        let output = format_recommended_moves(&moves);
        let plain = strip_ansi(&output[0]);
        assert!(plain.contains("shadow-ball"));
        assert!(plain.contains("mirror"));
    }

    #[test]
    fn test_format_recommended_moves_weakness_coverage_notation() {
        let moves = vec![RecommendedMove {
            move_name: "ice-beam".into(),
            move_type: PokemonType::Ice,
            power: 90,
            damage_class: "special".into(),
            role: MoveRole::WeaknessCoverage(vec![PokemonType::Ground]),
        }];
        let output = format_recommended_moves(&moves);
        let plain = strip_ansi(&output[0]);
        assert!(plain.contains("->Ground"));
    }

    #[test]
    fn test_capitalize_type() {
        assert_eq!(capitalize_type(&PokemonType::Fire), "Fire");
        assert_eq!(capitalize_type(&PokemonType::Water), "Water");
        assert_eq!(capitalize_type(&PokemonType::Electric), "Electric");
    }

    #[test]
    fn test_strip_ansi() {
        let colored = "\x1b[31mfire\x1b[0m";
        assert_eq!(strip_ansi(colored), "fire");
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    #[test]
    fn test_pokemon_offensive_strengths_fire_flying() {
        let chart = pokeplanner_service::type_chart::TypeChart::fallback();
        let types = vec![PokemonType::Fire, PokemonType::Flying];
        let strengths = pokemon_offensive_strengths(&types, &chart);
        assert!(strengths.contains(&PokemonType::Grass));
        assert!(strengths.contains(&PokemonType::Ice));
        assert!(strengths.contains(&PokemonType::Bug));
        assert!(strengths.contains(&PokemonType::Steel));
        assert!(strengths.contains(&PokemonType::Fighting));
        assert!(!strengths.contains(&PokemonType::Water));
        assert!(!strengths.contains(&PokemonType::Dragon));
    }

    #[test]
    fn test_pokemon_resistances_steel() {
        let chart = pokeplanner_service::type_chart::TypeChart::fallback();
        let types = vec![PokemonType::Steel];
        let resistances = pokemon_resistances(&types, &chart);
        assert!(resistances.contains(&PokemonType::Normal));
        assert!(resistances.contains(&PokemonType::Fairy));
        assert!(resistances.contains(&PokemonType::Ice));
    }

    #[test]
    fn test_pokemon_immunities_ghost() {
        let chart = pokeplanner_service::type_chart::TypeChart::fallback();
        let types = vec![PokemonType::Ghost];
        let immunities = pokemon_immunities(&types, &chart);
        assert!(immunities.contains(&PokemonType::Normal));
        assert!(immunities.contains(&PokemonType::Fighting));
    }
}
