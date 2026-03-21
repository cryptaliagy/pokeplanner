use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type JobId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub result: Option<JobResult>,
}

impl Job {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            status: JobStatus::Pending,
            created_at: now,
            updated_at: now,
            result: None,
        }
    }
}

impl Default for Job {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults_to_pending() {
        let job = Job::new();
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.result.is_none());
    }

    #[test]
    fn test_has_unique_ids() {
        let a = Job::new();
        let b = Job::new();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let job = Job::new();
        let json = serde_json::to_string(&job).unwrap();
        let deserialized: Job = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, job.id);
        assert_eq!(deserialized.status, job.status);
    }
}
