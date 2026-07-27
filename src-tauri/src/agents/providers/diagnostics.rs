//! Plan 138 (spec §4.1): "exit 0 for delivery failures after writing a
//! bounded diagnostic to notchtap's adapter log, never stdout" — this is
//! that adapter log. Deliberately its own file/format rather than
//! reusing `crate::logging`'s `tracing`-based appender: that module
//! (`mod logging;` in `lib.rs`) is private and its `log_dir`/rotation
//! machinery isn't reachable from outside the crate, and standing up a
//! second `tracing` subscriber inside a short-lived CLI process (one
//! that must never print to stdout, and whose whole job is a single
//! POST-and-exit) is more machinery than a handful of best-effort log
//! lines need. Same directory convention as `logging.rs::log_dir`
//! (`~/Library/Logs/notchtap/`), a different filename
//! (`notchtap-agent.log`) so the two never interleave or race on
//! rotation.
//!
//! Every write here is best-effort: a failure to log is swallowed, never
//! propagated — this module exists to explain a fail-open exit, not to
//! become a second thing that can fail the hook.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// A single diagnostic message is capped well under the wire schema's
/// own summary cap (spec §3.2: 500 scalars) — this is a one-line log
/// entry, not a dump.
const MAX_DIAGNOSTIC_CHARS: usize = 300;
/// Same order of magnitude as `logging.rs`'s 10 MiB rotation cap, but
/// this file just resets rather than rotating backups — adapter
/// diagnostics are a debugging aid, not an audit trail worth keeping
/// generations of.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

fn log_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join("Library").join("Logs").join("notchtap");
    fs::create_dir_all(&dir).ok()?;
    // matches `logging.rs::log_dir`'s 0700 posture — this file can carry
    // sanitized-but-still-somewhat-descriptive hook diagnostics (error
    // categories, native event names), same non-world-readable bar.
    #[cfg(unix)]
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    Some(dir)
}

fn cap_message(message: &str) -> String {
    if message.chars().count() <= MAX_DIAGNOSTIC_CHARS {
        message.to_string()
    } else {
        let mut s: String = message.chars().take(MAX_DIAGNOSTIC_CHARS).collect();
        s.push('…');
        s
    }
}

/// Writes one bounded diagnostic line, prefixed with a UTC timestamp and
/// `context` (e.g. `"claude-code deliver"`, `"claude-code parse"`).
/// Never writes to stdout/stderr — see this module's top doc for why
/// hook mode can't use either.
pub fn log_diagnostic(context: &str, message: &str) {
    let Some(dir) = log_dir() else { return };
    log_diagnostic_to(&dir, context, message);
}

/// The testable core of [`log_diagnostic`], split out so tests can point
/// it at a throwaway temp dir instead of mutating the process-global
/// `HOME` env var (which would race against every other test in this
/// binary that also resolves `dirs::home_dir()`, since `cargo test` runs
/// a crate's tests on multiple threads of one process by default).
fn log_diagnostic_to(dir: &Path, context: &str, message: &str) {
    let path = dir.join("notchtap-agent.log");

    // Best-effort size cap: reset rather than append-forever. A CLI
    // helper invoked once per hook event has no in-process rotation
    // worker like `logging.rs`'s appender, so this simple check-on-write
    // is the whole mechanism.
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = fs::remove_file(&path);
        }
    }

    let line = format!(
        "{} {context}: {}\n",
        chrono::Utc::now().to_rfc3339(),
        cap_message(message)
    );

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    if let Ok(mut file) = options.open(&path) {
        #[cfg(unix)]
        let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("notchtap-agent-diag-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_a_line_containing_context_and_message() {
        let dir = temp_dir();
        log_diagnostic_to(&dir, "test-context", "test-message");
        let contents = fs::read_to_string(dir.join("notchtap-agent.log")).unwrap();
        assert!(contents.contains("test-context"));
        assert!(contents.contains("test-message"));
    }

    #[test]
    fn overlong_message_is_capped() {
        let dir = temp_dir();
        let long = "x".repeat(MAX_DIAGNOSTIC_CHARS + 50);
        log_diagnostic_to(&dir, "ctx", &long);
        let contents = fs::read_to_string(dir.join("notchtap-agent.log")).unwrap();
        // capped text plus ellipsis, well short of the uncapped length
        assert!(contents.len() < long.len());
    }

    #[cfg(unix)]
    #[test]
    fn log_file_is_0600() {
        let dir = temp_dir();
        log_diagnostic_to(&dir, "ctx", "msg");
        let file_mode = fs::metadata(dir.join("notchtap-agent.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
    }

    #[test]
    fn real_log_dir_resolves_under_library_logs_notchtap() {
        // Exercises the real (non-test) `log_dir` path at least once —
        // this is the only test allowed to touch the developer's actual
        // `~/Library/Logs/notchtap/`, and it only reads the path shape,
        // never asserts on file contents (other tests already run
        // against this real dir via `log_diagnostic` in practice, so a
        // stray line here is expected and harmless).
        if let Some(dir) = log_dir() {
            assert!(dir.ends_with("Library/Logs/notchtap"));
        }
    }
}
