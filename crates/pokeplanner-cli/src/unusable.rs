use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Persistent store of pokemon form names marked as unusable.
///
/// Stored as a sorted JSON array in `~/.pokeplanner/unusable.json`.
/// Unusable pokemon are automatically excluded from team planning
/// and flagged when viewed.
pub struct UnusableStore {
    path: PathBuf,
    entries: BTreeSet<String>,
}

impl UnusableStore {
    /// Load the unusable store from disk, creating an empty one if missing.
    pub async fn load(data_dir: &Path) -> anyhow::Result<Self> {
        let path = data_dir.join("unusable.json");
        let entries = if path.exists() {
            let bytes = tokio::fs::read(&path).await?;
            let names: Vec<String> = serde_json::from_slice(&bytes)?;
            names.into_iter().collect()
        } else {
            BTreeSet::new()
        };
        Ok(Self { path, entries })
    }

    /// Save the current state to disk.
    async fn save(&self) -> anyhow::Result<()> {
        let sorted: Vec<&String> = self.entries.iter().collect();
        let json = serde_json::to_string_pretty(&sorted)?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&self.path, json).await?;
        Ok(())
    }

    /// Add one or more form names. Returns the names that were newly added.
    pub async fn add(&mut self, names: &[String]) -> anyhow::Result<Vec<String>> {
        let mut added = Vec::new();
        for name in names {
            let lower = name.to_lowercase();
            if self.entries.insert(lower.clone()) {
                added.push(lower);
            }
        }
        if !added.is_empty() {
            self.save().await?;
        }
        Ok(added)
    }

    /// Remove one or more form names. Returns the names that were removed.
    pub async fn remove(&mut self, names: &[String]) -> anyhow::Result<Vec<String>> {
        let mut removed = Vec::new();
        for name in names {
            let lower = name.to_lowercase();
            if self.entries.remove(&lower) {
                removed.push(lower);
            }
        }
        if !removed.is_empty() {
            self.save().await?;
        }
        Ok(removed)
    }

    /// Clear all entries. Returns the count removed.
    pub async fn clear(&mut self) -> anyhow::Result<usize> {
        let count = self.entries.len();
        self.entries.clear();
        self.save().await?;
        Ok(count)
    }

    /// Check if a form name is marked unusable.
    pub fn is_unusable(&self, form_name: &str) -> bool {
        self.entries.contains(&form_name.to_lowercase())
    }

    /// Returns all unusable form names (sorted).
    pub fn list(&self) -> Vec<&str> {
        self.entries.iter().map(|s| s.as_str()).collect()
    }

    /// Returns the entries as a Vec<String> for merging into exclude lists.
    pub fn to_exclude_list(&self) -> Vec<String> {
        self.entries.iter().cloned().collect()
    }

    /// Returns the number of entries.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UnusableStore::load(dir.path()).await.unwrap();

        let added = store
            .add(&["charizard-mega-x".to_string(), "mewtwo-mega-y".to_string()])
            .await
            .unwrap();
        assert_eq!(added.len(), 2);
        assert_eq!(store.len(), 2);

        let list = store.list();
        assert_eq!(list, vec!["charizard-mega-x", "mewtwo-mega-y"]);
    }

    #[tokio::test]
    async fn test_add_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UnusableStore::load(dir.path()).await.unwrap();

        store.add(&["pikachu".to_string()]).await.unwrap();
        let added = store.add(&["pikachu".to_string()]).await.unwrap();
        assert!(added.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[tokio::test]
    async fn test_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UnusableStore::load(dir.path()).await.unwrap();

        store.add(&["Charizard-Mega-X".to_string()]).await.unwrap();
        assert!(store.is_unusable("charizard-mega-x"));
        assert!(store.is_unusable("CHARIZARD-MEGA-X"));
    }

    #[tokio::test]
    async fn test_remove() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UnusableStore::load(dir.path()).await.unwrap();

        store
            .add(&["pikachu".to_string(), "charizard".to_string()])
            .await
            .unwrap();
        let removed = store.remove(&["pikachu".to_string()]).await.unwrap();
        assert_eq!(removed, vec!["pikachu"]);
        assert_eq!(store.len(), 1);
        assert!(!store.is_unusable("pikachu"));
        assert!(store.is_unusable("charizard"));
    }

    #[tokio::test]
    async fn test_clear() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UnusableStore::load(dir.path()).await.unwrap();

        store
            .add(&["pikachu".to_string(), "charizard".to_string()])
            .await
            .unwrap();
        let count = store.clear().await.unwrap();
        assert_eq!(count, 2);
        assert_eq!(store.len(), 0);
    }

    #[tokio::test]
    async fn test_persistence() {
        let dir = tempfile::tempdir().unwrap();

        {
            let mut store = UnusableStore::load(dir.path()).await.unwrap();
            store
                .add(&["pikachu".to_string(), "charizard".to_string()])
                .await
                .unwrap();
        }

        // Reload from disk
        let store = UnusableStore::load(dir.path()).await.unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.is_unusable("pikachu"));
        assert!(store.is_unusable("charizard"));
    }

    #[tokio::test]
    async fn test_to_exclude_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UnusableStore::load(dir.path()).await.unwrap();

        store
            .add(&["charizard-mega-x".to_string(), "pikachu".to_string()])
            .await
            .unwrap();
        let exclude = store.to_exclude_list();
        assert_eq!(exclude, vec!["charizard-mega-x", "pikachu"]);
    }
}
