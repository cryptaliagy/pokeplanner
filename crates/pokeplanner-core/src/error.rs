use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
