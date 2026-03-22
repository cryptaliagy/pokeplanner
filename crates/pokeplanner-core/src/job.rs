use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::team::TeamPlanRequest;

pub type JobId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    #[default]
    Generic,
    TeamPlan(TeamPlanRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub phase: String,
    pub completed_steps: u32,
    pub total_steps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub status: JobStatus,
    #[serde(default)]
    pub kind: JobKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub result: Option<JobResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<JobProgress>,
}

impl Job {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            status: JobStatus::Pending,
            kind: JobKind::Generic,
            created_at: now,
            updated_at: now,
            result: None,
            progress: None,
        }
    }

    pub fn with_kind(kind: JobKind) -> Self {
        let mut job = Self::new();
        job.kind = kind;
        job
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
        assert_eq!(job.kind, JobKind::Generic);
        assert!(job.result.is_none());
        assert!(job.progress.is_none());
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

    #[test]
    fn test_backward_compat_no_kind_field() {
        // Simulate old serialized job without kind/progress fields
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "status": "pending",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "result": null
        }"#;
        let job: Job = serde_json::from_str(json).unwrap();
        assert_eq!(job.kind, JobKind::Generic);
        assert!(job.progress.is_none());
    }

    #[test]
    fn test_job_result_with_data() {
        let result = JobResult {
            message: "done".to_string(),
            data: Some(serde_json::json!({"teams": []})),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: JobResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.message, "done");
        assert!(deserialized.data.is_some());
    }

    #[test]
    fn test_job_result_backward_compat() {
        // Old format with only "output" field should fail gracefully,
        // but new format with "message" works
        let json = r#"{"message": "completed"}"#;
        let result: JobResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.message, "completed");
        assert!(result.data.is_none());
    }

    #[test]
    fn test_job_progress() {
        let progress = JobProgress {
            phase: "Fetching pokemon".to_string(),
            completed_steps: 47,
            total_steps: 312,
        };
        let json = serde_json::to_string(&progress).unwrap();
        let deserialized: JobProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.phase, "Fetching pokemon");
        assert_eq!(deserialized.completed_steps, 47);
        assert_eq!(deserialized.total_steps, 312);
    }
}
