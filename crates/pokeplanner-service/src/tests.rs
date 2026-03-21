#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use pokeplanner_storage::JsonFileStorage;
    use crate::PokePlannerService;

    async fn make_service() -> PokePlannerService {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.keep()).await.unwrap());
        PokePlannerService::new(storage)
    }

    #[tokio::test]
    async fn test_health() {
        let svc = make_service().await;
        let h = svc.health();
        assert_eq!(h.status, "ok");
    }

    #[tokio::test]
    async fn test_noop() {
        let svc = make_service().await;
        let result = svc.noop().await.unwrap();
        assert_eq!(result, "noop");
    }

    #[tokio::test]
    async fn test_submit_and_get_job() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap());
        let svc = PokePlannerService::new(storage);

        let job_id = svc.submit_job().await.unwrap();
        // Give the background task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        let job = svc.get_job(&job_id).await.unwrap();
        assert_eq!(job.id, job_id);
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.path().to_path_buf()).await.unwrap());
        let svc = PokePlannerService::new(storage);

        svc.submit_job().await.unwrap();
        svc.submit_job().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let jobs = svc.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 2);
    }
}
