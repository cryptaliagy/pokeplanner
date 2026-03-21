#[cfg(test)]
mod tests {
    use crate::job::{Job, JobStatus};
    use crate::model::HealthResponse;

    #[test]
    fn test_job_new_defaults_to_pending() {
        let job = Job::new();
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.result.is_none());
    }

    #[test]
    fn test_job_has_unique_ids() {
        let a = Job::new();
        let b = Job::new();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_health_response_ok() {
        let h = HealthResponse::ok();
        assert_eq!(h.status, "ok");
    }

    #[test]
    fn test_job_serialization_roundtrip() {
        let job = Job::new();
        let json = serde_json::to_string(&job).unwrap();
        let deserialized: Job = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, job.id);
        assert_eq!(deserialized.status, job.status);
    }
}
