//! `SharedPreferences`-backed [`KeyValueStore`] for Android.
//!
//! Android apps build their own `Application`, so nothing installs this
//! automatically. Set it from the launch callback:
//!
//! ```ignore
//! cx.set_key_value_store(std::sync::Arc::new(gpui_mobile::android::AndroidKeyValueStore));
//! ```
//!
//! Values live in the app's default `SharedPreferences` under a `gpui:` prefix
//! so [`KeyValueStore::keys`] only reports entries written through this store.

use crate::packages::shared_preferences::SharedPreferences;
use anyhow::{Result, anyhow};
use gpui::key_value_store::KeyValueStore;

const PREFIX: &str = "gpui:";

/// Key-value store persisted in the default `SharedPreferences`.
#[derive(Debug, Default, Clone, Copy)]
pub struct AndroidKeyValueStore;

fn full_key(key: &str) -> String {
    format!("{PREFIX}{key}")
}

impl KeyValueStore for AndroidKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(SharedPreferences::instance().get_string(&full_key(key)))
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        SharedPreferences::instance()
            .set_string(&full_key(key), value)
            .map_err(|error| anyhow!("SharedPreferences putString failed: {error}"))
    }

    fn remove(&self, key: &str) -> Result<()> {
        SharedPreferences::instance()
            .remove(&full_key(key))
            .map_err(|error| anyhow!("SharedPreferences remove failed: {error}"))
    }

    fn keys(&self) -> Result<Vec<String>> {
        Ok(SharedPreferences::instance()
            .keys()
            .into_iter()
            .filter_map(|key| key.strip_prefix(PREFIX).map(str::to_owned))
            .collect())
    }
}
