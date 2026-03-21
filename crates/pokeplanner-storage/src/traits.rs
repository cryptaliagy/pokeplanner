use async_trait::async_trait;
use pokeplanner_core::{AppError, Job, JobId};

/// Storage trait providing a flexible interface for persistence.
/// Currently backed by JSON files, but designed for future SQL/NoSQL integration.
#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_job(&self, job: &Job) -> Result<(), AppError>;
    async fn get_job(&self, id: &JobId) -> Result<Job, AppError>;
    async fn list_jobs(&self) -> Result<Vec<Job>, AppError>;
    async fn update_job(&self, job: &Job) -> Result<(), AppError>;
}
