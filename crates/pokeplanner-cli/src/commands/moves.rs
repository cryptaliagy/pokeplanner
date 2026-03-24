use colored::Colorize;
use pokeplanner_core::PokemonType;
use pokeplanner_service::PokePlannerService;

use crate::display::{dedup_learnset, print_learnset, print_move_detail};
use crate::MoveAction;

pub async fn handle_move_action<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    action: MoveAction,
    service: &PokePlannerService<S, P>,
) -> anyhow::Result<()> {
    match action {
        MoveAction::Show { name, no_cache } => {
            let m = service.get_move(&name, no_cache).await?;
            print_move_detail(&m);
        }
        MoveAction::Search {
            pokemon,
            game,
            r#type,
            damage_class,
            min_power,
            learn_method,
            sort_by,
            no_cache,
        } => {
            let learnset = service
                .get_pokemon_learnset_detailed(&pokemon, game.as_deref(), no_cache)
                .await?;

            let mut filtered: Vec<&pokeplanner_core::DetailedLearnsetEntry> =
                learnset.iter().collect();

            // Filter by type
            if let Some(ref type_name) = r#type {
                if let Ok(t) = serde_json::from_value::<PokemonType>(serde_json::Value::String(
                    type_name.to_lowercase(),
                )) {
                    filtered.retain(|e| e.move_details.move_type == t);
                }
            }

            // Filter by damage class
            if let Some(ref dc) = damage_class {
                let dc_lower = dc.to_lowercase();
                filtered.retain(|e| e.move_details.damage_class == dc_lower);
            }

            // Filter by min power
            if let Some(min_pow) = min_power {
                filtered.retain(|e| e.move_details.power.unwrap_or(0) >= min_pow);
            }

            // Filter by learn method
            if let Some(ref method) = learn_method {
                let lm: pokeplanner_core::LearnMethod =
                    serde_json::from_value(serde_json::Value::String(method.clone()))
                        .unwrap_or(pokeplanner_core::LearnMethod::Other);
                filtered.retain(|e| e.learn_method == lm);
            }

            // Sort
            match sort_by.as_str() {
                "power" => filtered.sort_by(|a, b| {
                    b.move_details
                        .power
                        .unwrap_or(0)
                        .cmp(&a.move_details.power.unwrap_or(0))
                }),
                "accuracy" => filtered.sort_by(|a, b| {
                    b.move_details
                        .accuracy
                        .unwrap_or(0)
                        .cmp(&a.move_details.accuracy.unwrap_or(0))
                }),
                "pp" => filtered.sort_by(|a, b| {
                    b.move_details
                        .pp
                        .unwrap_or(0)
                        .cmp(&a.move_details.pp.unwrap_or(0))
                }),
                "name" => filtered.sort_by(|a, b| a.move_details.name.cmp(&b.move_details.name)),
                _ => {
                    // Default: sort by learn method then level
                    filtered.sort_by(|a, b| {
                        a.learn_method
                            .cmp(&b.learn_method)
                            .then(a.level.cmp(&b.level))
                            .then(a.move_details.name.cmp(&b.move_details.name))
                    });
                }
            }

            // Deduplicate and convert to owned for print
            let owned: Vec<pokeplanner_core::DetailedLearnsetEntry> =
                filtered.into_iter().cloned().collect();
            let owned = dedup_learnset(owned);

            let game_label = game.as_deref().unwrap_or("all games");
            println!(
                "{} {} {}",
                format!("{} moves found", owned.len()).bold(),
                format!("for {pokemon}").dimmed(),
                format!("({game_label})").dimmed(),
            );
            print_learnset(&owned);
        }
    }
    Ok(())
}
