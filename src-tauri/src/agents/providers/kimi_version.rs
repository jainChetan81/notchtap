//! Kimi hook version gate with pure version decisions and a bounded subprocess probe.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const MINIMUM_HOOK_VERSION: (u32, u32, u32) = (0, 9, 0);

/// Human-readable form of [`MINIMUM_HOOK_VERSION`], surfaced in `unavailable` diagnostics/status
/// output.
pub const MINIMUM_HOOK_VERSION_STR: &str = "0.9.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookSupport {
    /// The detected version meets [`MINIMUM_HOOK_VERSION`].
    Supported { detected: String },
    Unavailable {
        detected: Option<String>,
        minimum: &'static str,
    },
}

/// Returns `None` for anything that doesn't start with at least `major.minor` — a lone `major` is
/// unparseable; an ambiguous input must never read as "supported".
fn parse_semver_prefix(raw: &str) -> Option<(u32, u32, u32)> {
    let token = raw.split_whitespace().find(|tok| {
        tok.chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == 'v')
    })?;
    let token = token.trim_start_matches('v');
    let mut parts = token.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts
        .next()
        .and_then(|p| {
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                digits.parse().ok()
            }
        })
        .unwrap_or(0);
    Some((major, minor, patch))
}

/// Pure decision: does this Kimi version string advertise hook support? Never touches the
/// filesystem, network, or a subprocess.
pub fn hook_support(raw_version: &str) -> HookSupport {
    let Some(parsed) = parse_semver_prefix(raw_version) else {
        // Unparseable version string: never claim support.
        return HookSupport::Unavailable {
            detected: Some(raw_version.trim().to_string()),
            minimum: MINIMUM_HOOK_VERSION_STR,
        };
    };
    if parsed >= MINIMUM_HOOK_VERSION {
        HookSupport::Supported {
            detected: raw_version.trim().to_string(),
        }
    } else {
        HookSupport::Unavailable {
            detected: Some(raw_version.trim().to_string()),
            minimum: MINIMUM_HOOK_VERSION_STR,
        }
    }
}

/// Matches `delivery::DELIVERY_TIMEOUT`'s 750ms — a helper must never make the caller wait
/// perceptibly.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(10);

fn run_bounded(program: &str, args: &[&str], budget: Duration) -> Option<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now().checked_add(budget)?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let _ = child.wait();
                    return None;
                }
                return child.wait_with_output().ok().map(|o| o.stdout);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(PROBE_POLL_INTERVAL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// The impure half: shells out to `kimi --version` and returns whatever it printed.
pub fn detect_installed_version() -> Option<String> {
    let stdout = run_bounded("kimi", &["--version"], PROBE_TIMEOUT)?;
    let text = String::from_utf8(stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Combines the subprocess probe with the pure decision. `None` from the probe is `Unavailable`
/// with no detected version, never "assume supported".
pub fn probe_hook_support() -> HookSupport {
    match detect_installed_version() {
        Some(version) => hook_support(&version),
        None => HookSupport::Unavailable {
            detected: None,
            minimum: MINIMUM_HOOK_VERSION_STR,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_semver() {
        assert_eq!(parse_semver_prefix("0.9.0"), Some((0, 9, 0)));
        assert_eq!(parse_semver_prefix("1.2.3"), Some((1, 2, 3)));
    }

    #[test]
    fn parses_v_prefixed_and_labeled_output() {
        assert_eq!(parse_semver_prefix("kimi v0.20.1"), Some((0, 20, 1)));
        assert_eq!(parse_semver_prefix("v0.20.1"), Some((0, 20, 1)));
    }

    #[test]
    fn parses_build_metadata_suffix() {
        assert_eq!(parse_semver_prefix("0.9.0-beta.1"), Some((0, 9, 0)));
    }

    #[test]
    fn missing_patch_defaults_to_zero() {
        assert_eq!(parse_semver_prefix("0.9"), Some((0, 9, 0)));
    }

    #[test]
    fn unparseable_strings_return_none() {
        assert_eq!(parse_semver_prefix("kimi"), None);
        assert_eq!(parse_semver_prefix(""), None);
        assert_eq!(parse_semver_prefix("not a version"), None);
    }

    #[test]
    fn below_minimum_version_is_unavailable_with_minimum_reported() {
        let result = hook_support("0.8.0");
        assert_eq!(
            result,
            HookSupport::Unavailable {
                detected: Some("0.8.0".to_string()),
                minimum: MINIMUM_HOOK_VERSION_STR,
            }
        );
    }

    #[test]
    fn exactly_minimum_version_is_supported() {
        let result = hook_support(MINIMUM_HOOK_VERSION_STR);
        assert_eq!(
            result,
            HookSupport::Supported {
                detected: MINIMUM_HOOK_VERSION_STR.to_string()
            }
        );
    }

    #[test]
    fn above_minimum_version_is_supported() {
        let result = hook_support("1.0.0");
        assert_eq!(
            result,
            HookSupport::Supported {
                detected: "1.0.0".to_string()
            }
        );
    }

    #[test]
    fn unparseable_version_is_unavailable_not_assumed_supported() {
        let result = hook_support("nonsense");
        assert!(matches!(result, HookSupport::Unavailable { .. }));
    }

    #[test]
    fn run_bounded_returns_stdout_on_success() {
        let out = run_bounded("/bin/echo", &["hello"], Duration::from_secs(5))
            .expect("/bin/echo should succeed");
        assert_eq!(String::from_utf8(out).unwrap().trim(), "hello");
    }

    #[test]
    fn run_bounded_kills_a_child_that_outlives_its_budget() {
        let start = Instant::now();
        assert_eq!(
            run_bounded("/bin/sleep", &["5"], Duration::from_millis(100)),
            None
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn run_bounded_rejects_a_non_zero_exit() {
        assert_eq!(
            run_bounded("/bin/sh", &["-c", "exit 3"], Duration::from_secs(5)),
            None
        );
    }

    #[test]
    fn run_bounded_returns_none_when_the_program_does_not_exist() {
        assert_eq!(
            run_bounded(
                "/nonexistent/notchtap-probe-test",
                &[],
                Duration::from_secs(5)
            ),
            None
        );
    }
}
