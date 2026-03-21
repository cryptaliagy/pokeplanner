use std::future::Future;

use pokeplanner_core::{AppError, Job, JobId};

/// Storage trait providing a flexible interface for persistence.
/// Currently backed by JSON files, but designed for future SQL/NoSQL integration.
///
/// Uses native async via `impl Future` (no `async-trait` dependency). The `+ Send`
/// bound on returned futures ensures compatibility with multithreaded runtimes like tokio.
pub trait Storage: Send + Sync + 'static {
    fn save_job(&self, job: &Job) -> impl Future<Output = Result<(), AppError>> + Send;
    fn get_job(&self, id: &JobId) -> impl Future<Output = Result<Job, AppError>> + Send;
    fn list_jobs(&self) -> impl Future<Output = Result<Vec<Job>, AppError>> + Send;
    fn update_job(&self, job: &Job) -> impl Future<Output = Result<(), AppError>> + Send;
}
