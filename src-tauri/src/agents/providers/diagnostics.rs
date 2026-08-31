//! Bounded adapter diagnostics written to a dedicated log, never stdout.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const MAX_DIAGNOSTIC_CHARS: usize = 300;
const MAX_LOG_BYTES: u64 = 1024 * 1024;

fn log_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join("Library").join("Logs").join("notchtap");
    fs::create_dir_all(&dir).ok()?;
    // Matches `logging.rs::log_dir`'s 0700 posture — diagnostics carry native event names, so keep
    // the dir non-world-readable.
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

pub fn log_diagnostic(context: &str, message: &str) {
    let Some(dir) = log_dir() else { return };
    log_diagnostic_to(&dir, context, message);
}

/// The testable core of [`log_diagnostic`], split out so tests can point it at a temp dir instead
/// of mutating process-global `HOME`, which races other threads resolving `dirs::home_dir()`.
fn log_diagnostic_to(dir: &Path, context: &str, message: &str) {
    let path = dir.join("notchtap-agent.log");

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
        if let Some(dir) = log_dir() {
            assert!(dir.ends_with("Library/Logs/notchtap"));
        }
    }
}
