//! Plan 088: append-only JSONL notification history, gated behind the
//! opt-in `history_enabled` config flag (default `false`, see
//! `config.rs`). Backend-only — no invoke command reads this yet
//! (that's plan 089); today the only writer is `Engine::accept` and the
//! only readers are this module's own tests.
//!
//! DELIBERATE DIVERGENCE from `logging.rs`'s `SizeRotatingAppender`: this
//! store stats the file on every `append` instead of caching an open file
//! handle and a running size. `logging.rs` backs `tracing` and writes
//! potentially thousands of lines per second, so it needs the cached
//! handle to avoid a syscall per write. History writes a handful of lines
//! per hour (one per accepted one-shot notification), so the simpler
//! stat-per-append approach avoids holding a file handle open inside the
//! `Engine` for the entire lifetime of the app — this is a considered
//! choice, not an oversight; do not "fix" it into a shared abstraction
//! with `logging.rs`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::event::Event;

const HISTORY_FILENAME: &str = "history.jsonl";
const DEFAULT_MAX_SIZE: u64 = 5 * 1024 * 1024;
const DEFAULT_MAX_FILES: usize = 2;

/// One recorded notification. `recorded_at_ms` is wall-clock epoch millis
/// (the queue's own timing is `Instant`-based and deliberately clock-free,
/// so history stamps its own time here). Matches the `_ms: i64` convention
/// already used by `EventMeta::published_at_ms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub recorded_at_ms: i64,
    pub event: Event,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Append-only JSONL store for accepted notifications, size-rotated like
/// `logging.rs`'s appender but with its own (simpler, stat-per-append)
/// write path — see the module doc for why the two are not shared.
pub struct HistoryStore {
    dir: PathBuf,
    max_size: u64,
    max_files: usize,
    lock: Mutex<()>,
}

impl HistoryStore {
    /// `dir/history.jsonl`, defaults: 5MB cap, current + one `.1` backup.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        Self::with_limits(dir, DEFAULT_MAX_SIZE, DEFAULT_MAX_FILES)
    }

    /// Same as `new`, with explicit rotation caps. Tests MUST use this —
    /// a test that writes to the real `~/.config/notchtap/` is a bug.
    pub fn with_limits(dir: impl AsRef<Path>, max_size: u64, max_files: usize) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            max_size,
            max_files,
            lock: Mutex::new(()),
        })
    }

    fn path(&self) -> PathBuf {
        self.dir.join(HISTORY_FILENAME)
    }

    fn backup_path(&self, i: usize) -> PathBuf {
        self.dir.join(format!("{HISTORY_FILENAME}.{i}"))
    }

    /// Serialize `event` as one `HistoryEntry` line, rotating first if the
    /// new line would push the current file over `max_size` (same
    /// predicate as `logging.rs`: `size + line_len > max_size && size >
    /// 0` — an oversized single line lands whole in an empty file rather
    /// than rotating forever), then append it.
    pub fn append(&self, event: &Event) -> io::Result<()> {
        let _guard = self.lock.lock().unwrap();

        let entry = HistoryEntry {
            recorded_at_ms: now_ms(),
            event: event.clone(),
        };
        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');

        let path = self.path();
        let current_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if current_size + line.len() as u64 > self.max_size && current_size > 0 {
            self.rotate_locked()?;
        }

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    fn rotate_locked(&self) -> io::Result<()> {
        for i in (1..self.max_files).rev() {
            let src = self.backup_path(i);
            let dst = self.backup_path(i + 1);
            if src.exists() {
                fs::rename(&src, &dst)?;
            }
        }
        let current = self.path();
        let backup = self.backup_path(1);
        fs::rename(&current, &backup)?;
        Ok(())
    }

    /// Read the last `n` entries from the current file only (rotated
    /// backups stay out of scope, same choice `logging.rs::read_recent_lines`
    /// documents). A missing file reads as an empty Vec, not an error.
    /// Lines that fail to parse are skipped — a crash mid-write can leave
    /// a torn final line, and one bad line must never poison the whole
    /// read. Returned oldest -> newest, same ordering contract as
    /// `read_recent_lines`.
    ///
    /// Plan 088 ships this store dark: no invoke command reads it yet
    /// (that's plan 089's settings-window read surface). Today it's
    /// exercised only by this module's own tests and `engine.rs`'s
    /// history-hook tests — the same "seam exists before it's needed"
    /// shape as `Engine::apply`'s doc comment.
    #[allow(dead_code)]
    pub fn read_recent(&self, n: usize) -> io::Result<Vec<HistoryEntry>> {
        let _guard = self.lock.lock().unwrap();

        let contents = match fs::read_to_string(self.path()) {
            Ok(contents) => contents,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let entries: Vec<HistoryEntry> = contents
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        let start = entries.len().saturating_sub(n);
        Ok(entries[start..].to_vec())
    }

    /// Remove the current file and every rotated backup. A missing file
    /// is success, not an error.
    ///
    /// Same "not wired up yet" status as `read_recent` above — this is
    /// the manual "Clear history" control's future backing call
    /// (plan 059 decision #2), not reachable from production code until
    /// plan 089 adds its invoke command.
    #[allow(dead_code)]
    pub fn clear(&self) -> io::Result<()> {
        let _guard = self.lock.lock().unwrap();

        remove_if_exists(&self.path())?;
        for i in 1..=self.max_files {
            remove_if_exists(&self.backup_path(i))?;
        }
        Ok(())
    }
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::test_fixtures;
    use uuid::Uuid;

    // a fresh, unique dir per test, same rationale as logging.rs's
    // temp_dir(): a shared dir would silently shift the rotation
    // arithmetic between tests.
    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("notchtap-historytest-{}", Uuid::new_v4()))
    }

    #[test]
    fn append_then_read_recent_round_trips() {
        let dir = temp_dir();
        let store = HistoryStore::with_limits(&dir, DEFAULT_MAX_SIZE, DEFAULT_MAX_FILES).unwrap();

        for title in ["one", "two", "three"] {
            store.append(&test_fixtures::event(title)).unwrap();
        }

        let entries = store.read_recent(3).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].event.payload.title, "one");
        assert_eq!(entries[1].event.payload.title, "two");
        assert_eq!(entries[2].event.payload.title, "three");
        assert_eq!(entries[0].event.payload.body, "body");
        assert_eq!(entries[0].event.origin, crate::event::SourceKind::Manual);
    }

    #[test]
    fn missing_file_reads_as_empty_not_error() {
        let dir = temp_dir();
        let store = HistoryStore::with_limits(&dir, DEFAULT_MAX_SIZE, DEFAULT_MAX_FILES).unwrap();

        assert!(store.read_recent(10).unwrap().is_empty());
    }

    #[test]
    fn read_recent_returns_only_the_last_n() {
        let dir = temp_dir();
        let store = HistoryStore::with_limits(&dir, DEFAULT_MAX_SIZE, DEFAULT_MAX_FILES).unwrap();

        for i in 1..=10 {
            store
                .append(&test_fixtures::event(&format!("item-{i}")))
                .unwrap();
        }

        let entries = store.read_recent(3).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].event.payload.title, "item-8");
        assert_eq!(entries[1].event.payload.title, "item-9");
        assert_eq!(entries[2].event.payload.title, "item-10");
    }

    #[test]
    fn malformed_line_is_skipped_not_fatal() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let store = HistoryStore::with_limits(&dir, DEFAULT_MAX_SIZE, DEFAULT_MAX_FILES).unwrap();

        let good1 = serde_json::to_string(&HistoryEntry {
            recorded_at_ms: 1,
            event: test_fixtures::event("good-1"),
        })
        .unwrap();
        let good2 = serde_json::to_string(&HistoryEntry {
            recorded_at_ms: 2,
            event: test_fixtures::event("good-2"),
        })
        .unwrap();
        let contents = format!("{good1}\nnot valid json at all\n{good2}\n");
        fs::write(store.path(), contents).unwrap();

        let entries = store.read_recent(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].event.payload.title, "good-1");
        assert_eq!(entries[1].event.payload.title, "good-2");
    }

    #[test]
    fn rotation_at_threshold_creates_backup() {
        let dir = temp_dir();
        // small cap so a couple of appends cross the threshold
        let store = HistoryStore::with_limits(&dir, 100, 2).unwrap();

        // first append is small and well under the cap
        store.append(&test_fixtures::event("a")).unwrap();
        assert!(!dir.join("history.jsonl.1").exists());

        // pad subsequent appends with a long body so the cumulative size
        // crosses 100 bytes and triggers rotation on a later append
        let padded = test_fixtures::with_body(test_fixtures::event("b"), &"x".repeat(150));
        store.append(&padded).unwrap();

        assert!(
            dir.join("history.jsonl.1").exists(),
            "expected rotation to create a .1 backup once the threshold was crossed"
        );
        // the live file restarted with just the triggering append
        let current = fs::read_to_string(dir.join("history.jsonl")).unwrap();
        assert_eq!(current.lines().count(), 1);
        assert!(current.contains("\"b\""));
    }

    #[test]
    fn empty_current_file_never_rotates() {
        let dir = temp_dir();
        let store = HistoryStore::with_limits(&dir, 10, 2).unwrap();

        // size is 0 going in, so even though this single line exceeds
        // max_size, the guard (`current_size > 0`) skips rotation — the
        // oversized line lands whole in the current (empty) file.
        let big = test_fixtures::with_body(test_fixtures::event("big"), &"y".repeat(200));
        store.append(&big).unwrap();

        assert!(!dir.join("history.jsonl.1").exists());
        let current = fs::read_to_string(dir.join("history.jsonl")).unwrap();
        assert_eq!(current.lines().count(), 1);
    }

    #[test]
    fn clear_removes_current_and_backup() {
        let dir = temp_dir();
        let store = HistoryStore::with_limits(&dir, 100, 2).unwrap();

        store.append(&test_fixtures::event("a")).unwrap();
        let padded = test_fixtures::with_body(test_fixtures::event("b"), &"x".repeat(150));
        store.append(&padded).unwrap();
        assert!(dir.join("history.jsonl.1").exists());

        store.clear().unwrap();

        assert!(!dir.join("history.jsonl").exists());
        assert!(!dir.join("history.jsonl.1").exists());
        assert!(store.read_recent(10).unwrap().is_empty());
    }
}
