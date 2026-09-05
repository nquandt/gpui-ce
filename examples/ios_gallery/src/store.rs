//! Persistence for per-screen verdicts and notes, backed by
//! `gpui_mobile::packages::shared_preferences::SharedPreferences`.

use gpui_mobile::packages::shared_preferences::SharedPreferences;
use std::collections::HashMap;

/// The manual test verdict recorded for a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    #[default]
    Untested,
    Works,
    Partial,
    Broken,
}

impl Verdict {
    pub fn glyph(self) -> &'static str {
        match self {
            Verdict::Untested => "○",
            Verdict::Works => "✓",
            Verdict::Partial => "◐",
            Verdict::Broken => "✗",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Verdict::Untested => "Untested",
            Verdict::Works => "Works",
            Verdict::Partial => "Partial",
            Verdict::Broken => "Broken",
        }
    }

    fn to_store_str(self) -> &'static str {
        match self {
            Verdict::Untested => "untested",
            Verdict::Works => "works",
            Verdict::Partial => "partial",
            Verdict::Broken => "broken",
        }
    }

    fn from_store_str(s: &str) -> Self {
        match s {
            "works" => Verdict::Works,
            "partial" => Verdict::Partial,
            "broken" => Verdict::Broken,
            _ => Verdict::Untested,
        }
    }
}

fn verdict_key(id: &str) -> String {
    format!("gallery.{id}.verdict")
}

fn notes_key(id: &str) -> String {
    format!("gallery.{id}.notes")
}

/// Load the verdict recorded for a screen id.
pub fn load_verdict(id: &str) -> Verdict {
    SharedPreferences::instance()
        .get_string(&verdict_key(id))
        .map(|s| Verdict::from_store_str(&s))
        .unwrap_or_default()
}

/// Persist the verdict for a screen id.
pub fn save_verdict(id: &str, verdict: Verdict) {
    let _ = SharedPreferences::instance().set_string(&verdict_key(id), verdict.to_store_str());
}

/// Load the notes recorded for a screen id.
pub fn load_notes(id: &str) -> String {
    SharedPreferences::instance()
        .get_string(&notes_key(id))
        .unwrap_or_default()
}

/// Persist the notes for a screen id.
pub fn save_notes(id: &str, notes: &str) {
    let _ = SharedPreferences::instance().set_string(&notes_key(id), notes);
}

/// Load every screen's verdict and notes, keyed by screen id.
pub fn load_all(ids: &[&'static str]) -> HashMap<&'static str, (Verdict, String)> {
    ids.iter()
        .map(|&id| (id, (load_verdict(id), load_notes(id))))
        .collect()
}

/// Clear every screen's persisted verdict and notes.
pub fn reset_all(ids: &[&'static str]) {
    let prefs = SharedPreferences::instance();
    for id in ids {
        let _ = prefs.remove(&verdict_key(id));
        let _ = prefs.remove(&notes_key(id));
    }
}
