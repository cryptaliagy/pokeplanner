#[cfg(test)]
mod tests {
    use pokeplanner_core::Job;
    use crate::json_store::JsonFileStorage;
    use crate::traits::Storage;

    #[tokio::test]
    async fn test_save_and_get_job() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap();
        let job = Job::new();
        let job_id = job.id;

        store.save_job(&job).await.unwrap();
        let retrieved = store.get_job(&job_id).await.unwrap();
        assert_eq!(retrieved.id, job_id);
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap();

        store.save_job(&Job::new()).await.unwrap();
        store.save_job(&Job::new()).await.unwrap();

        let jobs = store.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 2);
    }

    #[tokio::test]
    async fn test_get_nonexistent_job_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap();
        let result = store.get_job(&uuid::Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_job() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap();
        let mut job = Job::new();
        let job_id = job.id;
        store.save_job(&job).await.unwrap();

        job.status = pokeplanner_core::JobStatus::Running;
        store.update_job(&job).await.unwrap();

        let updated = store.get_job(&job_id).await.unwrap();
        assert_eq!(updated.status, pokeplanner_core::JobStatus::Running);
    }
}
