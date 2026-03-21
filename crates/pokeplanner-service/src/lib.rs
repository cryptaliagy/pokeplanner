#[cfg(test)]
mod tests;

use std::sync::Arc;

use chrono::Utc;
use pokeplanner_core::{AppError, HealthResponse, Job, JobId, JobResult, JobStatus};
use pokeplanner_storage::Storage;
use tracing::info;

pub struct PokePlannerService {
    storage: Arc<dyn Storage>,
}

impl PokePlannerService {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    pub fn health(&self) -> HealthResponse {
        HealthResponse::ok()
    }

    /// No-op service call — placeholder for future business logic.
    pub async fn noop(&self) -> Result<String, AppError> {
        info!("noop called");
        Ok("noop".to_string())
    }

    /// Submit a long-running job. Returns the job ID immediately.
    pub async fn submit_job(&self) -> Result<JobId, AppError> {
        let job = Job::new();
        let job_id = job.id;
        self.storage.save_job(&job).await?;

        let storage = Arc::clone(&self.storage);
        tokio::spawn(async move {
            Self::run_job(storage, job_id).await;
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

    async fn run_job(storage: Arc<dyn Storage>, job_id: JobId) {
        // Mark as running
        if let Ok(mut job) = storage.get_job(&job_id).await {
            job.status = JobStatus::Running;
            job.updated_at = Utc::now();
            let _ = storage.update_job(&job).await;

            // Simulate work (no-op for now)
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Mark as completed
            job.status = JobStatus::Completed;
            job.updated_at = Utc::now();
            job.result = Some(JobResult {
                output: "Job completed successfully".to_string(),
            });
            let _ = storage.update_job(&job).await;
            info!(job_id = %job_id, "job completed");
        }
    }
}
