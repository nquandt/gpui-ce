//! `localStorage`-backed [`KeyValueStore`] for the browser.
//!
//! `localStorage` is synchronous and only exists on the main thread, so calls
//! from a web worker fail with an error rather than panicking. Every access
//! looks the storage object up again; no JavaScript value is held across
//! calls, which is what lets the store be `Send + Sync`.

use anyhow::{Result, anyhow};
use gpui::key_value_store::KeyValueStore;

/// Key-value store persisted in the browser's `localStorage`.
///
/// Keys are prefixed so several GPUI apps served from one origin do not
/// collide, and so that unrelated `localStorage` entries never show up in
/// [`KeyValueStore::keys`].
pub struct LocalStorageKeyValueStore {
    prefix: String,
}

impl Default for LocalStorageKeyValueStore {
    fn default() -> Self {
        Self::with_prefix("gpui:")
    }
}

impl LocalStorageKeyValueStore {
    /// Create a store whose keys all begin with `prefix`.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    fn storage() -> Result<web_sys::Storage> {
        let window = web_sys::window()
            .ok_or_else(|| anyhow!("localStorage is only available on the main thread"))?;
        window
            .local_storage()
            .map_err(|error| anyhow!("localStorage access failed: {error:?}"))?
            .ok_or_else(|| anyhow!("localStorage is disabled in this browser"))
    }

    fn full_key(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }
}

impl KeyValueStore for LocalStorageKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Self::storage()?
            .get_item(&self.full_key(key))
            .map_err(|error| anyhow!("localStorage.getItem failed: {error:?}"))
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        Self::storage()?
            .set_item(&self.full_key(key), value)
            .map_err(|error| anyhow!("localStorage.setItem failed (quota?): {error:?}"))
    }

    fn remove(&self, key: &str) -> Result<()> {
        Self::storage()?
            .remove_item(&self.full_key(key))
            .map_err(|error| anyhow!("localStorage.removeItem failed: {error:?}"))
    }

    fn keys(&self) -> Result<Vec<String>> {
        let storage = Self::storage()?;
        let count = storage
            .length()
            .map_err(|error| anyhow!("localStorage.length failed: {error:?}"))?;
        let mut keys = Vec::new();
        for index in 0..count {
            let Ok(Some(key)) = storage.key(index) else {
                continue;
            };
            if let Some(stripped) = key.strip_prefix(&self.prefix) {
                keys.push(stripped.to_owned());
            }
        }
        Ok(keys)
    }
}
