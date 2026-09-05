//! A small in-memory ring buffer of timestamped events, used by the shell
//! and by individual screens to record what happened during manual testing.
//! The Report screen reads the last 200 entries.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const CAPACITY: usize = 500;

struct LogState {
    start: Instant,
    entries: Vec<String>,
}

static STATE: OnceLock<Mutex<LogState>> = OnceLock::new();

fn state() -> &'static Mutex<LogState> {
    STATE.get_or_init(|| {
        Mutex::new(LogState {
            start: Instant::now(),
            entries: Vec::new(),
        })
    })
}

/// Append an entry to the global event log, prefixed with the elapsed time
/// (in seconds) since the log was first touched (effectively since launch).
pub fn push(message: impl Into<String>) {
    let mut guard = state().lock().unwrap();
    let elapsed = guard.start.elapsed().as_secs_f32();
    let line = format!("[{elapsed:8.3}s] {}", message.into());
    if guard.entries.len() >= CAPACITY {
        guard.entries.remove(0);
    }
    guard.entries.push(line);
}

/// Return a snapshot of every entry currently in the log, oldest first.
pub fn all() -> Vec<String> {
    state().lock().unwrap().entries.clone()
}

/// Return a snapshot of the last `n` entries, oldest first.
pub fn last(n: usize) -> Vec<String> {
    let entries = &state().lock().unwrap().entries;
    let start = entries.len().saturating_sub(n);
    entries[start..].to_vec()
}

/// Clear the event log.
pub fn clear() {
    state().lock().unwrap().entries.clear();
}
