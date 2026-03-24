use pokeplanner_core::{sort_pokemon, PokemonQueryParams, PokemonType};
use pokeplanner_service::PokePlannerService;

use crate::display::{dedup_learnset, print_learnset, print_pokemon_detail, print_pokemon_list};
use crate::unusable::UnusableStore;
use crate::PokemonAction;

/// Parse a stat filter string like "ge500", "lt100", "eq120".
/// Returns a closure that tests a u32 value against the filter.
pub fn parse_stat_filter(s: &str) -> anyhow::Result<Box<dyn Fn(u32) -> bool>> {
    let s = s.trim();
    let (op, val_str) = if let Some(rest) = s.strip_prefix("ge") {
        ("ge", rest)
    } else if let Some(rest) = s.strip_prefix("gt") {
        ("gt", rest)
    } else if let Some(rest) = s.strip_prefix("le") {
        ("le", rest)
    } else if let Some(rest) = s.strip_prefix("lt") {
        ("lt", rest)
    } else if let Some(rest) = s.strip_prefix("eq") {
        ("eq", rest)
    } else {
        // Default: treat bare number as "ge"
        ("ge", s)
    };

    let val: u32 = val_str.parse().map_err(|_| {
        anyhow::anyhow!("Invalid stat filter value: '{val_str}' (expected a number)")
    })?;

    Ok(match op {
        "ge" => Box::new(move |v| v >= val),
        "gt" => Box::new(move |v| v > val),
        "le" => Box::new(move |v| v <= val),
        "lt" => Box::new(move |v| v < val),
        "eq" => Box::new(move |v| v == val),
        _ => unreachable!(),
    })
}

pub async fn handle_pokemon_action<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    action: PokemonAction,
    service: &PokePlannerService<S, P>,
    unusable: &UnusableStore,
) -> anyhow::Result<()> {
    match action {
        PokemonAction::Show {
            name,
            no_cache,
            show_learnset,
            learnset_game,
        } => {
            let pokemon = service.get_pokemon(&name, no_cache).await?;
            print_pokemon_detail(&pokemon, unusable);

            // Show other forms/varieties of this species
            let varieties = service
                .get_species_varieties(&pokemon.species_name, no_cache)
                .await?;
            let others: Vec<&pokeplanner_core::Pokemon> = varieties
                .iter()
                .filter(|v| v.form_name != pokemon.form_name)
                .collect();
            if !others.is_empty() {
                use colored::Colorize;
                println!(
                    "  {} {}",
                    "Other forms:".bold(),
                    format!("({} total)", varieties.len()).dimmed(),
                );
                for v in &others {
                    print_pokemon_detail(v, unusable);
                }
            }

            // Show learnset if requested
            if show_learnset {
                let learnset = service
                    .get_pokemon_learnset_detailed(&name, learnset_game.as_deref(), no_cache)
                    .await?;
                let learnset = dedup_learnset(learnset);

                use colored::Colorize;
                let game_label = learnset_game.as_deref().unwrap_or("all games");
                println!();
                println!(
                    "  {} {}",
                    "Learnset".bold(),
                    format!("({game_label}, {} moves)", learnset.len()).dimmed(),
                );
                print_learnset(&learnset);
            }
        }
        PokemonAction::Search(crate::PokemonSearchArgs {
            game,
            pokedex,
            name,
            r#type,
            not_type,
            mono_type,
            dual_type,
            bst,
            hp,
            attack,
            defense,
            special_attack,
            special_defense,
            speed,
            default_only,
            variants_only,
            variant_type,
            sort_by,
            sort_order,
            limit,
            no_cache,
        }) => {
            // Step 1: Fetch candidate pokemon from source
            // Include all variants; we'll filter later
            let mut candidates = if let Some(games) = game {
                let mut all = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for g in &games {
                    let pokemon = service
                        .get_game_pokemon(
                            g,
                            &PokemonQueryParams {
                                no_cache,
                                include_variants: true,
                                ..Default::default()
                            },
                        )
                        .await?;
                    for p in pokemon {
                        if seen.insert(p.form_name.clone()) {
                            all.push(p);
                        }
                    }
                }
                all
            } else if let Some(ref dex) = pokedex {
                service
                    .get_pokedex_pokemon(
                        dex,
                        &PokemonQueryParams {
                            no_cache,
                            include_variants: true,
                            ..Default::default()
                        },
                    )
                    .await?
            } else {
                // Default: national pokedex
                service
                    .get_pokedex_pokemon(
                        "national",
                        &PokemonQueryParams {
                            no_cache,
                            include_variants: true,
                            ..Default::default()
                        },
                    )
                    .await?
            };

            // Step 2: Apply filters

            // Name substring filter
            if let Some(ref pattern) = name {
                let pattern_lower = pattern.to_lowercase();
                candidates.retain(|p| {
                    p.form_name.to_lowercase().contains(&pattern_lower)
                        || p.species_name.to_lowercase().contains(&pattern_lower)
                });
            }

            // Type inclusion filter: must have at least one of the specified types
            if let Some(ref type_names) = r#type {
                let types: Vec<PokemonType> = type_names
                    .iter()
                    .filter_map(|t| {
                        serde_json::from_value(serde_json::Value::String(t.to_lowercase())).ok()
                    })
                    .collect();
                if !types.is_empty() {
                    candidates.retain(|p| p.types.iter().any(|pt| types.contains(pt)));
                }
            }

            // Type exclusion filter: must NOT have any of the specified types
            if let Some(ref type_names) = not_type {
                let types: Vec<PokemonType> = type_names
                    .iter()
                    .filter_map(|t| {
                        serde_json::from_value(serde_json::Value::String(t.to_lowercase())).ok()
                    })
                    .collect();
                if !types.is_empty() {
                    candidates.retain(|p| !p.types.iter().any(|pt| types.contains(pt)));
                }
            }

            // Mono-type / dual-type
            if mono_type {
                candidates.retain(|p| p.types.len() == 1);
            }
            if dual_type {
                candidates.retain(|p| p.types.len() >= 2);
            }

            // Form filters
            if default_only {
                candidates.retain(|p| p.is_default_form);
            }
            if variants_only {
                candidates.retain(|p| !p.is_default_form);
            }
            if let Some(ref vt_keywords) = variant_type {
                candidates.retain(|p| {
                    if p.is_default_form {
                        return false;
                    }
                    let suffix = p
                        .form_name
                        .strip_prefix(&p.species_name)
                        .unwrap_or("")
                        .to_lowercase();
                    vt_keywords
                        .iter()
                        .any(|kw| suffix.contains(&kw.to_lowercase()))
                });
            }

            // Stat filters
            if let Some(ref f) = bst {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.bst()));
            }
            if let Some(ref f) = hp {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.hp));
            }
            if let Some(ref f) = attack {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.attack));
            }
            if let Some(ref f) = defense {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.defense));
            }
            if let Some(ref f) = special_attack {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.special_attack));
            }
            if let Some(ref f) = special_defense {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.special_defense));
            }
            if let Some(ref f) = speed {
                let pred = parse_stat_filter(f)?;
                candidates.retain(|p| pred(p.stats.speed));
            }

            // Step 3: Sort and limit
            if let Some(field) = sort_by {
                sort_pokemon(&mut candidates, field.into(), sort_order.into());
            }
            if let Some(n) = limit {
                candidates.truncate(n);
            }

            // Step 4: Display
            print_pokemon_list(&candidates, unusable);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stat_filter_ge() {
        let f = parse_stat_filter("ge500").unwrap();
        assert!(f(500));
        assert!(f(501));
        assert!(!f(499));
    }

    #[test]
    fn test_parse_stat_filter_gt() {
        let f = parse_stat_filter("gt100").unwrap();
        assert!(!f(100));
        assert!(f(101));
    }

    #[test]
    fn test_parse_stat_filter_le() {
        let f = parse_stat_filter("le50").unwrap();
        assert!(f(50));
        assert!(f(49));
        assert!(!f(51));
    }

    #[test]
    fn test_parse_stat_filter_lt() {
        let f = parse_stat_filter("lt200").unwrap();
        assert!(f(199));
        assert!(!f(200));
    }

    #[test]
    fn test_parse_stat_filter_eq() {
        let f = parse_stat_filter("eq120").unwrap();
        assert!(f(120));
        assert!(!f(119));
        assert!(!f(121));
    }

    #[test]
    fn test_parse_stat_filter_bare_number() {
        // Bare number defaults to ge
        let f = parse_stat_filter("500").unwrap();
        assert!(f(500));
        assert!(f(600));
        assert!(!f(499));
    }

    #[test]
    fn test_parse_stat_filter_invalid() {
        assert!(parse_stat_filter("geabc").is_err());
        assert!(parse_stat_filter("").is_err());
    }
}
