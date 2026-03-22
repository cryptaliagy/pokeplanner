use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pokeplanner_core::{
    AppError, PokemonQueryParams, SortField, SortOrder, TeamPlanRequest, TeamSource,
};
use pokeplanner_pokeapi::PokeApiHttpClient;
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tonic::{transport::Server, Request, Response, Status};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("pokeplanner");
}

use proto::poke_planner_service_server::{
    PokePlannerService as GrpcService, PokePlannerServiceServer,
};
use proto::*;

pub struct GrpcHandler {
    service: Arc<PokePlannerService<JsonFileStorage, PokeApiHttpClient>>,
}

impl GrpcHandler {
    fn pokemon_to_proto(p: &pokeplanner_core::Pokemon) -> Pokemon {
        Pokemon {
            species_name: p.species_name.clone(),
            form_name: p.form_name.clone(),
            pokedex_number: p.pokedex_number,
            types: p.types.iter().map(|t| t.to_string()).collect(),
            stats: Some(BaseStats {
                hp: p.stats.hp,
                attack: p.stats.attack,
                defense: p.stats.defense,
                special_attack: p.stats.special_attack,
                special_defense: p.stats.special_defense,
                speed: p.stats.speed,
            }),
            is_default_form: p.is_default_form,
            bst: p.bst(),
        }
    }

    fn coverage_to_proto(c: &pokeplanner_core::TypeCoverage) -> TypeCoverage {
        TypeCoverage {
            offensive_coverage: c.offensive_coverage.iter().map(|t| t.to_string()).collect(),
            defensive_weaknesses: c
                .defensive_weaknesses
                .iter()
                .map(|t| t.to_string())
                .collect(),
            uncovered_types: c.uncovered_types.iter().map(|t| t.to_string()).collect(),
            coverage_score: c.coverage_score,
        }
    }

    fn job_to_proto(job: &pokeplanner_core::Job) -> GetJobResponse {
        GetJobResponse {
            id: job.id.to_string(),
            status: format!("{:?}", job.status),
            kind: format!("{:?}", job.kind),
            created_at: job.created_at.to_rfc3339(),
            updated_at: job.updated_at.to_rfc3339(),
            result_message: job.result.as_ref().map(|r| r.message.clone()),
            result_data: job
                .result
                .as_ref()
                .and_then(|r| r.data.as_ref().map(|d| d.to_string())),
            progress: job.progress.as_ref().map(|p| JobProgress {
                phase: p.phase.clone(),
                completed_steps: p.completed_steps,
                total_steps: p.total_steps,
            }),
        }
    }

    fn proto_sort_field(f: i32) -> Option<SortField> {
        match proto::SortField::try_from(f) {
            Ok(proto::SortField::Bst) => Some(SortField::Bst),
            Ok(proto::SortField::Hp) => Some(SortField::Hp),
            Ok(proto::SortField::Attack) => Some(SortField::Attack),
            Ok(proto::SortField::Defense) => Some(SortField::Defense),
            Ok(proto::SortField::SpecialAttack) => Some(SortField::SpecialAttack),
            Ok(proto::SortField::SpecialDefense) => Some(SortField::SpecialDefense),
            Ok(proto::SortField::Speed) => Some(SortField::Speed),
            Ok(proto::SortField::Name) => Some(SortField::Name),
            Ok(proto::SortField::PokedexNumber) => Some(SortField::PokedexNumber),
            Err(_) => None,
        }
    }

    fn proto_sort_order(o: i32) -> SortOrder {
        match proto::SortOrder::try_from(o) {
            Ok(proto::SortOrder::Desc) => SortOrder::Desc,
            _ => SortOrder::Asc,
        }
    }

    fn app_error_to_status(e: AppError) -> Status {
        match &e {
            AppError::NotFound(_) | AppError::JobNotFound(_) => Status::not_found(e.to_string()),
            _ => Status::internal(e.to_string()),
        }
    }
}

#[tonic::async_trait]
impl GrpcService for GrpcHandler {
    async fn health(
        &self,
        _req: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let h = self.service.health();
        Ok(Response::new(HealthResponse {
            status: h.status,
            version: h.version,
        }))
    }

    async fn ping(&self, req: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let msg = req.into_inner().message;
        Ok(Response::new(PingResponse {
            message: format!("pong: {msg}"),
        }))
    }

    async fn submit_job(
        &self,
        _req: Request<SubmitJobRequest>,
    ) -> Result<Response<SubmitJobResponse>, Status> {
        let job_id = self
            .service
            .submit_job()
            .await
            .map_err(Self::app_error_to_status)?;
        Ok(Response::new(SubmitJobResponse {
            job_id: job_id.to_string(),
        }))
    }

    async fn get_job(
        &self,
        req: Request<GetJobRequest>,
    ) -> Result<Response<GetJobResponse>, Status> {
        let job_id = Uuid::parse_str(&req.into_inner().job_id)
            .map_err(|_| Status::invalid_argument("Invalid job ID"))?;
        let job = self
            .service
            .get_job(&job_id)
            .await
            .map_err(Self::app_error_to_status)?;
        Ok(Response::new(Self::job_to_proto(&job)))
    }

    async fn list_jobs(
        &self,
        _req: Request<ListJobsRequest>,
    ) -> Result<Response<ListJobsResponse>, Status> {
        let jobs = self
            .service
            .list_jobs()
            .await
            .map_err(Self::app_error_to_status)?;
        let jobs = jobs.iter().map(Self::job_to_proto).collect();
        Ok(Response::new(ListJobsResponse { jobs }))
    }

    async fn get_version_groups(
        &self,
        req: Request<GetVersionGroupsRequest>,
    ) -> Result<Response<GetVersionGroupsResponse>, Status> {
        let inner = req.into_inner();
        let groups = self
            .service
            .list_version_groups(inner.no_cache)
            .await
            .map_err(Self::app_error_to_status)?;
        let version_groups = groups
            .into_iter()
            .map(|g| VersionGroupInfo {
                name: g.name,
                versions: g.versions,
                pokedexes: g.pokedexes,
            })
            .collect();
        Ok(Response::new(GetVersionGroupsResponse { version_groups }))
    }

    async fn get_game_pokemon(
        &self,
        req: Request<GetGamePokemonRequest>,
    ) -> Result<Response<GetGamePokemonResponse>, Status> {
        let inner = req.into_inner();
        let pokemon = self
            .service
            .get_game_pokemon(
                &inner.version_group,
                &PokemonQueryParams {
                    min_bst: inner.min_bst,
                    no_cache: inner.no_cache,
                    sort_by: Self::proto_sort_field(inner.sort_by),
                    sort_order: Self::proto_sort_order(inner.sort_order),
                    include_variants: inner.include_variants,
                    limit: inner.limit.map(|l| l as usize),
                },
            )
            .await
            .map_err(Self::app_error_to_status)?;
        let count = pokemon.len() as u32;
        let pokemon = pokemon.iter().map(Self::pokemon_to_proto).collect();
        Ok(Response::new(GetGamePokemonResponse { pokemon, count }))
    }

    async fn get_pokedex_pokemon(
        &self,
        req: Request<GetPokedexPokemonRequest>,
    ) -> Result<Response<GetPokedexPokemonResponse>, Status> {
        let inner = req.into_inner();
        let pokemon = self
            .service
            .get_pokedex_pokemon(
                &inner.pokedex_name,
                &PokemonQueryParams {
                    min_bst: inner.min_bst,
                    no_cache: inner.no_cache,
                    sort_by: Self::proto_sort_field(inner.sort_by),
                    sort_order: Self::proto_sort_order(inner.sort_order),
                    include_variants: inner.include_variants,
                    limit: inner.limit.map(|l| l as usize),
                },
            )
            .await
            .map_err(Self::app_error_to_status)?;
        let count = pokemon.len() as u32;
        let pokemon = pokemon.iter().map(Self::pokemon_to_proto).collect();
        Ok(Response::new(GetPokedexPokemonResponse { pokemon, count }))
    }

    async fn get_pokemon(
        &self,
        req: Request<GetPokemonRequest>,
    ) -> Result<Response<GetPokemonResponse>, Status> {
        let inner = req.into_inner();
        let pokemon = self
            .service
            .get_pokemon(&inner.name, inner.no_cache)
            .await
            .map_err(Self::app_error_to_status)?;
        Ok(Response::new(GetPokemonResponse {
            pokemon: Some(Self::pokemon_to_proto(&pokemon)),
        }))
    }

    async fn plan_team(
        &self,
        req: Request<PlanTeamRequest>,
    ) -> Result<Response<PlanTeamResponse>, Status> {
        let inner = req.into_inner();
        let source = match inner.source {
            Some(team_source) => match team_source.source {
                Some(team_source::Source::Games(list)) => TeamSource::Game {
                    version_groups: list.version_groups,
                },
                Some(team_source::Source::Pokedex(name)) => {
                    TeamSource::Pokedex { pokedex_name: name }
                }
                Some(team_source::Source::Custom(list)) => TeamSource::Custom {
                    pokemon_names: list.pokemon_names,
                },
                None => return Err(Status::invalid_argument("TeamSource variant is required")),
            },
            None => return Err(Status::invalid_argument("source is required")),
        };
        let request = TeamPlanRequest {
            source,
            min_bst: inner.min_bst,
            no_cache: inner.no_cache,
            top_k: inner.top_k.map(|k| k as usize),
            include_variants: inner.include_variants,
            exclude: inner.exclude,
            exclude_species: inner.exclude_species,
            exclude_variant_types: inner.exclude_variant_types,
            counter_team: if inner.counter_team.is_empty() {
                None
            } else {
                Some(inner.counter_team)
            },
        };
        let job_id = self
            .service
            .submit_team_plan(request)
            .await
            .map_err(Self::app_error_to_status)?;
        Ok(Response::new(PlanTeamResponse {
            job_id: job_id.to_string(),
        }))
    }

    async fn analyze_team(
        &self,
        req: Request<AnalyzeTeamRequest>,
    ) -> Result<Response<AnalyzeTeamResponse>, Status> {
        let inner = req.into_inner();
        if inner.pokemon_names.is_empty() {
            return Err(Status::invalid_argument("pokemon_names must not be empty"));
        }
        let coverage = self
            .service
            .analyze_team(inner.pokemon_names, inner.no_cache)
            .await
            .map_err(Self::app_error_to_status)?;
        Ok(Response::new(AnalyzeTeamResponse {
            coverage: Some(Self::coverage_to_proto(&coverage)),
        }))
    }
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pokeplanner"))
        .unwrap_or_else(|| PathBuf::from(".pokeplanner"))
}

#[derive(Parser)]
#[command(name = "pokeplanner-grpc", about = "PokePlanner gRPC API server", version)]
struct Cli {
    /// Host address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 50051)]
    port: u16,

    /// Directory for cached PokeAPI data
    #[arg(long, default_value_os_t = default_data_dir().join("cache"))]
    cache_dir: PathBuf,

    /// Directory for job storage data
    #[arg(long, default_value_os_t = default_data_dir().join("jobs"))]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let storage = Arc::new(
        JsonFileStorage::new(cli.data_dir)
            .await
            .expect("Failed to initialize storage"),
    );
    let pokeapi = Arc::new(
        PokeApiHttpClient::new(cli.cache_dir)
            .await
            .expect("Failed to initialize PokeAPI client"),
    );
    let service = Arc::new(PokePlannerService::new(storage, pokeapi));

    let handler = GrpcHandler { service };
    let addr = format!("{}:{}", cli.host, cli.port).parse()?;
    tracing::info!("gRPC server listening on {addr}");

    Server::builder()
        .add_service(PokePlannerServiceServer::new(handler))
        .serve(addr)
        .await?;

    Ok(())
}
