#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use pokeplanner_service::PokePlannerService;
    use pokeplanner_storage::JsonFileStorage;
    use tower::ServiceExt;

    use crate::create_router;

    async fn make_app() -> axum::Router {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(JsonFileStorage::new(dir.keep()).await.unwrap());
        let service = Arc::new(PokePlannerService::new(storage));
        create_router(service)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_submit_job_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::post("/jobs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_list_jobs_endpoint() {
        let app = make_app().await;
        let resp = app
            .oneshot(Request::get("/jobs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_nonexistent_job() {
        let app = make_app().await;
        let resp = app
            .oneshot(
                Request::get("/jobs/00000000-0000-0000-0000-000000000000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
