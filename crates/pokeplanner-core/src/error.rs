use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("IO error: {context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Serialization error: {context}")]
    Serialization {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("PokeAPI error: {0}")]
    PokeApi(String),
}
