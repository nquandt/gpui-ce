//! A small, platform-neutral key-value store for app settings and state.
//!
//! Every platform backend installs a store that fits the host: `localStorage`
//! on the web, a JSON file in the user's data directory on desktop, and the
//! system preferences on mobile. Application code only talks to
//! [`KeyValueStore`] through [`App::key_value_store`](crate::App::key_value_store),
//! so one app core persists the same way on every target.
//!
//! The API is synchronous and string-based on purpose. The backing stores are
//! all synchronous, values are small, and JSON helpers cover structured data.
//! Large or relational data belongs in a real database, not here.

use anyhow::Result;
use parking_lot::Mutex;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

/// Persistent string key-value storage.
///
/// Implementors provide the four primitive methods. JSON helpers come from
/// [`KeyValueStoreExt`].
pub trait KeyValueStore: 'static + Send + Sync {
    /// Read the value stored under `key`.
    fn get(&self, key: &str) -> Result<Option<String>>;

    /// Store `value` under `key`, replacing any existing value.
    fn set(&self, key: &str, value: &str) -> Result<()>;

    /// Remove the value stored under `key`. Removing a missing key is not an error.
    fn remove(&self, key: &str) -> Result<()>;

    /// All keys currently stored, in no particular order.
    fn keys(&self) -> Result<Vec<String>>;
}

/// JSON helpers available on every [`KeyValueStore`], including
/// `Arc<dyn KeyValueStore>`.
///
/// Generic methods cannot live on an object-safe trait, so they are provided
/// through this blanket extension instead. Import it to use them.
pub trait KeyValueStoreExt {
    /// Read and deserialize a JSON value stored under `key`.
    fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;

    /// Serialize `value` as JSON and store it under `key`.
    fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
}

impl<S: KeyValueStore + ?Sized> KeyValueStoreExt for S {
    fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get(key)? {
            Some(text) => Ok(Some(serde_json::from_str(&text)?)),
            None => Ok(None),
        }
    }

    fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set(key, &serde_json::to_string(value)?)
    }
}

/// A store that keeps values in memory only.
///
/// This is the default when no platform store is installed. Nothing survives
/// a restart, which makes it right for tests and for hosts without storage.
#[derive(Default)]
pub struct MemoryKeyValueStore {
    entries: Mutex<BTreeMap<String, String>>,
}

impl MemoryKeyValueStore {
    /// Create an empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeyValueStore for MemoryKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.entries.lock().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.entries.lock().insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<()> {
        self.entries.lock().remove(key);
        Ok(())
    }

    fn keys(&self) -> Result<Vec<String>> {
        Ok(self.entries.lock().keys().cloned().collect())
    }
}

/// A store backed by one JSON file, written through on every change.
///
/// Suitable for desktop apps. The whole map lives in memory and is rewritten
/// on each `set` or `remove`, which is fine for settings-sized data.
#[cfg(not(target_family = "wasm"))]
pub struct FileKeyValueStore {
    path: std::path::PathBuf,
    entries: Mutex<BTreeMap<String, String>>,
}

#[cfg(not(target_family = "wasm"))]
impl FileKeyValueStore {
    /// Open or create the store at `path`. A missing or unreadable file
    /// starts empty; the parent directory is created on first write.
    pub fn open(path: impl Into<std::path::PathBuf>) -> Self {
        let path = path.into();
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    /// Open the store for `app_name` in the user's data directory:
    /// `%APPDATA%\<app>` on Windows, `~/Library/Application Support/<app>` on
    /// macOS, and `$XDG_DATA_HOME/<app>` or `~/.local/share/<app>` elsewhere.
    pub fn for_app(app_name: &str) -> Self {
        Self::open(Self::data_dir().join(app_name).join("key_value_store.json"))
    }

    /// The path of the backing file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn data_dir() -> std::path::PathBuf {
        use std::env::var_os;
        use std::path::PathBuf;
        if cfg!(target_os = "windows") {
            if let Some(dir) = var_os("APPDATA") {
                return PathBuf::from(dir);
            }
        } else if cfg!(target_os = "macos") {
            if let Some(home) = var_os("HOME") {
                return PathBuf::from(home).join("Library/Application Support");
            }
        } else {
            if let Some(dir) = var_os("XDG_DATA_HOME") {
                return PathBuf::from(dir);
            }
            if let Some(home) = var_os("HOME") {
                return PathBuf::from(home).join(".local/share");
            }
        }
        std::env::temp_dir()
    }

    fn write(&self, entries: &BTreeMap<String, String>) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(entries)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(not(target_family = "wasm"))]
impl KeyValueStore for FileKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.entries.lock().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut entries = self.entries.lock();
        entries.insert(key.to_owned(), value.to_owned());
        self.write(&entries)
    }

    fn remove(&self, key: &str) -> Result<()> {
        let mut entries = self.entries.lock();
        if entries.remove(key).is_some() {
            self.write(&entries)?;
        }
        Ok(())
    }

    fn keys(&self) -> Result<Vec<String>> {
        Ok(self.entries.lock().keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryKeyValueStore::new();
        assert_eq!(store.get("a").unwrap(), None);
        store.set("a", "1").unwrap();
        store.set("b", "2").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("1"));
        assert_eq!(store.keys().unwrap(), vec!["a", "b"]);
        store.remove("a").unwrap();
        store.remove("missing").unwrap();
        assert_eq!(store.keys().unwrap(), vec!["b"]);
    }

    #[test]
    fn json_helpers_work_on_trait_objects() {
        let store: Arc<dyn KeyValueStore> = Arc::new(MemoryKeyValueStore::new());
        store.set_json("point", &(1, 2)).unwrap();
        let point: Option<(i32, i32)> = store.get_json("point").unwrap();
        assert_eq!(point, Some((1, 2)));
        let missing: Option<(i32, i32)> = store.get_json("nope").unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn file_store_persists_across_opens() {
        let dir = std::env::temp_dir().join(format!("gpui-kv-test-{}", std::process::id()));
        let path = dir.join("nested").join("store.json");
        {
            let store = FileKeyValueStore::open(&path);
            store.set("theme", "dark").unwrap();
            store.set("gone", "x").unwrap();
            store.remove("gone").unwrap();
        }
        let store = FileKeyValueStore::open(&path);
        assert_eq!(store.get("theme").unwrap().as_deref(), Some("dark"));
        assert_eq!(store.keys().unwrap(), vec!["theme"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
