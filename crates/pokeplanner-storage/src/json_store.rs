use std::path::PathBuf;

use async_trait::async_trait;
use pokeplanner_core::{AppError, Job, JobId};
use tokio::sync::RwLock;

use crate::traits::Storage;

pub struct JsonFileStorage {
    base_path: PathBuf,
    lock: RwLock<()>,
}

impl JsonFileStorage {
    pub async fn new(base_path: PathBuf) -> Result<Self, AppError> {
        tokio::fs::create_dir_all(&base_path)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to create storage dir: {e}")))?;
        Ok(Self {
            base_path,
            lock: RwLock::new(()),
        })
    }

    fn job_path(&self, id: &JobId) -> PathBuf {
        self.base_path.join(format!("{id}.json"))
    }
}

#[async_trait]
impl Storage for JsonFileStorage {
    async fn save_job(&self, job: &Job) -> Result<(), AppError> {
        let _guard = self.lock.write().await;
        let path = self.job_path(&job.id);
        let data = serde_json::to_string_pretty(job)
            .map_err(|e| AppError::Storage(format!("Serialization error: {e}")))?;
        tokio::fs::write(&path, data)
            .await
            .map_err(|e| AppError::Storage(format!("Write error: {e}")))?;
        Ok(())
    }

    async fn get_job(&self, id: &JobId) -> Result<Job, AppError> {
        let _guard = self.lock.read().await;
        let path = self.job_path(id);
        let data = tokio::fs::read_to_string(&path)
            .await
            .map_err(|_| AppError::JobNotFound(*id))?;
        let job: Job = serde_json::from_str(&data)
            .map_err(|e| AppError::Storage(format!("Deserialization error: {e}")))?;
        Ok(job)
    }

    async fn list_jobs(&self) -> Result<Vec<Job>, AppError> {
        let _guard = self.lock.read().await;
        let mut jobs = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.base_path)
            .await
            .map_err(|e| AppError::Storage(format!("Read dir error: {e}")))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::Storage(format!("Dir entry error: {e}")))?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let data = tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| AppError::Storage(format!("Read error: {e}")))?;
                if let Ok(job) = serde_json::from_str::<Job>(&data) {
                    jobs.push(job);
                }
            }
        }
        Ok(jobs)
    }

    async fn update_job(&self, job: &Job) -> Result<(), AppError> {
        let _guard = self.lock.write().await;
        let path = self.job_path(&job.id);
        if !path.exists() {
            return Err(AppError::JobNotFound(job.id));
        }
        let data = serde_json::to_string_pretty(job)
            .map_err(|e| AppError::Storage(format!("Serialization error: {e}")))?;
        tokio::fs::write(&path, data)
            .await
            .map_err(|e| AppError::Storage(format!("Write error: {e}")))?;
        Ok(())
    }
}
