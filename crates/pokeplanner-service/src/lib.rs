pub mod team_planner;
pub mod type_chart;

use std::sync::Arc;

use chrono::Utc;
use pokeplanner_core::{
    AppError, HealthResponse, Job, JobId, JobKind, JobProgress, JobResult, JobStatus, Pokemon,
    PokemonType, SortField, SortOrder, TeamPlanRequest, TeamSource, TypeCoverage,
};
use pokeplanner_pokeapi::{PokeApiClient, VersionGroupInfo};
use pokeplanner_storage::Storage;
use tracing::{info, warn};

use crate::team_planner::TeamPlanner;
use crate::type_chart::TypeChart;

pub struct PokePlannerService<S: Storage, P: PokeApiClient> {
    storage: Arc<S>,
    pokeapi: Arc<P>,
}

impl<S: Storage, P: PokeApiClient> PokePlannerService<S, P> {
    pub fn new(storage: Arc<S>, pokeapi: Arc<P>) -> Self {
        Self { storage, pokeapi }
    }

    pub fn health(&self) -> HealthResponse {
        HealthResponse::ok()
    }

    /// No-op service call — placeholder for future business logic.
    pub async fn noop(&self) -> Result<String, AppError> {
        info!("noop called");
        Ok("noop".to_string())
    }

    // --- Job management ---

    /// Submit a generic long-running job. Returns the job ID immediately.
    pub async fn submit_job(&self) -> Result<JobId, AppError> {
        let job = Job::new();
        let job_id = job.id;
        self.storage.save_job(&job).await?;

        let storage = Arc::clone(&self.storage);
        tokio::spawn(async move {
            Self::run_generic_job(storage, job_id).await;
        });

        info!(job_id = %job_id, "job submitted");
        Ok(job_id)
    }

    /// Retrieve a job by ID.
    pub async fn get_job(&self, id: &JobId) -> Result<Job, AppError> {
        self.storage.get_job(id).await
    }

    /// List all jobs.
    pub async fn list_jobs(&self) -> Result<Vec<Job>, AppError> {
        self.storage.list_jobs().await
    }

    async fn run_generic_job(storage: Arc<S>, job_id: JobId) {
        if let Ok(mut job) = storage.get_job(&job_id).await {
            job.status = JobStatus::Running;
            job.updated_at = Utc::now();
            let _ = storage.update_job(&job).await;

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            job.status = JobStatus::Completed;
            job.updated_at = Utc::now();
            job.result = Some(JobResult {
                message: "Job completed successfully".to_string(),
                data: None,
            });
            let _ = storage.update_job(&job).await;
            info!(job_id = %job_id, "job completed");
        }
    }

    // --- PokeAPI features ---

    /// List available version groups (games).
    pub async fn list_version_groups(
        &self,
        no_cache: bool,
    ) -> Result<Vec<VersionGroupInfo>, AppError> {
        self.pokeapi.get_version_groups(no_cache).await
    }

    /// Get pokemon available in a game, with optional filtering, sorting, and limit.
    pub async fn get_game_pokemon(
        &self,
        version_group: &str,
        min_bst: Option<u32>,
        no_cache: bool,
        sort_by: Option<SortField>,
        sort_order: SortOrder,
        include_variants: bool,
        limit: Option<usize>,
    ) -> Result<Vec<Pokemon>, AppError> {
        let pokemon = self
            .pokeapi
            .get_game_pokemon(version_group, no_cache, include_variants)
            .await?;
        Ok(filter_sort_limit(pokemon, min_bst, sort_by, sort_order, limit))
    }

    /// Get pokemon from a specific pokedex (e.g., "national" for all pokemon).
    pub async fn get_pokedex_pokemon(
        &self,
        pokedex_name: &str,
        min_bst: Option<u32>,
        no_cache: bool,
        sort_by: Option<SortField>,
        sort_order: SortOrder,
        include_variants: bool,
        limit: Option<usize>,
    ) -> Result<Vec<Pokemon>, AppError> {
        let pokemon = self
            .pokeapi
            .get_pokedex_pokemon(pokedex_name, no_cache, include_variants)
            .await?;
        Ok(filter_sort_limit(pokemon, min_bst, sort_by, sort_order, limit))
    }

    /// Get a single pokemon by name.
    pub async fn get_pokemon(
        &self,
        name: &str,
        no_cache: bool,
    ) -> Result<Pokemon, AppError> {
        self.pokeapi.get_pokemon(name, no_cache).await
    }

    /// Submit a team planning job. Returns the job ID immediately.
    pub async fn submit_team_plan(
        &self,
        request: TeamPlanRequest,
    ) -> Result<JobId, AppError> {
        let job = Job::with_kind(JobKind::TeamPlan(request.clone()));
        let job_id = job.id;
        self.storage.save_job(&job).await?;

        let storage = Arc::clone(&self.storage);
        let pokeapi = Arc::clone(&self.pokeapi);
        tokio::spawn(async move {
            Self::run_team_plan_job(storage, pokeapi, job_id, request).await;
        });

        info!(job_id = %job_id, "team plan job submitted");
        Ok(job_id)
    }

    async fn run_team_plan_job(
        storage: Arc<S>,
        pokeapi: Arc<P>,
        job_id: JobId,
        request: TeamPlanRequest,
    ) {
        // Mark as running
        let mut job = match storage.get_job(&job_id).await {
            Ok(j) => j,
            Err(e) => {
                warn!("Failed to get job {job_id}: {e}");
                return;
            }
        };
        job.status = JobStatus::Running;
        job.updated_at = Utc::now();
        job.progress = Some(JobProgress {
            phase: "Fetching pokemon data".to_string(),
            completed_steps: 0,
            total_steps: 3,
        });
        let _ = storage.update_job(&job).await;

        // Step 1: Fetch candidate pokemon
        let candidates = match &request.source {
            TeamSource::Game { version_group } => {
                match pokeapi
                    .get_game_pokemon(version_group, request.no_cache, request.include_variants)
                    .await
                {
                    Ok(pokemon) => pokemon,
                    Err(e) => {
                        Self::fail_job(&storage, &mut job, &format!("Failed to fetch game pokemon: {e}")).await;
                        return;
                    }
                }
            }
            TeamSource::Pokedex { pokedex_name } => {
                match pokeapi
                    .get_pokedex_pokemon(pokedex_name, request.no_cache, request.include_variants)
                    .await
                {
                    Ok(pokemon) => pokemon,
                    Err(e) => {
                        Self::fail_job(&storage, &mut job, &format!("Failed to fetch pokedex pokemon: {e}")).await;
                        return;
                    }
                }
            }
            TeamSource::Custom { pokemon_names } => {
                let mut pokemon_list = Vec::new();
                for name in pokemon_names {
                    match pokeapi.get_pokemon(name, request.no_cache).await {
                        Ok(p) => pokemon_list.push(p),
                        Err(e) => warn!("Skipping {name}: {e}"),
                    }
                }
                pokemon_list
            }
        };

        // Update progress
        job.progress = Some(JobProgress {
            phase: "Filtering candidates".to_string(),
            completed_steps: 1,
            total_steps: 3,
        });
        job.updated_at = Utc::now();
        let _ = storage.update_job(&job).await;

        // Step 2: Apply BST filter
        let mut filtered = candidates;
        if let Some(min_bst) = request.min_bst {
            filtered.retain(|p| p.bst() >= min_bst);
        }

        if filtered.is_empty() {
            Self::fail_job(&storage, &mut job, "No candidates remaining after filtering").await;
            return;
        }

        // Step 3: Fetch type chart and run planner
        job.progress = Some(JobProgress {
            phase: "Computing optimal teams".to_string(),
            completed_steps: 2,
            total_steps: 3,
        });
        job.updated_at = Utc::now();
        let _ = storage.update_job(&job).await;

        let type_chart = match pokeapi.get_type_chart(request.no_cache).await {
            Ok(data) => TypeChart::from_api_data(&data),
            Err(e) => {
                warn!("Failed to fetch type chart, using fallback: {e}");
                TypeChart::fallback()
            }
        };

        // Resolve counter-team if specified
        let counter_pokemon = if let Some(ref names) = request.counter_team {
            let mut enemies = Vec::new();
            for name in names {
                match pokeapi.get_pokemon(name, request.no_cache).await {
                    Ok(p) => enemies.push(p),
                    Err(e) => warn!("Skipping counter-team member {name}: {e}"),
                }
            }
            Some(enemies)
        } else {
            None
        };

        let top_k = request.top_k.unwrap_or(5);
        let mut planner = TeamPlanner::new(&type_chart);
        if let Some(ref enemies) = counter_pokemon {
            planner = planner.with_counter_team(enemies);
        }
        let plans = planner.plan_teams(&filtered, top_k);

        // Complete
        job.status = JobStatus::Completed;
        job.updated_at = Utc::now();
        job.progress = Some(JobProgress {
            phase: "Complete".to_string(),
            completed_steps: 3,
            total_steps: 3,
        });
        job.result = Some(JobResult {
            message: format!(
                "Generated {} team plan(s) from {} candidates",
                plans.len(),
                filtered.len()
            ),
            data: serde_json::to_value(&plans).ok(),
        });
        let _ = storage.update_job(&job).await;
        info!(job_id = %job_id, "team plan job completed");
    }

    /// Synchronous team type coverage analysis.
    pub async fn analyze_team(
        &self,
        pokemon_names: Vec<String>,
        no_cache: bool,
    ) -> Result<TypeCoverage, AppError> {
        let mut team: Vec<Pokemon> = Vec::new();
        for name in &pokemon_names {
            team.push(self.pokeapi.get_pokemon(name, no_cache).await?);
        }

        let type_chart = match self.pokeapi.get_type_chart(no_cache).await {
            Ok(data) => TypeChart::from_api_data(&data),
            Err(e) => {
                warn!("Failed to fetch type chart, using fallback: {e}");
                TypeChart::fallback()
            }
        };

        let team_types: Vec<Vec<PokemonType>> = team.iter().map(|p| p.types.clone()).collect();

        let offensive_coverage: Vec<PokemonType> = PokemonType::ALL
            .iter()
            .filter(|&&target| {
                team_types.iter().any(|ptypes| {
                    ptypes
                        .iter()
                        .any(|&atk| type_chart.effectiveness(atk, target) >= 2.0)
                })
            })
            .copied()
            .collect();

        let defensive_weaknesses = type_chart.shared_weaknesses(&team_types);
        let uncovered_types = type_chart.uncovered_types(&team_types);
        let coverage_score = type_chart.team_offensive_coverage(&team_types);

        Ok(TypeCoverage {
            offensive_coverage,
            defensive_weaknesses,
            uncovered_types,
            coverage_score,
        })
    }

    async fn fail_job(storage: &Arc<S>, job: &mut Job, message: &str) {
        job.status = JobStatus::Failed;
        job.updated_at = Utc::now();
        job.result = Some(JobResult {
            message: message.to_string(),
            data: None,
        });
        let _ = storage.update_job(job).await;
        warn!(job_id = %job.id, "job failed: {message}");
    }
}

fn filter_sort_limit(
    mut pokemon: Vec<Pokemon>,
    min_bst: Option<u32>,
    sort_by: Option<SortField>,
    sort_order: SortOrder,
    limit: Option<usize>,
) -> Vec<Pokemon> {
    if let Some(min) = min_bst {
        pokemon.retain(|p| p.bst() >= min);
    }
    if let Some(field) = sort_by {
        sort_pokemon(&mut pokemon, field, sort_order);
    }
    if let Some(n) = limit {
        pokemon.truncate(n);
    }
    pokemon
}

fn sort_pokemon(pokemon: &mut [Pokemon], field: SortField, order: SortOrder) {
    pokemon.sort_by(|a, b| {
        let cmp = match field {
            SortField::Bst => a.bst().cmp(&b.bst()),
            SortField::Hp => a.stats.hp.cmp(&b.stats.hp),
            SortField::Attack => a.stats.attack.cmp(&b.stats.attack),
            SortField::Defense => a.stats.defense.cmp(&b.stats.defense),
            SortField::SpecialAttack => a.stats.special_attack.cmp(&b.stats.special_attack),
            SortField::SpecialDefense => a.stats.special_defense.cmp(&b.stats.special_defense),
            SortField::Speed => a.stats.speed.cmp(&b.stats.speed),
            SortField::Name => a.form_name.cmp(&b.form_name),
            SortField::PokedexNumber => a.pokedex_number.cmp(&b.pokedex_number),
        };
        match order {
            SortOrder::Asc => cmp,
            SortOrder::Desc => cmp.reverse(),
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pokeplanner_core::BaseStats;
    use pokeplanner_storage::JsonFileStorage;

    use super::*;

    // A mock PokeApiClient for testing (returns empty/fallback data)
    struct MockPokeApi;

    impl PokeApiClient for MockPokeApi {
        async fn get_version_groups(
            &self,
            _no_cache: bool,
        ) -> Result<Vec<VersionGroupInfo>, AppError> {
            Ok(vec![VersionGroupInfo {
                name: "test-game".to_string(),
                versions: vec!["test-v1".to_string()],
                pokedexes: vec!["test-dex".to_string()],
            }])
        }

        async fn get_game_pokemon(
            &self,
            _version_group: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![
                make_test_pokemon("pikachu", vec![PokemonType::Electric], 320),
                make_test_pokemon("charizard", vec![PokemonType::Fire, PokemonType::Flying], 534),
            ])
        }

        async fn get_pokemon(
            &self,
            name: &str,
            _no_cache: bool,
        ) -> Result<Pokemon, AppError> {
            Ok(make_test_pokemon(name, vec![PokemonType::Normal], 400))
        }

        async fn get_species_varieties(
            &self,
            species_name: &str,
            _no_cache: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![make_test_pokemon(
                species_name,
                vec![PokemonType::Normal],
                400,
            )])
        }

        async fn get_pokedex_pokemon(
            &self,
            _pokedex_name: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![
                make_test_pokemon("pikachu", vec![PokemonType::Electric], 320),
                make_test_pokemon("charizard", vec![PokemonType::Fire, PokemonType::Flying], 534),
                make_test_pokemon("mewtwo", vec![PokemonType::Psychic], 680),
            ])
        }

        async fn get_type_chart(
            &self,
            _no_cache: bool,
        ) -> Result<pokeplanner_pokeapi::TypeEffectivenessData, AppError> {
            // Return empty data — TypeChart::from_api_data will default to 1.0 for all
            Ok(pokeplanner_pokeapi::TypeEffectivenessData {
                entries: Vec::new(),
            })
        }
    }

    fn make_test_pokemon(name: &str, types: Vec<PokemonType>, bst: u32) -> Pokemon {
        let per = bst / 6;
        Pokemon {
            species_name: name.to_string(),
            form_name: name.to_string(),
            pokedex_number: 1,
            types,
            stats: BaseStats {
                hp: per,
                attack: per,
                defense: per,
                special_attack: per,
                special_defense: per,
                speed: per,
            },
            is_default_form: true,
        }
    }

    async fn make_service() -> PokePlannerService<JsonFileStorage, MockPokeApi> {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.keep()).await.unwrap());
        let pokeapi = Arc::new(MockPokeApi);
        PokePlannerService::new(storage, pokeapi)
    }

    #[tokio::test]
    async fn test_health() {
        let svc = make_service().await;
        let h = svc.health();
        assert_eq!(h.status, "ok");
    }

    #[tokio::test]
    async fn test_noop() {
        let svc = make_service().await;
        let result = svc.noop().await.unwrap();
        assert_eq!(result, "noop");
    }

    #[tokio::test]
    async fn test_submit_and_get_job() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap());
        let pokeapi = Arc::new(MockPokeApi);
        let svc = PokePlannerService::new(storage, pokeapi);

        let job_id = svc.submit_job().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        let job = svc.get_job(&job_id).await.unwrap();
        assert_eq!(job.id, job_id);
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap());
        let pokeapi = Arc::new(MockPokeApi);
        let svc = PokePlannerService::new(storage, pokeapi);

        svc.submit_job().await.unwrap();
        svc.submit_job().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let jobs = svc.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 2);
    }

    #[tokio::test]
    async fn test_list_version_groups() {
        let svc = make_service().await;
        let groups = svc.list_version_groups(false).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "test-game");
    }

    #[tokio::test]
    async fn test_get_game_pokemon() {
        let svc = make_service().await;
        let pokemon = svc
            .get_game_pokemon("test-game", None, false, None, SortOrder::Asc, true, None)
            .await
            .unwrap();
        assert_eq!(pokemon.len(), 2);
    }

    #[tokio::test]
    async fn test_get_game_pokemon_with_bst_filter() {
        let svc = make_service().await;
        let pokemon = svc
            .get_game_pokemon("test-game", Some(400), false, None, SortOrder::Asc, true, None)
            .await
            .unwrap();
        // Only charizard (534) should pass, pikachu (320) filtered out
        assert_eq!(pokemon.len(), 1);
        assert_eq!(pokemon[0].form_name, "charizard");
    }

    #[tokio::test]
    async fn test_get_game_pokemon_sorted() {
        let svc = make_service().await;
        let pokemon = svc
            .get_game_pokemon(
                "test-game",
                None,
                false,
                Some(SortField::Bst),
                SortOrder::Desc,
                true,
                None,
            )
            .await
            .unwrap();
        assert_eq!(pokemon[0].form_name, "charizard");
        assert_eq!(pokemon[1].form_name, "pikachu");
    }

    #[tokio::test]
    async fn test_get_pokemon() {
        let svc = make_service().await;
        let p = svc.get_pokemon("pikachu", false).await.unwrap();
        assert_eq!(p.form_name, "pikachu");
    }

    #[tokio::test]
    async fn test_analyze_team() {
        let svc = make_service().await;
        let coverage = svc
            .analyze_team(vec!["pikachu".to_string(), "charizard".to_string()], false)
            .await
            .unwrap();
        // With mock returning all Normal types and empty type chart, coverage will reflect that
        assert!(coverage.coverage_score >= 0.0);
    }
}
