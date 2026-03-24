use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use pokeplanner_core::AppError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::{debug, warn};

pub const CACHE_TTL_DAYS: i64 = 365;

/// Known cache categories and their descriptions.
pub const CACHE_CATEGORIES: &[(&str, &str)] = &[
    ("pokemon", "Individual pokemon form data"),
    ("species", "Pokemon species and variety data"),
    ("pokedex", "Pokedex entry lists"),
    ("version-group", "Version group metadata"),
    ("type", "Type effectiveness data"),
    ("meta", "API metadata (version group lists)"),
    ("game-pokemon", "Aggregated game pokemon lists"),
    ("pokedex-pokemon", "Aggregated pokedex pokemon lists"),
    ("type-chart", "Pre-computed type chart"),
    ("pokemon-full", "Full pokemon data including moves"),
    ("move", "Individual move data"),
];

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
            .map_err(|e| AppError::Io {
                context: "creating cache directory".into(),
                source: e,
            })?;
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
                .map_err(|e| AppError::Io {
                    context: "creating cache dir".into(),
                    source: e,
                })?;
        }

        let entry = CacheEntry {
            cached_at: Utc::now(),
            data,
        };

        let json = serde_json::to_vec(&entry).map_err(|e| AppError::Serialization {
            context: "serializing cache entry".into(),
            source: e,
        })?;

        tokio::fs::write(&path, json)
            .await
            .map_err(|e| AppError::Io {
                context: "writing cache file".into(),
                source: e,
            })?;

        debug!("Cache set for {category}/{key}");
        Ok(())
    }

    /// Returns the base path of the cache directory.
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Remove a specific cache entry.
    pub async fn remove(&self, category: &str, key: &str) -> Result<bool, AppError> {
        let path = self.cache_path(category, key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(AppError::Io {
                context: format!("removing cache entry {category}/{key}"),
                source: e,
            }),
        }
    }

    /// Remove all entries in a cache category. Returns count of files removed.
    pub async fn clear_category(&self, category: &str) -> Result<u64, AppError> {
        let dir = self.base_path.join(category);
        if !dir.exists() {
            return Ok(0);
        }
        let mut count = 0u64;
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| AppError::Io {
            context: format!("reading cache dir {category}/"),
            source: e,
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|e| AppError::Io {
            context: "reading cache dir entry".into(),
            source: e,
        })? {
            if entry.path().is_file() {
                let _ = tokio::fs::remove_file(entry.path()).await;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Remove all cached data.
    pub async fn clear_all(&self) -> Result<u64, AppError> {
        let mut total = 0u64;
        for &(cat, _) in CACHE_CATEGORIES {
            total += self.clear_category(cat).await?;
        }
        Ok(total)
    }

    /// Remove expired cache entries. Returns count of files removed.
    pub async fn clear_stale(&self) -> Result<u64, AppError> {
        let ttl = Duration::days(CACHE_TTL_DAYS);
        let now = Utc::now();
        let mut count = 0u64;

        for &(cat, _) in CACHE_CATEGORIES {
            let dir = self.base_path.join(cat);
            if !dir.exists() {
                continue;
            }
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(e) => e,
                Err(_) => continue,
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    // Parse just the cached_at timestamp
                    #[derive(Deserialize)]
                    struct Timestamp {
                        cached_at: DateTime<Utc>,
                    }
                    if let Ok(ts) = serde_json::from_slice::<Timestamp>(&bytes) {
                        if now - ts.cached_at > ttl {
                            let _ = tokio::fs::remove_file(&path).await;
                            count += 1;
                        }
                    } else {
                        // Corrupt entry — remove it
                        let _ = tokio::fs::remove_file(&path).await;
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Gather statistics about the cache.
    pub async fn stats(&self) -> CacheStats {
        let mut categories = Vec::new();
        let mut total_entries = 0u64;
        let mut total_size = 0u64;

        for &(cat, description) in CACHE_CATEGORIES {
            let dir = self.base_path.join(cat);
            let mut cat_entries = 0u64;
            let mut cat_size = 0u64;

            if dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry.path().is_file() {
                            cat_entries += 1;
                            if let Ok(meta) = entry.metadata().await {
                                cat_size += meta.len();
                            }
                        }
                    }
                }
            }

            if cat_entries > 0 {
                categories.push(CategoryStats {
                    name: cat.to_string(),
                    description: description.to_string(),
                    entries: cat_entries,
                    size_bytes: cat_size,
                });
            }
            total_entries += cat_entries;
            total_size += cat_size;
        }

        CacheStats {
            base_path: self.base_path.clone(),
            total_entries,
            total_size_bytes: total_size,
            categories,
        }
    }
}

pub struct CacheStats {
    pub base_path: PathBuf,
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub categories: Vec<CategoryStats>,
}

pub struct CategoryStats {
    pub name: String,
    pub description: String,
    pub entries: u64,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_set_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache
            .set("pokemon", "pikachu", &"yellow mouse")
            .await
            .unwrap();
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
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
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

    #[tokio::test]
    async fn test_remove_existing_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache.set("pokemon", "pikachu", &"data").await.unwrap();
        assert!(cache.remove("pokemon", "pikachu").await.unwrap());
        let result: Option<String> = cache.get("pokemon", "pikachu", false).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        assert!(!cache.remove("pokemon", "nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_clear_category() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache.set("pokemon", "pikachu", &"a").await.unwrap();
        cache.set("pokemon", "charizard", &"b").await.unwrap();
        cache.set("species", "pikachu", &"c").await.unwrap();

        let removed = cache.clear_category("pokemon").await.unwrap();
        assert_eq!(removed, 2);

        // Species should still exist
        let result: Option<String> = cache.get("species", "pikachu", false).await;
        assert_eq!(result, Some("c".to_string()));
    }

    #[tokio::test]
    async fn test_clear_all() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache.set("pokemon", "pikachu", &"a").await.unwrap();
        cache.set("species", "pikachu", &"b").await.unwrap();
        cache.set("type", "fire", &"c").await.unwrap();

        let removed = cache.clear_all().await.unwrap();
        assert_eq!(removed, 3);

        let result: Option<String> = cache.get("pokemon", "pikachu", false).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_stats() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        cache.set("pokemon", "pikachu", &"data1").await.unwrap();
        cache.set("pokemon", "charizard", &"data2").await.unwrap();
        cache.set("type", "fire", &"data3").await.unwrap();

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 3);
        assert!(stats.total_size_bytes > 0);
        assert_eq!(stats.categories.len(), 2);

        let pokemon_cat = stats
            .categories
            .iter()
            .find(|c| c.name == "pokemon")
            .unwrap();
        assert_eq!(pokemon_cat.entries, 2);
    }

    #[tokio::test]
    async fn test_clear_stale_removes_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf()).await.unwrap();

        // Write corrupt data to a known category
        let path = cache.cache_path("pokemon", "corrupt");
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&path, b"not valid json").await.unwrap();

        // Write valid data
        cache.set("pokemon", "valid", &"data").await.unwrap();

        let removed = cache.clear_stale().await.unwrap();
        assert_eq!(removed, 1); // only the corrupt entry

        // Valid entry still exists
        let result: Option<String> = cache.get("pokemon", "valid", false).await;
        assert!(result.is_some());
    }
}
