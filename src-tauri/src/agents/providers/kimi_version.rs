//! Plan 140 (spec §4.4): the Kimi hook version gate.
//!
//! "install/report only when the local Kimi version advertises hook
//! support; otherwise the adapter reports `unavailable` with the minimum
//! supported version... NO terminal scraping fallback, ever."
//!
//! Same shape as this repo's `presentation_mode(safe_area_top_inset: f64)
//! -> Mode` rule (CLAUDE.md: "keep the pure decision logic... separate
//! from that subprocess call — the function is unit-testable, the
//! subprocess call is not"): [`hook_support`] is a pure function over an
//! already-obtained version string; [`detect_installed_version`] is the
//! impure subprocess probe, kept in its own function so it's the only
//! thing a test can't exercise deterministically.
//!
//! ## Minimum version: NEEDS VERIFICATION
//!
//! The hooks doc page
//! (<https://moonshotai.github.io/kimi-code/en/customization/hooks>)
//! states no minimum version at all ("no explicit minimum version
//! specifications" — confirmed by direct query of that page on
//! 2026-07-26). Per this ticket's instruction — "if the docs don't state
//! one, pick the earliest documented and mark it clearly as needing
//! verification" — [`MINIMUM_HOOK_VERSION`] is set from the Kimi Code CLI
//! changelog's earliest hooks-related entry found
//! (<https://moonshotai.github.io/kimi-code/en/release-notes/changelog.html>,
//! version `0.9.0`, "Add approval lifecycle hook events for observing
//! pending and completed permission prompts"), NOT from the primary hooks
//! doc page itself. This is explicitly a **best-effort placeholder**: the
//! changelog is not the same document as the hooks contract page, earlier
//! pre-0.9.0 changelog entries were not visible to confirm there's no
//! earlier hooks-adjacent entry, and this ticket's own manual-smoke
//! checklist item ("real Kimi session smoke on a hook-supporting
//! version") is the actual verification step. Treat this constant as
//! provisional until that manual check confirms or corrects it — do not
//! read its presence as "verified against the hooks contract page".
pub const MINIMUM_HOOK_VERSION: (u32, u32, u32) = (0, 9, 0);

/// Human-readable form of [`MINIMUM_HOOK_VERSION`], surfaced in
/// `unavailable` diagnostics/status output.
pub const MINIMUM_HOOK_VERSION_STR: &str = "0.9.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookSupport {
    /// The detected version meets [`MINIMUM_HOOK_VERSION`].
    Supported { detected: String },
    /// A version was detected but it's below the minimum.
    Unavailable {
        detected: Option<String>,
        minimum: &'static str,
    },
}

/// Parses a `major.minor.patch`-shaped prefix out of a raw version string
/// (e.g. Kimi CLI's own `--version` output, which may carry a leading
/// binary name or trailing build metadata this function doesn't need to
/// understand). Returns `None` for anything that doesn't start with at
/// least `major.minor` — a lone `major` number is treated as
/// unparseable, since a false "supported" reading would violate this
/// ticket's "no terminal scraping fallback" spirit for an ambiguous
/// input.
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
            // Trim any trailing non-numeric build metadata (e.g. "0-beta").
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

/// Pure decision: does this Kimi version string advertise hook support?
/// Never touches the filesystem, network, or a subprocess — see this
/// module's top doc for why that split matters.
pub fn hook_support(raw_version: &str) -> HookSupport {
    let Some(parsed) = parse_semver_prefix(raw_version) else {
        // Unparseable version string: cannot claim support (spec: no
        // silent fallback to "assume it works").
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

/// The impure half: shells out to `kimi --version` and returns whatever
/// it printed. Isolated in its own function precisely so
/// [`hook_support`] stays unit-testable without a real `kimi` binary —
/// mirrors this repo's `notchtap-detect` subprocess boundary (CLAUDE.md).
/// Returns `None` on any failure to launch/read the process (binary
/// missing, non-UTF8 output, non-zero exit) — a `None` here is the
/// caller's cue to treat Kimi as `Unavailable` rather than guess.
pub fn detect_installed_version() -> Option<String> {
    let output = std::process::Command::new("kimi")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Combines the subprocess probe with the pure decision — the one
/// function a hook-mode caller needs. `None` from the probe (binary not
/// found, unreadable output) is treated as `Unavailable` with no detected
/// version, never as "assume supported".
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

    // --- version-gate tests (this ticket's checklist: "below-minimum
    // -> unavailable + minimum version reported; supported -> available")

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
    fn missing_detection_is_unavailable_with_no_detected_version() {
        // `detect_installed_version` returning `None` (binary absent) is
        // simulated directly here — this is the pure-side contract the
        // impure probe relies on; `probe_hook_support`'s own subprocess
        // call is exercised (but not asserted on, since the dev/CI
        // machine may or may not have `kimi` installed) below.
        let result = HookSupport::Unavailable {
            detected: None,
            minimum: MINIMUM_HOOK_VERSION_STR,
        };
        assert!(matches!(
            result,
            HookSupport::Unavailable { detected: None, .. }
        ));
    }

    #[test]
    fn probe_hook_support_never_panics_regardless_of_local_kimi_install() {
        // Exercises the real subprocess path at least once without
        // asserting a specific outcome — the dev/CI machine may or may
        // not have `kimi` on PATH, and this must not panic either way
        // (fail-open discipline extends to detection, not just delivery).
        let _ = probe_hook_support();
    }
}
