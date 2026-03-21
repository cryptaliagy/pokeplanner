use std::sync::Arc;

use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use tonic::{transport::Server, Request, Response, Status};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("pokeplanner");
}

use proto::poke_planner_service_server::{PokePlannerServiceServer, PokePlannerService as GrpcService};
use proto::*;

pub struct GrpcHandler {
    service: Arc<PokePlannerService<JsonFileStorage>>,
}

#[tonic::async_trait]
impl GrpcService for GrpcHandler {
    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<HealthResponse>, Status> {
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

    async fn submit_job(&self, _req: Request<SubmitJobRequest>) -> Result<Response<SubmitJobResponse>, Status> {
        let job_id = self.service.submit_job().await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(SubmitJobResponse {
            job_id: job_id.to_string(),
        }))
    }

    async fn get_job(&self, req: Request<GetJobRequest>) -> Result<Response<GetJobResponse>, Status> {
        let job_id = Uuid::parse_str(&req.into_inner().job_id)
            .map_err(|_| Status::invalid_argument("Invalid job ID"))?;
        let job = self.service.get_job(&job_id).await
            .map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(GetJobResponse {
            id: job.id.to_string(),
            status: format!("{:?}", job.status),
            created_at: job.created_at.to_rfc3339(),
            updated_at: job.updated_at.to_rfc3339(),
            result_output: job.result.map(|r| r.output),
        }))
    }

    async fn list_jobs(&self, _req: Request<ListJobsRequest>) -> Result<Response<ListJobsResponse>, Status> {
        let jobs = self.service.list_jobs().await
            .map_err(|e| Status::internal(e.to_string()))?;
        let jobs = jobs.into_iter().map(|job| GetJobResponse {
            id: job.id.to_string(),
            status: format!("{:?}", job.status),
            created_at: job.created_at.to_rfc3339(),
            updated_at: job.updated_at.to_rfc3339(),
            result_output: job.result.map(|r| r.output),
        }).collect();
        Ok(Response::new(ListJobsResponse { jobs }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let storage = Arc::new(
        JsonFileStorage::new("data/jobs".into())
            .await
            .expect("Failed to initialize storage"),
    );
    let service = Arc::new(PokePlannerService::new(storage));

    let handler = GrpcHandler { service };
    let addr = "0.0.0.0:50051".parse()?;
    tracing::info!("gRPC server listening on {addr}");

    Server::builder()
        .add_service(PokePlannerServiceServer::new(handler))
        .serve(addr)
        .await?;

    Ok(())
}
