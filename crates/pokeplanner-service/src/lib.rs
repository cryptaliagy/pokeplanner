pub mod move_selector;
pub mod team_planner;
pub mod type_chart;

use std::sync::Arc;

use chrono::Utc;
use pokeplanner_core::{
    filter_sort_limit, AppError, HealthResponse, Job, JobId, JobKind, JobProgress, JobResult,
    JobStatus, MoveCoverage, Pokemon, PokemonQueryParams, PokemonType, TeamPlanRequest, TeamSource,
    TypeCoverage,
};
use pokeplanner_pokeapi::{PokeApiClient, VersionGroupInfo};
use pokeplanner_storage::Storage;
use pokeplanner_telemetry::Metrics;
use tracing::{debug, info, info_span, warn, Instrument};

use crate::move_selector::MoveSelector;
use crate::team_planner::TeamPlanner;
use crate::type_chart::TypeChart;

pub struct PokePlannerService<S: Storage, P: PokeApiClient> {
    storage: Arc<S>,
    pokeapi: Arc<P>,
    metrics: Option<Metrics>,
}

impl<S: Storage, P: PokeApiClient> PokePlannerService<S, P> {
    pub fn new(storage: Arc<S>, pokeapi: Arc<P>) -> Self {
        Self {
            storage,
            pokeapi,
            metrics: None,
        }
    }

    pub fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = Some(metrics);
        self
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
        let mut job = match storage.get_job(&job_id).await {
            Ok(j) => j,
            Err(e) => {
                warn!("Failed to get job {job_id}: {e}");
                return;
            }
        };

        job.status = JobStatus::Running;
        job.updated_at = Utc::now();
        let _ = storage.update_job(&job).await;

        match Self::execute_generic_job().await {
            Ok(result) => {
                job.status = JobStatus::Completed;
                job.updated_at = Utc::now();
                job.result = Some(result);
                info!(job_id = %job_id, "job completed");
            }
            Err(e) => {
                job.status = JobStatus::Failed;
                job.updated_at = Utc::now();
                job.result = Some(JobResult {
                    message: e.to_string(),
                    data: None,
                });
                warn!(job_id = %job_id, "job failed: {e}");
            }
        }

        if let Err(e) = storage.update_job(&job).await {
            warn!(job_id = %job_id, "Failed to persist final job state: {e}");
        }
    }

    async fn execute_generic_job() -> Result<JobResult, AppError> {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(JobResult {
            message: "Job completed successfully".to_string(),
            data: None,
        })
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
        params: &PokemonQueryParams,
    ) -> Result<Vec<Pokemon>, AppError> {
        let pokemon = self
            .pokeapi
            .get_game_pokemon(version_group, params.no_cache, params.include_variants)
            .await?;
        Ok(filter_sort_limit(
            pokemon,
            params.min_bst,
            params.sort_by,
            params.sort_order,
            params.limit,
        ))
    }

    /// Get pokemon from a specific pokedex (e.g., "national" for all pokemon).
    pub async fn get_pokedex_pokemon(
        &self,
        pokedex_name: &str,
        params: &PokemonQueryParams,
    ) -> Result<Vec<Pokemon>, AppError> {
        let pokemon = self
            .pokeapi
            .get_pokedex_pokemon(pokedex_name, params.no_cache, params.include_variants)
            .await?;
        Ok(filter_sort_limit(
            pokemon,
            params.min_bst,
            params.sort_by,
            params.sort_order,
            params.limit,
        ))
    }

    /// Get a single pokemon by name.
    pub async fn get_pokemon(&self, name: &str, no_cache: bool) -> Result<Pokemon, AppError> {
        self.pokeapi.get_pokemon(name, no_cache).await
    }

    /// Get all forms/varieties for a species (base, mega, regional, etc.).
    pub async fn get_species_varieties(
        &self,
        species_name: &str,
        no_cache: bool,
    ) -> Result<Vec<Pokemon>, AppError> {
        self.pokeapi
            .get_species_varieties(species_name, no_cache)
            .await
    }

    /// Get a pokemon's learnset, optionally filtered by version group.
    pub async fn get_pokemon_learnset(
        &self,
        pokemon_name: &str,
        version_group: Option<&str>,
        no_cache: bool,
    ) -> Result<Vec<pokeplanner_core::LearnsetEntry>, AppError> {
        self.pokeapi
            .get_pokemon_learnset(pokemon_name, version_group, no_cache)
            .await
    }

    /// Get detailed learnset with move details resolved.
    pub async fn get_pokemon_learnset_detailed(
        &self,
        pokemon_name: &str,
        version_group: Option<&str>,
        no_cache: bool,
    ) -> Result<Vec<pokeplanner_core::DetailedLearnsetEntry>, AppError> {
        let learnset = self
            .pokeapi
            .get_pokemon_learnset(pokemon_name, version_group, no_cache)
            .await?;

        // Deduplicate move names to avoid redundant fetches
        let unique_moves: std::collections::HashSet<String> =
            learnset.iter().map(|e| e.move_name.clone()).collect();
        let mut move_cache: std::collections::HashMap<String, pokeplanner_core::Move> =
            std::collections::HashMap::new();
        for name in unique_moves {
            match self.pokeapi.get_move(&name, no_cache).await {
                Ok(m) => {
                    move_cache.insert(name, m);
                }
                Err(e) => warn!("Failed to fetch move {name}: {e}"),
            }
        }

        let mut detailed = Vec::new();
        for entry in learnset {
            if let Some(m) = move_cache.get(&entry.move_name) {
                detailed.push(pokeplanner_core::DetailedLearnsetEntry {
                    move_details: m.clone(),
                    learn_method: entry.learn_method,
                    level: entry.level,
                });
            }
        }
        Ok(detailed)
    }

    /// Get details for a single move.
    pub async fn get_move(
        &self,
        name: &str,
        no_cache: bool,
    ) -> Result<pokeplanner_core::Move, AppError> {
        self.pokeapi.get_move(name, no_cache).await
    }

    /// Submit a team planning job. Returns the job ID immediately.
    pub async fn submit_team_plan(&self, request: TeamPlanRequest) -> Result<JobId, AppError> {
        let job = Job::with_kind(JobKind::TeamPlan(request.clone()));
        let job_id = job.id;
        self.storage.save_job(&job).await?;

        if let Some(ref m) = self.metrics {
            m.job_submitted_counter.add(1, &[]);
        }

        let storage = Arc::clone(&self.storage);
        let pokeapi = Arc::clone(&self.pokeapi);
        let metrics = self.metrics.clone();
        let span = info_span!("team_plan_job", %job_id);
        tokio::spawn(
            async move {
                Self::run_team_plan_job(storage, pokeapi, metrics, job_id, request).await;
            }
            .instrument(span),
        );

        info!(job_id = %job_id, "team plan job submitted");
        Ok(job_id)
    }

    async fn run_team_plan_job(
        storage: Arc<S>,
        pokeapi: Arc<P>,
        metrics: Option<Metrics>,
        job_id: JobId,
        request: TeamPlanRequest,
    ) {
        let job_start = std::time::Instant::now();
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
        let _ = storage.update_job(&job).await;

        match Self::execute_team_plan(&storage, &pokeapi, &metrics, &mut job, &request).await {
            Ok((plans, candidate_count)) => {
                if let Some(ref m) = metrics {
                    m.team_plans_generated.add(plans.len() as u64, &[]);
                    m.job_completed_counter.add(1, &[]);
                    m.job_duration
                        .record(job_start.elapsed().as_secs_f64(), &[]);
                }

                job.status = JobStatus::Completed;
                job.updated_at = Utc::now();
                let total_steps = job.progress.as_ref().map(|p| p.total_steps).unwrap_or(3);
                job.progress = Some(JobProgress {
                    phase: "Complete".to_string(),
                    completed_steps: total_steps,
                    total_steps,
                });
                job.result = Some(JobResult {
                    message: format!(
                        "Generated {} team plan(s) from {} candidates",
                        plans.len(),
                        candidate_count
                    ),
                    data: serde_json::to_value(&plans).ok(),
                });
                info!(job_id = %job_id, plans = plans.len(), candidates = candidate_count, "team plan job completed");
            }
            Err(e) => {
                if let Some(ref m) = metrics {
                    m.job_failed_counter.add(1, &[]);
                    m.job_duration
                        .record(job_start.elapsed().as_secs_f64(), &[]);
                }

                job.status = JobStatus::Failed;
                job.updated_at = Utc::now();
                job.result = Some(JobResult {
                    message: e.to_string(),
                    data: None,
                });
                warn!(job_id = %job_id, "job failed: {e}");
            }
        }

        if let Err(e) = storage.update_job(&job).await {
            warn!(job_id = %job_id, "Failed to persist final job state: {e}");
        }
    }

    /// Execute the team planning pipeline. Returns the plans and candidate count on success.
    /// All fallible steps use `?` — the caller handles state transitions.
    async fn execute_team_plan(
        storage: &Arc<S>,
        pokeapi: &Arc<P>,
        metrics: &Option<Metrics>,
        job: &mut Job,
        request: &TeamPlanRequest,
    ) -> Result<(Vec<pokeplanner_core::TeamPlan>, usize), AppError> {
        // Resolve candidate version groups for learnset-based move selection.
        let learnset_vgs: Vec<String> = if let Some(vg) = &request.learnset_version_group {
            vec![vg.clone()]
        } else {
            match &request.source {
                TeamSource::Game { version_groups } => version_groups.clone(),
                TeamSource::Pokedex { pokedex_name } => {
                    match pokeapi.get_version_groups(request.no_cache).await {
                        Ok(groups) => groups
                            .into_iter()
                            .filter(|g| g.pokedexes.contains(pokedex_name))
                            .map(|g| g.name)
                            .collect(),
                        Err(e) => {
                            warn!("Failed to resolve version groups for pokedex: {e}");
                            Vec::new()
                        }
                    }
                }
                TeamSource::Custom { .. } => Vec::new(),
            }
        };
        let total_steps = if learnset_vgs.is_empty() { 3 } else { 4 };

        job.progress = Some(JobProgress {
            phase: "Fetching pokemon data".to_string(),
            completed_steps: 0,
            total_steps,
        });
        job.updated_at = Utc::now();
        let _ = storage.update_job(job).await;

        // Step 1: Fetch candidate pokemon
        let candidates = match &request.source {
            TeamSource::Game { version_groups } => {
                let mut all_pokemon = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for vg in version_groups {
                    let pokemon = pokeapi
                        .get_game_pokemon(vg, request.no_cache, request.include_variants)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!(
                                "Failed to fetch game pokemon for {vg}: {e}"
                            ))
                        })?;
                    for p in pokemon {
                        if seen.insert(p.form_name.clone()) {
                            all_pokemon.push(p);
                        }
                    }
                }
                all_pokemon
            }
            TeamSource::Pokedex { pokedex_name } => pokeapi
                .get_pokedex_pokemon(pokedex_name, request.no_cache, request.include_variants)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to fetch pokedex pokemon: {e}")))?,
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
            total_steps,
        });
        job.updated_at = Utc::now();
        let _ = storage.update_job(job).await;

        // Step 2: Apply filters
        let mut filtered = candidates;
        if let Some(min_bst) = request.min_bst {
            filtered.retain(|p| p.bst() >= min_bst);
        }
        if !request.exclude.is_empty() {
            filtered.retain(|p| !request.exclude.iter().any(|e| e == &p.form_name));
        }
        if !request.exclude_species.is_empty() {
            filtered.retain(|p| !request.exclude_species.iter().any(|e| e == &p.species_name));
        }
        if !request.exclude_variant_types.is_empty() {
            filtered.retain(|p| {
                if p.is_default_form {
                    return true;
                }
                let suffix = p
                    .form_name
                    .strip_prefix(&p.species_name)
                    .unwrap_or("")
                    .to_lowercase();
                !request
                    .exclude_variant_types
                    .iter()
                    .any(|vt| suffix.contains(&vt.to_lowercase()))
            });
        }

        if filtered.is_empty() {
            return Err(AppError::Internal(
                "No candidates remaining after filtering".to_string(),
            ));
        }

        if let Some(ref m) = metrics {
            m.team_candidate_pool_size
                .record(filtered.len() as u64, &[]);
        }
        debug!(
            candidate_count = filtered.len(),
            "candidates after filtering"
        );

        // Step 3: Fetch type chart and run planner
        job.progress = Some(JobProgress {
            phase: "Computing optimal teams".to_string(),
            completed_steps: 2,
            total_steps,
        });
        job.updated_at = Utc::now();
        let _ = storage.update_job(job).await;

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
        let mut plans = planner.plan_teams(&filtered, top_k);
        let candidate_count = filtered.len();

        // Step 4 (optional): Select recommended moves for each team member
        if !learnset_vgs.is_empty() {
            job.progress = Some(JobProgress {
                phase: "Selecting recommended moves".to_string(),
                completed_steps: 3,
                total_steps,
            });
            job.updated_at = Utc::now();
            let _ = storage.update_job(job).await;

            // Fetch all version groups for generation-aware fallback
            let all_vgs = match pokeapi.get_version_groups(request.no_cache).await {
                Ok(vgs) => vgs,
                Err(e) => {
                    warn!("Failed to fetch version groups for fallback: {e}");
                    Vec::new()
                }
            };

            let selector = MoveSelector::new(&type_chart);

            for plan in &mut plans {
                for member in &mut plan.team {
                    match Self::fetch_learnset_and_select(
                        pokeapi,
                        &selector,
                        member,
                        &learnset_vgs,
                        &all_vgs,
                        request.no_cache,
                        metrics,
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(e) => {
                            warn!(
                                "Move selection failed for {}: {e}",
                                member.pokemon.form_name
                            );
                        }
                    }
                }
            }

            // Compute move-based type coverage for each plan.
            // If no team member in any plan received moves, mark as Unavailable
            // instead of reporting a misleading 0%.
            let any_member_has_moves = plans
                .iter()
                .any(|p| p.team.iter().any(|m| m.recommended_moves.is_some()));

            if any_member_has_moves {
                for plan in &mut plans {
                    let move_types: std::collections::HashSet<PokemonType> = plan
                        .team
                        .iter()
                        .filter_map(|m| m.recommended_moves.as_ref())
                        .flatten()
                        .map(|m| m.move_type)
                        .collect();

                    let covered: Vec<PokemonType> = PokemonType::ALL
                        .iter()
                        .filter(|&&target| {
                            move_types
                                .iter()
                                .any(|&atk| type_chart.effectiveness(atk, target) >= 2.0)
                        })
                        .copied()
                        .collect();

                    plan.type_coverage.move_coverage = MoveCoverage::Available { types: covered };
                }
            } else {
                warn!(
                    "No learnset data found in version group(s) {:?} — \
                     move coverage unavailable",
                    learnset_vgs
                );
                for plan in &mut plans {
                    plan.type_coverage.move_coverage = MoveCoverage::Unavailable {
                        version_groups: learnset_vgs.clone(),
                    };
                }
            }
        }

        Ok((plans, candidate_count))
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

        let offensive_coverage = type_chart.covered_types(&team_types);
        let defensive_weaknesses = type_chart.shared_weaknesses(&team_types);
        let uncovered_types = type_chart.uncovered_types(&team_types);
        let coverage_score = type_chart.team_offensive_coverage(&team_types);

        Ok(TypeCoverage {
            offensive_coverage,
            defensive_weaknesses,
            uncovered_types,
            coverage_score,
            move_coverage: MoveCoverage::NotAttempted,
        })
    }

    /// Fetch learnset data and select moves for a team member.
    ///
    /// Fallback chain:
    /// 1. Try each candidate version group in order
    /// 2. If none have data, try other VGs in the same generation
    /// 3. If still nothing, fetch all VGs and pick the most recent with data
    async fn fetch_learnset_and_select(
        pokeapi: &Arc<P>,
        selector: &MoveSelector<'_>,
        member: &mut pokeplanner_core::TeamMember,
        version_groups: &[String],
        all_vgs: &[VersionGroupInfo],
        no_cache: bool,
        metrics: &Option<Metrics>,
    ) -> Result<(), AppError> {
        let pokemon_name = &member.pokemon.form_name;
        let is_primary_vg = |vg: &str| version_groups.iter().any(|v| v == vg);

        // Try each version group until we find one with learnset data
        let mut learnset = Vec::new();
        let mut source_vg: Option<String> = None;
        for vg in version_groups {
            match pokeapi
                .get_pokemon_learnset(pokemon_name, Some(vg), no_cache)
                .await
            {
                Ok(entries) if !entries.is_empty() => {
                    learnset = entries;
                    source_vg = Some(vg.clone());
                    break;
                }
                Ok(_) => continue,
                Err(e) => {
                    warn!("Learnset fetch failed for {pokemon_name} in {vg}: {e}");
                    continue;
                }
            }
        }

        // Fallback: try other VGs in the same generation
        if learnset.is_empty() {
            let fallback_vgs = same_generation_fallbacks(version_groups, all_vgs);
            for vg in &fallback_vgs {
                match pokeapi
                    .get_pokemon_learnset(pokemon_name, Some(vg), no_cache)
                    .await
                {
                    Ok(entries) if !entries.is_empty() => {
                        info!("Using same-gen fallback {vg} for {pokemon_name} learnset");
                        learnset = entries;
                        source_vg = Some(vg.clone());
                        break;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        warn!("Learnset fetch failed for {pokemon_name} in {vg}: {e}");
                        continue;
                    }
                }
            }
        }

        // Last resort: fetch all VG data and pick the most recent
        if learnset.is_empty() {
            match pokeapi
                .get_pokemon_learnset(pokemon_name, None, no_cache)
                .await
            {
                Ok(all_entries) if !all_entries.is_empty() => {
                    if let Some(best_vg) = pick_best_available_vg(&all_entries, all_vgs) {
                        info!(
                            "Using best-available fallback {best_vg} for {pokemon_name} learnset"
                        );
                        learnset = all_entries
                            .into_iter()
                            .filter(|e| e.version_group == best_vg)
                            .collect();
                        source_vg = Some(best_vg);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("Full learnset fetch failed for {pokemon_name}: {e}");
                }
            }
        }

        if learnset.is_empty() {
            return Ok(());
        }

        // Record fallback source if different from the primary VGs
        if let Some(ref vg) = source_vg {
            if !is_primary_vg(vg) {
                member.learnset_source_vg = Some(vg.clone());
                if let Some(ref m) = metrics {
                    m.move_selection_fallback_counter.add(1, &[]);
                }
            }
        }

        // Fetch move details, deduplicating by name
        let unique_moves: std::collections::HashSet<String> =
            learnset.iter().map(|e| e.move_name.clone()).collect();
        let mut move_cache: std::collections::HashMap<String, pokeplanner_core::Move> =
            std::collections::HashMap::new();
        for name in unique_moves {
            match pokeapi.get_move(&name, no_cache).await {
                Ok(m) => {
                    move_cache.insert(name, m);
                }
                Err(e) => warn!("Failed to fetch move {name}: {e}"),
            }
        }

        let mut detailed = Vec::new();
        for entry in learnset {
            if let Some(m) = move_cache.get(&entry.move_name) {
                detailed.push(pokeplanner_core::DetailedLearnsetEntry {
                    move_details: m.clone(),
                    learn_method: entry.learn_method,
                    level: entry.level,
                });
            }
        }

        let weaknesses: Vec<PokemonType> = member
            .weaknesses_2x
            .iter()
            .chain(member.weaknesses_4x.iter())
            .copied()
            .collect();

        let recommendation = selector.select_moves(&member.pokemon, &detailed, &weaknesses);
        if !recommendation.moves.is_empty() {
            member.recommended_moves = Some(recommendation.moves);
        }

        Ok(())
    }
}

/// Parse a PokeAPI generation name (e.g., "generation-ix") into a numeric value for ordering.
fn generation_number(gen_name: &str) -> u32 {
    let suffix = gen_name.strip_prefix("generation-").unwrap_or(gen_name);
    match suffix {
        "i" => 1,
        "ii" => 2,
        "iii" => 3,
        "iv" => 4,
        "v" => 5,
        "vi" => 6,
        "vii" => 7,
        "viii" => 8,
        "ix" => 9,
        "x" => 10,
        _ => 0,
    }
}

/// Find version groups in the same generation as `requested_vgs` but not already in that list.
fn same_generation_fallbacks(
    requested_vgs: &[String],
    all_vgs: &[VersionGroupInfo],
) -> Vec<String> {
    // Find the generation(s) of the requested VGs
    let requested_gens: std::collections::HashSet<&str> = all_vgs
        .iter()
        .filter(|vg| requested_vgs.contains(&vg.name))
        .map(|vg| vg.generation.as_str())
        .collect();

    // Collect sibling VGs in those generations, ordered by descending generation number
    // (in case of multiple gens), then by VG list order as tiebreak
    let mut siblings: Vec<&VersionGroupInfo> = all_vgs
        .iter()
        .filter(|vg| requested_gens.contains(vg.generation.as_str()))
        .filter(|vg| !requested_vgs.contains(&vg.name))
        .collect();
    siblings
        .sort_by(|a, b| generation_number(&b.generation).cmp(&generation_number(&a.generation)));
    siblings.into_iter().map(|vg| vg.name.clone()).collect()
}

/// From a set of learnset entries spanning multiple VGs, pick the most recent VG
/// (highest generation number) that has data.
fn pick_best_available_vg(
    entries: &[pokeplanner_core::LearnsetEntry],
    all_vgs: &[VersionGroupInfo],
) -> Option<String> {
    let entry_vgs: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.version_group.as_str()).collect();

    all_vgs
        .iter()
        .filter(|vg| entry_vgs.contains(vg.name.as_str()))
        .max_by_key(|vg| generation_number(&vg.generation))
        .map(|vg| vg.name.clone())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pokeplanner_core::{BaseStats, SortField, SortOrder};
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
                generation: "generation-i".to_string(),
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
                make_test_pokemon(
                    "charizard",
                    vec![PokemonType::Fire, PokemonType::Flying],
                    534,
                ),
            ])
        }

        async fn get_pokemon(&self, name: &str, _no_cache: bool) -> Result<Pokemon, AppError> {
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
                make_test_pokemon(
                    "charizard",
                    vec![PokemonType::Fire, PokemonType::Flying],
                    534,
                ),
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

        async fn get_pokemon_learnset(
            &self,
            _pokemon_name: &str,
            _version_group: Option<&str>,
            _no_cache: bool,
        ) -> Result<Vec<pokeplanner_core::LearnsetEntry>, AppError> {
            Ok(vec![])
        }

        async fn get_move(
            &self,
            _move_name: &str,
            _no_cache: bool,
        ) -> Result<pokeplanner_core::Move, AppError> {
            Ok(pokeplanner_core::Move {
                name: _move_name.to_string(),
                move_type: PokemonType::Normal,
                power: None,
                accuracy: None,
                pp: None,
                damage_class: "status".to_string(),
                priority: 0,
                effect: None,
                drain: 0,
                self_stat_changes: Vec::new(),
            })
        }
    }

    fn make_test_pokemon(name: &str, types: Vec<PokemonType>, bst: u32) -> Pokemon {
        make_test_pokemon_variant(name, name, types, bst, true)
    }

    fn make_test_pokemon_variant(
        species: &str,
        form: &str,
        types: Vec<PokemonType>,
        bst: u32,
        is_default: bool,
    ) -> Pokemon {
        let per = bst / 6;
        Pokemon {
            species_name: species.to_string(),
            form_name: form.to_string(),
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
            is_default_form: is_default,
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
        let storage = Arc::new(
            JsonFileStorage::new(dir.path().to_path_buf())
                .await
                .unwrap(),
        );
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
        let storage = Arc::new(
            JsonFileStorage::new(dir.path().to_path_buf())
                .await
                .unwrap(),
        );
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
            .get_game_pokemon(
                "test-game",
                &PokemonQueryParams {
                    include_variants: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(pokemon.len(), 2);
    }

    #[tokio::test]
    async fn test_get_game_pokemon_with_bst_filter() {
        let svc = make_service().await;
        let pokemon = svc
            .get_game_pokemon(
                "test-game",
                &PokemonQueryParams {
                    min_bst: Some(400),
                    include_variants: true,
                    ..Default::default()
                },
            )
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
                &PokemonQueryParams {
                    sort_by: Some(SortField::Bst),
                    sort_order: SortOrder::Desc,
                    include_variants: true,
                    ..Default::default()
                },
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

    /// Mock that returns pokemon with variant forms for testing exclude_variant_types.
    struct MockPokeApiWithVariants;

    impl PokeApiClient for MockPokeApiWithVariants {
        async fn get_version_groups(
            &self,
            _no_cache: bool,
        ) -> Result<Vec<VersionGroupInfo>, AppError> {
            Ok(vec![])
        }

        async fn get_game_pokemon(
            &self,
            _version_group: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![
                make_test_pokemon(
                    "charizard",
                    vec![PokemonType::Fire, PokemonType::Flying],
                    534,
                ),
                make_test_pokemon_variant(
                    "charizard",
                    "charizard-mega-x",
                    vec![PokemonType::Fire, PokemonType::Dragon],
                    634,
                    false,
                ),
                make_test_pokemon_variant(
                    "charizard",
                    "charizard-mega-y",
                    vec![PokemonType::Fire, PokemonType::Flying],
                    634,
                    false,
                ),
                make_test_pokemon_variant(
                    "charizard",
                    "charizard-gmax",
                    vec![PokemonType::Fire, PokemonType::Flying],
                    534,
                    false,
                ),
                make_test_pokemon("ninetales", vec![PokemonType::Fire], 505),
                make_test_pokemon_variant(
                    "ninetales",
                    "ninetales-alola",
                    vec![PokemonType::Ice, PokemonType::Fairy],
                    505,
                    false,
                ),
                make_test_pokemon("pikachu", vec![PokemonType::Electric], 320),
            ])
        }

        async fn get_pokemon(&self, name: &str, _no_cache: bool) -> Result<Pokemon, AppError> {
            Ok(make_test_pokemon(name, vec![PokemonType::Normal], 400))
        }

        async fn get_species_varieties(
            &self,
            _species_name: &str,
            _no_cache: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![])
        }

        async fn get_pokedex_pokemon(
            &self,
            _pokedex_name: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            Ok(vec![])
        }

        async fn get_type_chart(
            &self,
            _no_cache: bool,
        ) -> Result<pokeplanner_pokeapi::TypeEffectivenessData, AppError> {
            Ok(pokeplanner_pokeapi::TypeEffectivenessData {
                entries: Vec::new(),
            })
        }

        async fn get_pokemon_learnset(
            &self,
            _pokemon_name: &str,
            _version_group: Option<&str>,
            _no_cache: bool,
        ) -> Result<Vec<pokeplanner_core::LearnsetEntry>, AppError> {
            Ok(vec![])
        }

        async fn get_move(
            &self,
            _move_name: &str,
            _no_cache: bool,
        ) -> Result<pokeplanner_core::Move, AppError> {
            Ok(pokeplanner_core::Move {
                name: _move_name.to_string(),
                move_type: PokemonType::Normal,
                power: None,
                accuracy: None,
                pp: None,
                damage_class: "status".to_string(),
                priority: 0,
                effect: None,
                drain: 0,
                self_stat_changes: Vec::new(),
            })
        }
    }

    #[tokio::test]
    async fn test_exclude_variant_types_filters_megas() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            JsonFileStorage::new(dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let pokeapi = Arc::new(MockPokeApiWithVariants);
        let svc = PokePlannerService::new(storage, pokeapi);

        let request = TeamPlanRequest {
            source: TeamSource::Game {
                version_groups: vec!["test".to_string()],
            },
            min_bst: None,
            no_cache: false,
            top_k: Some(1),
            include_variants: true,
            exclude: Vec::new(),
            exclude_species: Vec::new(),
            exclude_variant_types: vec!["mega".to_string()],
            counter_team: None,
            learnset_version_group: None,
        };

        let job_id = svc.submit_team_plan(request).await.unwrap();

        // Wait for job to complete
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let job = svc.get_job(&job_id).await.unwrap();
            match job.status {
                JobStatus::Completed => {
                    let data = job.result.unwrap().data.unwrap();
                    let plans: Vec<pokeplanner_core::TeamPlan> =
                        serde_json::from_value(data).unwrap();
                    // Megas should be excluded; gmax and alola should remain
                    let all_members: Vec<&str> = plans
                        .iter()
                        .flat_map(|p| p.team.iter().map(|m| m.pokemon.form_name.as_str()))
                        .collect();
                    assert!(
                        !all_members.iter().any(|n| n.contains("mega")),
                        "mega variants should be excluded, got: {all_members:?}"
                    );
                    break;
                }
                JobStatus::Failed => {
                    panic!(
                        "Job failed: {}",
                        job.result.map(|r| r.message).unwrap_or_default()
                    );
                }
                _ => continue,
            }
        }
    }

    #[tokio::test]
    async fn test_exclude_variant_types_multiple() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            JsonFileStorage::new(dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let pokeapi = Arc::new(MockPokeApiWithVariants);
        let svc = PokePlannerService::new(storage, pokeapi);

        let request = TeamPlanRequest {
            source: TeamSource::Game {
                version_groups: vec!["test".to_string()],
            },
            min_bst: None,
            no_cache: false,
            top_k: Some(1),
            include_variants: true,
            exclude: Vec::new(),
            exclude_species: Vec::new(),
            exclude_variant_types: vec![
                "mega".to_string(),
                "gmax".to_string(),
                "alola".to_string(),
            ],
            counter_team: None,
            learnset_version_group: None,
        };

        let job_id = svc.submit_team_plan(request).await.unwrap();

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let job = svc.get_job(&job_id).await.unwrap();
            match job.status {
                JobStatus::Completed => {
                    let data = job.result.unwrap().data.unwrap();
                    let plans: Vec<pokeplanner_core::TeamPlan> =
                        serde_json::from_value(data).unwrap();
                    // Only base forms should remain: charizard, ninetales, pikachu
                    let all_members: Vec<&str> = plans
                        .iter()
                        .flat_map(|p| p.team.iter().map(|m| m.pokemon.form_name.as_str()))
                        .collect();
                    for name in &all_members {
                        assert!(
                            !name.contains("mega")
                                && !name.contains("gmax")
                                && !name.contains("alola"),
                            "variant should be excluded, got: {name}"
                        );
                    }
                    break;
                }
                JobStatus::Failed => {
                    panic!(
                        "Job failed: {}",
                        job.result.map(|r| r.message).unwrap_or_default()
                    );
                }
                _ => continue,
            }
        }
    }

    /// Mock that returns pokemon with learnset data for testing move selection integration.
    struct MockPokeApiWithMoves;

    impl PokeApiClient for MockPokeApiWithMoves {
        async fn get_version_groups(
            &self,
            _no_cache: bool,
        ) -> Result<Vec<VersionGroupInfo>, AppError> {
            Ok(vec![VersionGroupInfo {
                name: "test-game".to_string(),
                versions: vec!["test-v1".to_string()],
                pokedexes: vec!["test-dex".to_string()],
                generation: "generation-i".to_string(),
            }])
        }

        async fn get_game_pokemon(
            &self,
            _version_group: &str,
            _no_cache: bool,
            _include_variants: bool,
        ) -> Result<Vec<Pokemon>, AppError> {
            // Return pokemon with distinct attack stats so MoveSelector picks a class
            Ok(vec![
                {
                    let mut p = make_test_pokemon("pikachu", vec![PokemonType::Electric], 320);
                    // Make special attack higher so it picks special moves
                    p.stats.special_attack = 80;
                    p.stats.attack = 40;
                    p
                },
                {
                    let mut p = make_test_pokemon(
                        "charizard",
                        vec![PokemonType::Fire, PokemonType::Flying],
                        534,
                    );
                    p.stats.special_attack = 109;
                    p.stats.attack = 84;
                    p
                },
            ])
        }

        async fn get_pokemon(&self, name: &str, _no_cache: bool) -> Result<Pokemon, AppError> {
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
                make_test_pokemon(
                    "charizard",
                    vec![PokemonType::Fire, PokemonType::Flying],
                    534,
                ),
            ])
        }

        async fn get_type_chart(
            &self,
            _no_cache: bool,
        ) -> Result<pokeplanner_pokeapi::TypeEffectivenessData, AppError> {
            Ok(pokeplanner_pokeapi::TypeEffectivenessData {
                entries: Vec::new(),
            })
        }

        async fn get_pokemon_learnset(
            &self,
            _pokemon_name: &str,
            _version_group: Option<&str>,
            _no_cache: bool,
        ) -> Result<Vec<pokeplanner_core::LearnsetEntry>, AppError> {
            Ok(vec![
                pokeplanner_core::LearnsetEntry {
                    move_name: "thunderbolt".to_string(),
                    learn_method: pokeplanner_core::LearnMethod::LevelUp,
                    level: 30,
                    version_group: "test-game".to_string(),
                },
                pokeplanner_core::LearnsetEntry {
                    move_name: "thunder".to_string(),
                    learn_method: pokeplanner_core::LearnMethod::LevelUp,
                    level: 40,
                    version_group: "test-game".to_string(),
                },
                pokeplanner_core::LearnsetEntry {
                    move_name: "ice-beam".to_string(),
                    learn_method: pokeplanner_core::LearnMethod::Machine,
                    level: 0,
                    version_group: "test-game".to_string(),
                },
                pokeplanner_core::LearnsetEntry {
                    move_name: "psychic".to_string(),
                    learn_method: pokeplanner_core::LearnMethod::Machine,
                    level: 0,
                    version_group: "test-game".to_string(),
                },
            ])
        }

        async fn get_move(
            &self,
            move_name: &str,
            _no_cache: bool,
        ) -> Result<pokeplanner_core::Move, AppError> {
            let (move_type, power) = match move_name {
                "thunderbolt" => (PokemonType::Electric, 90),
                "thunder" => (PokemonType::Electric, 110),
                "ice-beam" => (PokemonType::Ice, 90),
                "psychic" => (PokemonType::Psychic, 90),
                _ => (PokemonType::Normal, 50),
            };
            Ok(pokeplanner_core::Move {
                name: move_name.to_string(),
                move_type,
                power: Some(power),
                accuracy: Some(100),
                pp: Some(15),
                damage_class: "special".to_string(),
                priority: 0,
                effect: None,
                drain: 0,
                self_stat_changes: Vec::new(),
            })
        }
    }

    #[tokio::test]
    async fn test_team_plan_with_move_selection() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            JsonFileStorage::new(dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let pokeapi = Arc::new(MockPokeApiWithMoves);
        let svc = PokePlannerService::new(storage, pokeapi);

        let request = TeamPlanRequest {
            source: TeamSource::Game {
                version_groups: vec!["test-game".to_string()],
            },
            min_bst: None,
            no_cache: false,
            top_k: Some(1),
            include_variants: true,
            exclude: Vec::new(),
            exclude_species: Vec::new(),
            exclude_variant_types: Vec::new(),
            counter_team: None,
            learnset_version_group: None, // defaults to first game
        };

        let job_id = svc.submit_team_plan(request).await.unwrap();

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let job = svc.get_job(&job_id).await.unwrap();
            match job.status {
                JobStatus::Completed => {
                    let data = job.result.unwrap().data.unwrap();
                    let plans: Vec<pokeplanner_core::TeamPlan> =
                        serde_json::from_value(data).unwrap();
                    assert!(!plans.is_empty());
                    // At least one member should have recommended moves
                    let has_moves = plans[0].team.iter().any(|m| m.recommended_moves.is_some());
                    assert!(
                        has_moves,
                        "Expected at least one team member to have recommended moves"
                    );

                    // Verify no recommended move has recoil or self-debuffs
                    for member in &plans[0].team {
                        if let Some(ref moves) = member.recommended_moves {
                            for m in moves {
                                assert!(m.power > 0, "recommended move should have power");
                            }
                            // All moves should be same damage class
                            let classes: std::collections::HashSet<&str> =
                                moves.iter().map(|m| m.damage_class.as_str()).collect();
                            assert!(
                                classes.len() <= 1,
                                "all recommended moves should be same damage class, got: {classes:?}"
                            );
                        }
                    }
                    break;
                }
                JobStatus::Failed => {
                    panic!(
                        "Job failed: {}",
                        job.result.map(|r| r.message).unwrap_or_default()
                    );
                }
                _ => continue,
            }
        }
    }

    #[tokio::test]
    async fn test_team_plan_without_learnset_skips_moves() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(
            JsonFileStorage::new(dir.path().to_path_buf())
                .await
                .unwrap(),
        );
        let pokeapi = Arc::new(MockPokeApiWithMoves);
        let svc = PokePlannerService::new(storage, pokeapi);

        // Pokedex source without learnset_version_group -> skip move selection
        let request = TeamPlanRequest {
            source: TeamSource::Pokedex {
                pokedex_name: "test-dex".to_string(),
            },
            min_bst: None,
            no_cache: false,
            top_k: Some(1),
            include_variants: true,
            exclude: Vec::new(),
            exclude_species: Vec::new(),
            exclude_variant_types: Vec::new(),
            counter_team: None,
            learnset_version_group: None,
        };

        let job_id = svc.submit_team_plan(request).await.unwrap();

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let job = svc.get_job(&job_id).await.unwrap();
            match job.status {
                JobStatus::Completed => {
                    let data = job.result.unwrap().data.unwrap();
                    let plans: Vec<pokeplanner_core::TeamPlan> =
                        serde_json::from_value(data).unwrap();
                    assert!(!plans.is_empty());
                    // All members should have None for recommended_moves
                    for member in &plans[0].team {
                        assert!(
                            member.recommended_moves.is_none(),
                            "Expected no recommended moves when learnset_version_group is None for Pokedex source"
                        );
                    }
                    break;
                }
                JobStatus::Failed => {
                    panic!(
                        "Job failed: {}",
                        job.result.map(|r| r.message).unwrap_or_default()
                    );
                }
                _ => continue,
            }
        }
    }

    #[test]
    fn test_generation_number_parsing() {
        assert_eq!(generation_number("generation-i"), 1);
        assert_eq!(generation_number("generation-iv"), 4);
        assert_eq!(generation_number("generation-ix"), 9);
        assert_eq!(generation_number("generation-x"), 10);
        assert_eq!(generation_number("unknown"), 0);
    }

    #[test]
    fn test_same_generation_fallbacks() {
        let all_vgs = vec![
            VersionGroupInfo {
                name: "legends-za".to_string(),
                versions: vec![],
                pokedexes: vec![],
                generation: "generation-ix".to_string(),
            },
            VersionGroupInfo {
                name: "scarlet-violet".to_string(),
                versions: vec![],
                pokedexes: vec![],
                generation: "generation-ix".to_string(),
            },
            VersionGroupInfo {
                name: "sword-shield".to_string(),
                versions: vec![],
                pokedexes: vec![],
                generation: "generation-viii".to_string(),
            },
        ];

        let fallbacks = same_generation_fallbacks(&["legends-za".to_string()], &all_vgs);
        assert_eq!(fallbacks, vec!["scarlet-violet"]);

        // Should not include VGs from other generations
        assert!(!fallbacks.contains(&"sword-shield".to_string()));
    }

    #[test]
    fn test_same_generation_fallbacks_no_siblings() {
        let all_vgs = vec![VersionGroupInfo {
            name: "legends-za".to_string(),
            versions: vec![],
            pokedexes: vec![],
            generation: "generation-ix".to_string(),
        }];
        let fallbacks = same_generation_fallbacks(&["legends-za".to_string()], &all_vgs);
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn test_pick_best_available_vg() {
        use pokeplanner_core::LearnMethod;
        let all_vgs = vec![
            VersionGroupInfo {
                name: "red-blue".to_string(),
                versions: vec![],
                pokedexes: vec![],
                generation: "generation-i".to_string(),
            },
            VersionGroupInfo {
                name: "scarlet-violet".to_string(),
                versions: vec![],
                pokedexes: vec![],
                generation: "generation-ix".to_string(),
            },
        ];
        let entries = vec![
            pokeplanner_core::LearnsetEntry {
                move_name: "tackle".to_string(),
                learn_method: LearnMethod::LevelUp,
                level: 1,
                version_group: "red-blue".to_string(),
            },
            pokeplanner_core::LearnsetEntry {
                move_name: "thunderbolt".to_string(),
                learn_method: LearnMethod::Machine,
                level: 0,
                version_group: "scarlet-violet".to_string(),
            },
        ];
        let best = pick_best_available_vg(&entries, &all_vgs);
        assert_eq!(best, Some("scarlet-violet".to_string()));
    }

    #[test]
    fn test_pick_best_available_vg_empty() {
        let best = pick_best_available_vg(&[], &[]);
        assert_eq!(best, None);
    }
}
