use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use pokeplanner_core::AppError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::{debug, warn};

const CACHE_TTL_DAYS: i64 = 365;

#[derive(Serialize, Deserialize)]
struct CacheEntry<T> {
    cached_at: DateTime<Utc>,
    data: T,
}

pub struct DiskCache {
    base_path: PathBuf,
}

impl DiskCache {
    pub async fn new(base_path: PathBuf) -> Result<Self, AppError> {
        tokio::fs::create_dir_all(&base_path)
            .await
            .map_err(|e| AppError::Cache(format!("Failed to create cache directory: {e}")))?;
        Ok(Self { base_path })
    }

    fn cache_path(&self, category: &str, key: &str) -> PathBuf {
        self.base_path.join(category).join(format!("{key}.json"))
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        category: &str,
        key: &str,
        no_cache: bool,
    ) -> Option<T> {
        if no_cache {
            return None;
        }

        let path = self.cache_path(category, key);
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(_) => return None,
        };

        let entry: CacheEntry<T> = match serde_json::from_slice(&bytes) {
            Ok(e) => e,
            Err(e) => {
                warn!("Cache corruption for {category}/{key}, deleting: {e}");
                let _ = tokio::fs::remove_file(&path).await;
                return None;
            }
        };

        let ttl = Duration::days(CACHE_TTL_DAYS);
        if Utc::now() - entry.cached_at > ttl {
            debug!("Cache expired for {category}/{key}");
            let _ = tokio::fs::remove_file(&path).await;
            return None;
        }

        debug!("Cache hit for {category}/{key}");
        Some(entry.data)
    }

    pub async fn set<T: Serialize>(
        &self,
        category: &str,
        key: &str,
        data: &T,
    ) -> Result<(), AppError> {
        let path = self.cache_path(category, key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Cache(format!("Failed to create cache dir: {e}")))?;
        }

        let entry = CacheEntry {
            cached_at: Utc::now(),
            data,
        };

        let json = serde_json::to_vec(&entry)
            .map_err(|e| AppError::Cache(format!("Failed to serialize cache entry: {e}")))?;

        tokio::fs::write(&path, json)
            .await
            .map_err(|e| AppError::Cache(format!("Failed to write cache file: {e}")))?;

        debug!("Cache set for {category}/{key}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_set_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache.set("pokemon", "pikachu", &"yellow mouse").await.unwrap();
        let result: Option<String> = cache.get("pokemon", "pikachu", false).await;
        assert_eq!(result, Some("yellow mouse".to_string()));
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        let result: Option<String> = cache.get("pokemon", "nonexistent", false).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_no_cache_flag() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache.set("pokemon", "pikachu", &"data").await.unwrap();
        let result: Option<String> = cache.get("pokemon", "pikachu", true).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_handles_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        // Write corrupt data
        let path = cache.cache_path("pokemon", "bad");
        tokio::fs::create_dir_all(path.parent().unwrap()).await.unwrap();
        tokio::fs::write(&path, b"not json").await.unwrap();

        let result: Option<String> = cache.get("pokemon", "bad", false).await;
        assert!(result.is_none());

        // Corrupt file should be deleted
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_cache_structured_data() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestPokemon {
            name: String,
            level: u32,
        }

        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        let pokemon = TestPokemon {
            name: "charizard".to_string(),
            level: 50,
        };
        cache.set("pokemon", "charizard", &pokemon).await.unwrap();

        let result: Option<TestPokemon> = cache.get("pokemon", "charizard", false).await;
        assert_eq!(result, Some(pokemon));
    }
}
