//! `NSUserDefaults`-backed [`KeyValueStore`] for iOS.
//!
//! [`run_app`](super::ffi::run_app) installs [`IosKeyValueStore`] before the
//! app callback runs, so `cx.key_value_store()` persists across launches
//! without any host code. Values live in `NSUserDefaults.standardUserDefaults`
//! under a `gpui:` prefix, which keeps [`KeyValueStore::keys`] free of the
//! system and framework entries that share the same defaults domain.

use anyhow::{Result, anyhow};
use gpui::key_value_store::KeyValueStore;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

const PREFIX: &str = "gpui:";

/// Key-value store persisted in `NSUserDefaults`.
#[derive(Debug, Default, Clone, Copy)]
pub struct IosKeyValueStore;

unsafe fn user_defaults() -> Result<*mut AnyObject> {
    let defaults: *mut AnyObject =
        unsafe { msg_send![class!(NSUserDefaults), standardUserDefaults] };
    if defaults.is_null() {
        Err(anyhow!("NSUserDefaults.standardUserDefaults unavailable"))
    } else {
        Ok(defaults)
    }
}

unsafe fn nsstring_to_string(string: *mut AnyObject) -> Option<String> {
    unsafe {
        if string.is_null() {
            return None;
        }
        let utf8: *const std::ffi::c_char = msg_send![string, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

fn full_key(key: &str) -> String {
    format!("{PREFIX}{key}")
}

impl KeyValueStore for IosKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        unsafe {
            let defaults = user_defaults()?;
            let key = super::util::nsstring(&full_key(key));
            let value: *mut AnyObject = msg_send![defaults, stringForKey: key];
            Ok(nsstring_to_string(value))
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        unsafe {
            let defaults = user_defaults()?;
            let key = super::util::nsstring(&full_key(key));
            let value = super::util::nsstring(value);
            let _: () = msg_send![defaults, setObject: value, forKey: key];
            Ok(())
        }
    }

    fn remove(&self, key: &str) -> Result<()> {
        unsafe {
            let defaults = user_defaults()?;
            let key = super::util::nsstring(&full_key(key));
            let _: () = msg_send![defaults, removeObjectForKey: key];
            Ok(())
        }
    }

    fn keys(&self) -> Result<Vec<String>> {
        unsafe {
            let defaults = user_defaults()?;
            let dictionary: *mut AnyObject = msg_send![defaults, dictionaryRepresentation];
            if dictionary.is_null() {
                return Ok(Vec::new());
            }
            let all_keys: *mut AnyObject = msg_send![dictionary, allKeys];
            let count: usize = msg_send![all_keys, count];
            let mut keys = Vec::new();
            for index in 0..count {
                let key: *mut AnyObject = msg_send![all_keys, objectAtIndex: index];
                if let Some(key) = nsstring_to_string(key)
                    && let Some(stripped) = key.strip_prefix(PREFIX)
                {
                    keys.push(stripped.to_owned());
                }
            }
            Ok(keys)
        }
    }
}
