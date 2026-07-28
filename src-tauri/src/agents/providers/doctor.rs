//! Plan 152: the read-only setup-inspection half of `notchtap-agent
//! doctor`.
//!
//! The Settings window's Adapter Health cards only report what has been
//! *received*, so an un-wired Agent Runtime and a correctly-wired-but-idle
//! one look identical ("no events yet"). This module answers the other
//! question: is the Agent Adapter actually installed, and does the command
//! its hooks point at resolve to something executable?
//!
//! **This module NEVER writes.** It reads each runtime's hook config file
//! and reports what it finds. It must never create, edit, or repair one —
//! spec §4.6: "v7 does not silently edit a user's global provider
//! configuration".
//!
//! Same pure/impure split this repo applies to `presentation_mode` and
//! [`super::kimi_version`] (CLAUDE.md: "keep the pure decision logic...
//! separate from that subprocess call — the function is unit-testable, the
//! subprocess call is not"): every decision function here takes file
//! *contents* (a `&str`), a `bool`, or an explicit `&Path`, and resolves
//! nothing from the environment. The handful of impure helpers
//! ([`is_executable_file`], [`path_dirs_from_env`], [`listener_reachable`])
//! are deliberately tiny and hold no decision logic, so tests never need to
//! touch the real home directory (see [`super::diagnostics`] for why that
//! matters in this test binary).

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The hook events `AgentsSection.tsx`'s Claude Code setup snippet
/// installs. Pinned against that file by
/// `src/settings/hookEventParity.test.ts`.
pub const CLAUDE_CODE_HOOK_EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "PostToolUse",
    "PostToolUseFailure",
    "SubagentStart",
    "SubagentStop",
];

/// The hook events `AgentsSection.tsx`'s Codex setup snippet installs.
/// Pinned against that file by `src/settings/hookEventParity.test.ts`.
pub const CODEX_HOOK_EVENTS: [&str; 8] = [
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "PreToolUse",
    "PostToolUse",
];

/// The hook events `AgentsSection.tsx`'s Kimi setup snippet installs.
/// Pinned against that file by `src/settings/hookEventParity.test.ts`.
pub const KIMI_HOOK_EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "PostToolUse",
    "PostToolUseFailure",
    "SubagentStart",
    "SubagentStop",
];

/// What one runtime's Adapter installation looks like on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterInstall {
    /// The config file does not exist at the inspected path.
    ConfigMissing,
    /// The file exists but could not be read or parsed. `reason` is a
    /// bounded category (an `io::ErrorKind` debug name, or a fixed
    /// "malformed json"/"malformed toml" string) — NEVER a raw error
    /// string, which can embed the user's absolute home path.
    ConfigUnreadable { reason: String },
    /// Parsed. `wired` and `missing` are both in the canonical order of
    /// the corresponding `*_HOOK_EVENTS` const, never file order.
    Inspected {
        wired: Vec<String>,
        missing: Vec<String>,
        /// Every distinct command string found on a notchtap hook entry,
        /// first-seen order. Normally exactly one; more than one means a
        /// partially edited install, and each is reported separately.
        commands: Vec<String>,
    },
    /// OpenCode only: plugin-file presence, no hook list.
    PluginFile { present: bool },
}

/// What a hook's command string actually points at. The program is the
/// first whitespace-separated token of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTarget {
    /// Contains a `/`, and that path is an existing executable file.
    Resolved { path: PathBuf },
    /// Contains a `/`, but nothing executable is there — the failure
    /// mode that silently breaks every hook.
    Broken { path: PathBuf },
    /// A bare name (no `/`), which the provider must resolve via PATH.
    BareName {
        name: String,
        found_on_path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub runtime: &'static str,
    /// Already home-relative — produced by [`display_path`] before this
    /// struct is built, so [`render`] stays a pure formatter with no
    /// path logic and no `home` parameter.
    pub config_path_display: String,
    pub install: AdapterInstall,
    /// One entry per distinct command string in
    /// `AdapterInstall::Inspected.commands`, same order. Empty for every
    /// other `AdapterInstall` variant.
    pub command_targets: Vec<(String, CommandTarget)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub listener_ok: bool,
    pub port: u16,
    pub runtimes: Vec<RuntimeReport>,
    /// The Kimi hook-version line, pre-rendered by the caller from
    /// `kimi_version::probe_hook_support()`. `None` when not probed.
    pub kimi_note: Option<String>,
}

// --- inspection (pure: file contents in, report out) -------------------

/// Claude Code's `~/.claude/settings.json`.
pub fn inspect_claude_code(json: &str) -> AdapterInstall {
    inspect_hooks_json(json, &CLAUDE_CODE_HOOK_EVENTS, "claude-code")
}

/// Codex's `~/.codex/hooks.json` — same JSON shape as Claude Code's, so
/// this is the shared helper with a different const and token.
pub fn inspect_codex(json: &str) -> AdapterInstall {
    inspect_hooks_json(json, &CODEX_HOOK_EVENTS, "codex")
}

/// Kimi's `~/.kimi-code/config.toml` — an array of `[[hooks]]` tables
/// rather than a JSON object, so it gets its own extraction step but the
/// same [`assemble`] decision.
pub fn inspect_kimi(toml_text: &str) -> AdapterInstall {
    let Ok(table) = toml_text.parse::<toml::Table>() else {
        return AdapterInstall::ConfigUnreadable {
            reason: "malformed toml".to_string(),
        };
    };
    assemble(&toml_hook_pairs(&table), &KIMI_HOOK_EVENTS, "kimi")
}

/// OpenCode ships a plugin file, not hook entries, so presence is the
/// whole check. Taking a `bool` (not a path) keeps this pure — the
/// caller does the `is_file()`.
pub fn inspect_plugin_file(present: bool) -> AdapterInstall {
    AdapterInstall::PluginFile { present }
}

/// The shared Claude Code / Codex JSON body: parse, pull every
/// `(event, command)` pair out of the `hooks` object, decide.
fn inspect_hooks_json(json: &str, expected: &[&str], runtime_token: &str) -> AdapterInstall {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return AdapterInstall::ConfigUnreadable {
            reason: "malformed json".to_string(),
        };
    };
    assemble(&json_hook_pairs(&value), expected, runtime_token)
}

/// Every `(event, command)` pair in a `{"hooks": {"Event": [{"hooks":
/// [{"command": "..."}]}]}}` document. A document with no `hooks` key —
/// or any entry with a shape this doesn't recognise — yields no pairs
/// rather than an error: the file parsed fine, the wiring just isn't
/// there.
fn json_hook_pairs(value: &serde_json::Value) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let Some(hooks) = value.get("hooks").and_then(|h| h.as_object()) else {
        return pairs;
    };
    for (event, groups) in hooks {
        let Some(groups) = groups.as_array() else {
            continue;
        };
        for group in groups {
            let Some(entries) = group.get("hooks").and_then(|h| h.as_array()) else {
                continue;
            };
            for entry in entries {
                if let Some(command) = entry.get("command").and_then(|c| c.as_str()) {
                    pairs.push((event.clone(), command.to_string()));
                }
            }
        }
    }
    pairs
}

/// Every `(event, command)` pair in a TOML `[[hooks]]` array of tables. A
/// table missing either key is skipped, never a panic.
fn toml_hook_pairs(table: &toml::Table) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let Some(entries) = table.get("hooks").and_then(|h| h.as_array()) else {
        return pairs;
    };
    for entry in entries {
        let Some(entry) = entry.as_table() else {
            continue;
        };
        let (Some(event), Some(command)) = (
            entry.get("event").and_then(|v| v.as_str()),
            entry.get("command").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        pairs.push((event.to_string(), command.to_string()));
    }
    pairs
}

/// The wired/missing decision, shared by every config format.
///
/// A hook counts as wired when its command *contains* `hook <token>` —
/// a substring match, not equality, because users are expected to point
/// at an absolute path (`/Users/x/.local/bin/notchtap-agent hook kimi`).
/// Iteration is over `expected`, never the file, so `wired`/`missing`
/// come out in canonical order whatever order the user's file uses.
fn assemble(pairs: &[(String, String)], expected: &[&str], runtime_token: &str) -> AdapterInstall {
    let needle = format!("hook {runtime_token}");
    let mut wired = Vec::new();
    let mut missing = Vec::new();
    let mut commands: Vec<String> = Vec::new();

    for event in expected {
        let mut found = false;
        for (pair_event, command) in pairs {
            if pair_event == event && command.contains(&needle) {
                found = true;
                if !commands.iter().any(|c| c == command) {
                    commands.push(command.clone());
                }
            }
        }
        if found {
            wired.push((*event).to_string());
        } else {
            missing.push((*event).to_string());
        }
    }

    AdapterInstall::Inspected {
        wired,
        missing,
        commands,
    }
}

// --- command classification + path display (pure) ---------------------

/// Classifies a hook command's program. `path_dirs` is `$PATH` already
/// split by the caller, and `exists_executable` is injected so this is
/// unit-testable without touching the filesystem.
pub fn classify_command(
    command: &str,
    path_dirs: &[PathBuf],
    exists_executable: &dyn Fn(&Path) -> bool,
) -> CommandTarget {
    let Some(program) = command.split_whitespace().next() else {
        return CommandTarget::BareName {
            name: String::new(),
            found_on_path: None,
        };
    };

    if program.contains('/') {
        let path = PathBuf::from(program);
        if exists_executable(&path) {
            CommandTarget::Resolved { path }
        } else {
            CommandTarget::Broken { path }
        }
    } else {
        let found_on_path = path_dirs
            .iter()
            .map(|dir| dir.join(program))
            .find(|candidate| exists_executable(candidate));
        CommandTarget::BareName {
            name: program.to_string(),
            found_on_path,
        }
    }
}

/// Renders `path` with the user's home directory replaced by `~`, so no
/// absolute home path ever reaches the output. Falls back to the full
/// path when it is not under `home`.
pub fn display_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

// --- the impure helpers (no decision logic lives here) ----------------

/// The real-filesystem executable predicate: an existing *file* with any
/// unix execute bit set. `is_file()` first — a directory with the execute
/// bit is not a program.
pub fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(p) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// `$PATH` split into directories. Empty when `PATH` is unset.
pub fn path_dirs_from_env() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|raw| std::env::split_paths(&raw).collect())
        .unwrap_or_default()
}

/// `Ok(())` when something is listening on `127.0.0.1:port`, `Err(reason)`
/// otherwise. Impure (opens a socket), deliberately tiny, and prints
/// nothing so each caller can word its own output.
pub fn listener_reachable(port: u16) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    addr.parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())
        .and_then(|sock| {
            TcpStream::connect_timeout(&sock, Duration::from_millis(500)).map_err(|e| e.to_string())
        })
        .map(|_| ())
}

// --- the exit rule + the renderer (pure) ------------------------------

/// FAILURE only when the listener is unreachable, or when NOT ONE runtime
/// shows any evidence of installation. A runtime the user does not use
/// must never fail the command.
///
/// "Evidence of installation" means: `Inspected` with a non-empty `wired`
/// list (a partial install still counts — 8/10 is wired-but-incomplete,
/// not un-wired), or `PluginFile { present: true }`. `ConfigMissing`,
/// `ConfigUnreadable`, `PluginFile { present: false }`, and `Inspected`
/// with an empty `wired` list are all "no evidence".
pub fn is_healthy(report: &DoctorReport) -> bool {
    if !report.listener_ok {
        return false;
    }
    report
        .runtimes
        .iter()
        .any(|runtime| match &runtime.install {
            AdapterInstall::Inspected { wired, .. } => !wired.is_empty(),
            AdapterInstall::PluginFile { present } => *present,
            AdapterInstall::ConfigMissing | AdapterInstall::ConfigUnreadable { .. } => false,
        })
}

/// The whole human-readable report, as one string. Pure: no printing, no
/// clock, no filesystem, and no `home` parameter — config paths arrive
/// already home-relative via [`display_path`].
pub fn render(report: &DoctorReport) -> String {
    let mut out = String::from("notchtap doctor\n\n");
    out.push_str(&format!(
        "listener   127.0.0.1:{}   {}\n",
        report.port,
        if report.listener_ok {
            "reachable"
        } else {
            "not reachable"
        }
    ));

    for runtime in &report.runtimes {
        out.push('\n');
        out.push_str(&format!(
            "{}   {}\n",
            runtime.runtime, runtime.config_path_display
        ));
        match &runtime.install {
            AdapterInstall::ConfigMissing => {
                out.push_str("  config file not found — this runtime is not wired\n");
            }
            AdapterInstall::ConfigUnreadable { reason } => {
                out.push_str(&format!("  config file unreadable ({reason})\n"));
            }
            AdapterInstall::Inspected { wired, missing, .. } => {
                out.push_str(&format!(
                    "  {}/{} hooks wired\n",
                    wired.len(),
                    wired.len() + missing.len()
                ));
                if !missing.is_empty() {
                    out.push_str(&format!("  missing: {}\n", missing.join(", ")));
                }
            }
            AdapterInstall::PluginFile { present: true } => {
                out.push_str("  plugin file present\n");
            }
            AdapterInstall::PluginFile { present: false } => {
                out.push_str("  plugin file not found — this runtime is not wired\n");
            }
        }

        for (command, target) in &runtime.command_targets {
            let suffix = match target {
                CommandTarget::Resolved { .. } => "resolved".to_string(),
                CommandTarget::Broken { .. } => "NOT FOUND at that path".to_string(),
                CommandTarget::BareName {
                    found_on_path: Some(found),
                    ..
                } => format!("resolved via PATH ({})", found.display()),
                CommandTarget::BareName {
                    found_on_path: None,
                    ..
                } => "NOT FOUND on PATH".to_string(),
            };
            out.push_str(&format!("  command: {command} -> {suffix}\n"));
        }

        // The Kimi version gate belongs to the Kimi row visually (it is
        // the only runtime with one), even though the report carries it
        // once at the top level.
        if runtime.runtime == "kimi" {
            if let Some(note) = &report.kimi_note {
                out.push_str(&format!("  {note}\n"));
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_from_pairs(pairs: &[(&str, &str)]) -> String {
        let entries: Vec<String> = pairs
            .iter()
            .map(|(event, command)| {
                format!(
                    "\"{event}\": [{{ \"hooks\": [{{ \"type\": \"command\", \"command\": \"{command}\" }}] }}]"
                )
            })
            .collect();
        format!("{{ \"hooks\": {{ {} }} }}", entries.join(", "))
    }

    fn json_wiring(events: &[&str], command: &str) -> String {
        let pairs: Vec<(&str, &str)> = events.iter().map(|e| (*e, command)).collect();
        json_from_pairs(&pairs)
    }

    fn toml_wiring(events: &[&str], command: &str) -> String {
        events
            .iter()
            .map(|event| format!("[[hooks]]\nevent = \"{event}\"\ncommand = \"{command}\"\n"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn inspected(install: &AdapterInstall) -> (&[String], &[String], &[String]) {
        match install {
            AdapterInstall::Inspected {
                wired,
                missing,
                commands,
            } => (wired, missing, commands),
            other => panic!("expected Inspected, got {other:?}"),
        }
    }

    // --- inspection, JSON ---------------------------------------------

    #[test]
    fn claude_code_all_ten_events_wired() {
        let json = json_wiring(&CLAUDE_CODE_HOOK_EVENTS, "notchtap-agent hook claude-code");
        let install = inspect_claude_code(&json);
        let (wired, missing, commands) = inspected(&install);
        assert_eq!(wired.len(), 10);
        assert!(missing.is_empty());
        assert_eq!(commands, ["notchtap-agent hook claude-code".to_string()]);
    }

    #[test]
    fn claude_code_missing_events_reported_in_canonical_order() {
        // Deliberately scrambled file order, and missing two events that
        // are NOT adjacent: SessionEnd (canonical index 1) and
        // PostToolUse (index 6).
        let present = [
            "SubagentStop",
            "PostToolUseFailure",
            "Stop",
            "SessionStart",
            "SubagentStart",
            "Notification",
            "StopFailure",
            "PermissionRequest",
        ];
        let json = json_wiring(&present, "notchtap-agent hook claude-code");
        let install = inspect_claude_code(&json);
        let (wired, missing, _) = inspected(&install);
        assert_eq!(wired.len(), 8);
        assert_eq!(
            missing,
            ["SessionEnd".to_string(), "PostToolUse".to_string()]
        );
    }

    #[test]
    fn claude_code_hook_without_the_runtime_token_counts_missing() {
        let mut pairs: Vec<(&str, &str)> = CLAUDE_CODE_HOOK_EVENTS
            .iter()
            .map(|e| (*e, "notchtap-agent hook claude-code"))
            .collect();
        // "Stop" is wired to something else entirely.
        pairs[4] = ("Stop", "echo hello");
        let json = json_from_pairs(&pairs);
        let install = inspect_claude_code(&json);
        let (wired, missing, commands) = inspected(&install);
        assert_eq!(wired.len(), 9);
        assert_eq!(missing, ["Stop".to_string()]);
        assert_eq!(commands, ["notchtap-agent hook claude-code".to_string()]);
    }

    #[test]
    fn claude_code_absolute_path_command_is_still_wired() {
        let absolute = "/opt/notchtap/bin/notchtap-agent hook claude-code";
        let json = json_wiring(&CLAUDE_CODE_HOOK_EVENTS, absolute);
        let install = inspect_claude_code(&json);
        let (wired, missing, commands) = inspected(&install);
        assert_eq!(wired.len(), 10);
        assert!(missing.is_empty());
        assert_eq!(commands, [absolute.to_string()]);
    }

    #[test]
    fn claude_code_inspector_ignores_a_codex_wired_file() {
        let json = json_wiring(&CLAUDE_CODE_HOOK_EVENTS, "notchtap-agent hook codex");
        let install = inspect_claude_code(&json);
        let (wired, missing, commands) = inspected(&install);
        assert!(wired.is_empty());
        assert_eq!(missing.len(), 10);
        assert!(commands.is_empty());
    }

    #[test]
    fn malformed_json_is_config_unreadable() {
        let install = inspect_claude_code("{ \"hooks\": ");
        assert_eq!(
            install,
            AdapterInstall::ConfigUnreadable {
                reason: "malformed json".to_string()
            }
        );
    }

    #[test]
    fn valid_json_without_a_hooks_key_is_inspected_not_unreadable() {
        let install = inspect_claude_code("{ \"theme\": \"dark\" }");
        let (wired, missing, commands) = inspected(&install);
        assert!(wired.is_empty());
        assert_eq!(missing.len(), 10);
        assert!(commands.is_empty());
    }

    // --- inspection, Codex + TOML -------------------------------------

    #[test]
    fn codex_all_eight_events_wired() {
        let json = json_wiring(&CODEX_HOOK_EVENTS, "notchtap-agent hook codex");
        let install = inspect_codex(&json);
        let (wired, missing, _) = inspected(&install);
        assert_eq!(wired.len(), 8);
        assert!(missing.is_empty());
    }

    #[test]
    fn kimi_all_ten_hook_tables_wired() {
        let text = toml_wiring(&KIMI_HOOK_EVENTS, "notchtap-agent hook kimi");
        let install = inspect_kimi(&text);
        let (wired, missing, commands) = inspected(&install);
        assert_eq!(wired.len(), 10);
        assert!(missing.is_empty());
        assert_eq!(commands, ["notchtap-agent hook kimi".to_string()]);
    }

    #[test]
    fn kimi_table_without_a_command_key_is_missing_not_a_panic() {
        let text = format!(
            "[[hooks]]\nevent = \"SessionStart\"\n\n{}",
            toml_wiring(&KIMI_HOOK_EVENTS[1..], "notchtap-agent hook kimi")
        );
        let install = inspect_kimi(&text);
        let (wired, missing, _) = inspected(&install);
        assert_eq!(wired.len(), 9);
        assert_eq!(missing, ["SessionStart".to_string()]);
    }

    #[test]
    fn malformed_toml_is_config_unreadable() {
        let install = inspect_kimi("[[hooks]\nevent = \"SessionStart\"\n");
        assert_eq!(
            install,
            AdapterInstall::ConfigUnreadable {
                reason: "malformed toml".to_string()
            }
        );
    }

    #[test]
    fn kimi_config_with_unrelated_settings_still_parses() {
        let text = format!(
            "model = \"kimi-k2\"\n\n[ui]\ntheme = \"dark\"\n\n{}",
            toml_wiring(&KIMI_HOOK_EVENTS, "notchtap-agent hook kimi")
        );
        let install = inspect_kimi(&text);
        let (wired, missing, _) = inspected(&install);
        assert_eq!(wired.len(), 10);
        assert!(missing.is_empty());
    }

    // --- classify_command ---------------------------------------------

    #[test]
    fn classify_absolute_path_that_is_executable_resolves() {
        let target = classify_command("/opt/bin/notchtap-agent hook kimi", &[], &|p: &Path| {
            p == Path::new("/opt/bin/notchtap-agent")
        });
        assert_eq!(
            target,
            CommandTarget::Resolved {
                path: PathBuf::from("/opt/bin/notchtap-agent")
            }
        );
    }

    #[test]
    fn classify_absolute_path_that_does_not_exist_is_broken() {
        let target = classify_command("/opt/bin/notchtap-agent hook kimi", &[], &|_: &Path| false);
        assert_eq!(
            target,
            CommandTarget::Broken {
                path: PathBuf::from("/opt/bin/notchtap-agent")
            }
        );
    }

    #[test]
    fn classify_absolute_path_that_exists_but_is_not_executable_is_broken() {
        // `is_executable_file` requires the execute bit, not just
        // existence. Fixtures are this crate's own committed files (never
        // anything under the user's home directory): `Cargo.toml` is a
        // real mode-644 file, and the manifest directory has the execute
        // bit but is not a program.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml = manifest_dir.join("Cargo.toml");
        assert!(cargo_toml.is_file(), "fixture must exist");
        assert!(!is_executable_file(&cargo_toml));
        assert!(!is_executable_file(&manifest_dir));

        // ...and `classify_command` turns that "false" into Broken, not
        // Resolved: the verdict below is the real predicate's answer for
        // a file that exists with no execute bit.
        let target = classify_command("/opt/bin/notchtap-agent hook kimi", &[], &|_: &Path| {
            is_executable_file(&cargo_toml)
        });
        assert_eq!(
            target,
            CommandTarget::Broken {
                path: PathBuf::from("/opt/bin/notchtap-agent")
            }
        );
    }

    #[test]
    fn classify_bare_name_found_in_one_path_dir() {
        let dirs = [
            PathBuf::from("/usr/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ];
        let target = classify_command("notchtap-agent hook kimi", &dirs, &|p: &Path| {
            p == Path::new("/opt/homebrew/bin/notchtap-agent")
        });
        assert_eq!(
            target,
            CommandTarget::BareName {
                name: "notchtap-agent".to_string(),
                found_on_path: Some(PathBuf::from("/opt/homebrew/bin/notchtap-agent")),
            }
        );
    }

    #[test]
    fn classify_bare_name_found_in_no_path_dir() {
        let dirs = [PathBuf::from("/usr/bin")];
        let target = classify_command("notchtap-agent hook kimi", &dirs, &|_: &Path| false);
        assert_eq!(
            target,
            CommandTarget::BareName {
                name: "notchtap-agent".to_string(),
                found_on_path: None,
            }
        );
    }

    #[test]
    fn classify_empty_command_does_not_panic() {
        let target = classify_command("   ", &[PathBuf::from("/usr/bin")], &|_: &Path| true);
        assert_eq!(
            target,
            CommandTarget::BareName {
                name: String::new(),
                found_on_path: None,
            }
        );
    }

    // --- display_path + inspect_plugin_file ---------------------------

    #[test]
    fn display_path_shortens_home_and_passes_other_paths_through() {
        let home = Path::new("/Users/example");
        assert_eq!(
            display_path(Path::new("/Users/example/.claude/settings.json"), home),
            "~/.claude/settings.json"
        );
        assert_eq!(
            display_path(Path::new("/etc/notchtap/settings.json"), home),
            "/etc/notchtap/settings.json"
        );
    }

    #[test]
    fn inspect_plugin_file_maps_presence_to_the_two_variants() {
        assert_eq!(
            inspect_plugin_file(true),
            AdapterInstall::PluginFile { present: true }
        );
        assert_eq!(
            inspect_plugin_file(false),
            AdapterInstall::PluginFile { present: false }
        );
    }

    // --- render + is_healthy ------------------------------------------

    fn wired_claude_code_report() -> DoctorReport {
        DoctorReport {
            listener_ok: true,
            port: 9789,
            runtimes: vec![RuntimeReport {
                runtime: "claude-code",
                config_path_display: "~/.claude/settings.json".to_string(),
                install: AdapterInstall::Inspected {
                    wired: CLAUDE_CODE_HOOK_EVENTS
                        .iter()
                        .map(|e| (*e).to_string())
                        .collect(),
                    missing: Vec::new(),
                    commands: vec!["notchtap-agent hook claude-code".to_string()],
                },
                command_targets: vec![(
                    "notchtap-agent hook claude-code".to_string(),
                    CommandTarget::BareName {
                        name: "notchtap-agent".to_string(),
                        found_on_path: Some(PathBuf::from("/opt/homebrew/bin/notchtap-agent")),
                    },
                )],
            }],
            kimi_note: None,
        }
    }

    #[test]
    fn render_reports_the_wired_count_without_leaking_an_absolute_home_path() {
        let out = render(&wired_claude_code_report());
        assert!(out.contains("10/10 hooks wired"), "got:\n{out}");
        assert!(out.contains("~/"), "got:\n{out}");
        assert!(!out.contains("/Users/"), "got:\n{out}");
    }

    #[test]
    fn is_healthy_fails_only_on_a_dead_listener_or_zero_evidence() {
        // 1. Listener down, everything wired -> unhealthy.
        let mut down = wired_claude_code_report();
        down.listener_ok = false;
        assert!(!is_healthy(&down));

        // 2. Listener up, no runtime shows any evidence -> unhealthy.
        let nothing = DoctorReport {
            listener_ok: true,
            port: 9789,
            runtimes: vec![
                RuntimeReport {
                    runtime: "claude-code",
                    config_path_display: "~/.claude/settings.json".to_string(),
                    install: AdapterInstall::ConfigMissing,
                    command_targets: Vec::new(),
                },
                RuntimeReport {
                    runtime: "codex",
                    config_path_display: "~/.codex/hooks.json".to_string(),
                    install: AdapterInstall::ConfigUnreadable {
                        reason: "malformed json".to_string(),
                    },
                    command_targets: Vec::new(),
                },
                RuntimeReport {
                    runtime: "kimi",
                    config_path_display: "~/.kimi-code/config.toml".to_string(),
                    install: AdapterInstall::Inspected {
                        wired: Vec::new(),
                        missing: KIMI_HOOK_EVENTS.iter().map(|e| (*e).to_string()).collect(),
                        commands: Vec::new(),
                    },
                    command_targets: Vec::new(),
                },
                RuntimeReport {
                    runtime: "opencode",
                    config_path_display: "~/.config/opencode/plugins/notchtap.ts".to_string(),
                    install: AdapterInstall::PluginFile { present: false },
                    command_targets: Vec::new(),
                },
            ],
            kimi_note: None,
        };
        assert!(!is_healthy(&nothing));

        // 3. Listener up, one partial install (8/10) + three missing
        //    configs -> healthy. A partial install is still an install,
        //    and a runtime the user doesn't use must never fail this.
        let mut partial = nothing.clone();
        partial.runtimes[2].install = AdapterInstall::Inspected {
            wired: KIMI_HOOK_EVENTS[..8]
                .iter()
                .map(|e| (*e).to_string())
                .collect(),
            missing: KIMI_HOOK_EVENTS[8..]
                .iter()
                .map(|e| (*e).to_string())
                .collect(),
            commands: vec!["notchtap-agent hook kimi".to_string()],
        };
        partial.runtimes[1].install = AdapterInstall::ConfigMissing;
        assert!(is_healthy(&partial));

        // 4. Listener up, only the OpenCode plugin file present ->
        //    healthy.
        let mut plugin_only = nothing.clone();
        plugin_only.runtimes[1].install = AdapterInstall::ConfigMissing;
        plugin_only.runtimes[3].install = AdapterInstall::PluginFile { present: true };
        assert!(is_healthy(&plugin_only));
    }
}
