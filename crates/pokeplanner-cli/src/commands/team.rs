use colored::Colorize;
use pokeplanner_core::TeamPlanRequest;
use pokeplanner_service::PokePlannerService;

use crate::display::print_team_plans;

pub async fn handle_plan_team<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    service: &PokePlannerService<S, P>,
    request: TeamPlanRequest,
) -> anyhow::Result<()> {
    let job_id = service.submit_team_plan(request).await?;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let job = service.get_job(&job_id).await?;

        if let Some(progress) = &job.progress {
            eprint!(
                "\r  {} ({}/{})",
                progress.phase, progress.completed_steps, progress.total_steps
            );
        }

        match job.status {
            pokeplanner_core::JobStatus::Completed => {
                eprintln!();
                if let Some(result) = &job.result {
                    println!("{}", result.message.dimmed());
                    if let Some(data) = &result.data {
                        let plans: Vec<pokeplanner_core::TeamPlan> =
                            serde_json::from_value(data.clone()).unwrap_or_default();
                        print_team_plans(&plans);
                    }
                }
                break;
            }
            pokeplanner_core::JobStatus::Failed => {
                eprintln!();
                if let Some(result) = &job.result {
                    eprintln!("{} {}", "Error:".red().bold(), result.message);
                }
                break;
            }
            _ => continue,
        }
    }
    Ok(())
}

pub async fn handle_analyze_team<
    S: pokeplanner_storage::Storage,
    P: pokeplanner_pokeapi::PokeApiClient,
>(
    service: &PokePlannerService<S, P>,
    pokemon: Vec<String>,
    no_cache: bool,
) -> anyhow::Result<()> {
    let coverage = service.analyze_team(pokemon, no_cache).await?;
    println!("{}", serde_json::to_string_pretty(&coverage)?);
    Ok(())
}
