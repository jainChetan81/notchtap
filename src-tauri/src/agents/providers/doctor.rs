use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::agents::adapter::runtime_wire_label;
use crate::agents::model::AgentRuntime;

/// The hook events `AgentsSection.tsx`'s Claude Code setup snippet installs.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterInstall {
    ConfigMissing,
    /// `reason` is a bounded category (an `io::ErrorKind` debug name, or a fixed "malformed
    /// json"/"malformed toml" string) — NEVER a raw error string.
    ConfigUnreadable { reason: String },
    /// `wired` and `missing` are both in the canonical order of the corresponding `*_HOOK_EVENTS`
    /// const, never file order.
    Inspected {
        wired: Vec<String>,
        missing: Vec<String>,
        commands: Vec<String>,
    },
    PluginFile { present: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTarget {
    Resolved { path: PathBuf },
    Broken { path: PathBuf },
    /// A bare name (no `/`), which the provider must resolve via PATH.
    BareName {
        name: String,
        found_on_path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub runtime: AgentRuntime,
    pub config_path_display: String,
    pub install: AdapterInstall,
    pub command_targets: Vec<(String, CommandTarget)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub listener_ok: bool,
    /// `None` whenever `listener_ok` is true.
    pub listener_error: Option<String>,
    pub port: u16,
    pub runtimes: Vec<RuntimeReport>,
    pub kimi_note: Option<String>,
}

pub fn inspect_claude_code(json: &str) -> AdapterInstall {
    inspect_hooks_json(json, &CLAUDE_CODE_HOOK_EVENTS, AgentRuntime::ClaudeCode)
}

/// Codex's `~/.codex/hooks.json` — same JSON shape as Claude Code's, so this is the shared helper
/// with a different const and token.
pub fn inspect_codex(json: &str) -> AdapterInstall {
    inspect_hooks_json(json, &CODEX_HOOK_EVENTS, AgentRuntime::Codex)
}

/// Kimi's `~/.kimi-code/config.toml` — an array of `[[hooks]]` tables rather than a JSON object, so
/// it gets its own extraction step but the same [`assemble`] decision.
pub fn inspect_kimi(toml_text: &str) -> AdapterInstall {
    let Ok(table) = toml_text.parse::<toml::Table>() else {
        return AdapterInstall::ConfigUnreadable {
            reason: "malformed toml".to_string(),
        };
    };
    assemble(
        &toml_hook_pairs(&table),
        &KIMI_HOOK_EVENTS,
        AgentRuntime::Kimi,
    )
}

/// OpenCode ships a plugin file, not hook entries, so presence is the whole check.
pub fn inspect_plugin_file(present: bool) -> AdapterInstall {
    AdapterInstall::PluginFile { present }
}

fn inspect_hooks_json(json: &str, expected: &[&str], runtime: AgentRuntime) -> AdapterInstall {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return AdapterInstall::ConfigUnreadable {
            reason: "malformed json".to_string(),
        };
    };
    assemble(&json_hook_pairs(&value), expected, runtime)
}

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

/// A table missing either key is skipped, never a panic.
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

/// Iteration is over `expected`, never the file, so `wired`/`missing` come out in canonical order
/// whatever order the user's file uses.
fn assemble(
    pairs: &[(String, String)],
    expected: &[&str],
    runtime: AgentRuntime,
) -> AdapterInstall {
    let needle = format!("hook {}", runtime_wire_label(runtime));
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

/// Renders `path` with the user's home directory replaced by `~`, so no absolute home path ever
/// reaches the output.
pub fn display_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

pub fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(p) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// `$PATH` split into directories.
pub fn path_dirs_from_env() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|raw| std::env::split_paths(&raw).collect())
        .unwrap_or_default()
}

/// `Ok(())` when something is listening on `127.0.0.1:port`, `Err(reason)` otherwise.
pub fn listener_reachable(port: u16) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    addr.parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())
        .and_then(|sock| {
            TcpStream::connect_timeout(&sock, Duration::from_millis(500)).map_err(|e| e.to_string())
        })
        .map(|_| ())
}

// A runtime the user does not use must never fail the command.
pub fn setup_ok(report: &DoctorReport) -> bool {
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

/// The whole human-readable report, as one string.
pub fn render(report: &DoctorReport) -> String {
    let mut out = String::from("notchtap doctor\n\n");
    out.push_str(&format!(
        "listener   127.0.0.1:{}   {}\n",
        report.port,
        match (report.listener_ok, &report.listener_error) {
            (true, _) => "reachable".to_string(),
            (false, Some(reason)) => format!("not reachable ({reason})"),
            (false, None) => "not reachable".to_string(),
        }
    ));

    for runtime in &report.runtimes {
        out.push('\n');
        out.push_str(&format!(
            "{}   {}\n",
            runtime_wire_label(runtime.runtime),
            runtime.config_path_display
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

        if runtime.runtime == AgentRuntime::Kimi {
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
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml = manifest_dir.join("Cargo.toml");
        assert!(cargo_toml.is_file(), "fixture must exist");
        assert!(!is_executable_file(&cargo_toml));
        assert!(!is_executable_file(&manifest_dir));

        let command = cargo_toml.to_str().expect("fixture path is utf-8");
        assert!(
            !command.contains(char::is_whitespace),
            "fixture path must not contain whitespace — `classify_command` \
             splits the program off on whitespace, so a spaced checkout \
             directory would make this assertion test the wrong thing"
        );
        let target = classify_command(command, &[], &is_executable_file);
        assert_eq!(
            target,
            CommandTarget::Broken {
                path: cargo_toml.clone()
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



    fn wired_claude_code_report() -> DoctorReport {
        DoctorReport {
            listener_ok: true,
            listener_error: None,
            port: 9789,
            runtimes: vec![RuntimeReport {
                runtime: AgentRuntime::ClaudeCode,
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
    fn render_carries_the_listener_failure_reason_when_there_is_one() {
        let mut down = wired_claude_code_report();
        down.listener_ok = false;
        down.listener_error = Some("Connection refused (os error 61)".to_string());
        let out = render(&down);
        assert!(
            out.contains("not reachable (Connection refused (os error 61))"),
            "got:\n{out}"
        );

        down.listener_error = None;
        let bare = render(&down);
        assert!(bare.contains("not reachable\n"), "got:\n{bare}");
        assert!(!bare.contains("not reachable ("), "got:\n{bare}");
    }

    #[test]
    fn setup_ok_fails_only_on_a_dead_listener_or_zero_evidence() {
        let mut down = wired_claude_code_report();
        down.listener_ok = false;
        assert!(!setup_ok(&down));

        let nothing = DoctorReport {
            listener_ok: true,
            listener_error: None,
            port: 9789,
            runtimes: vec![
                RuntimeReport {
                    runtime: AgentRuntime::ClaudeCode,
                    config_path_display: "~/.claude/settings.json".to_string(),
                    install: AdapterInstall::ConfigMissing,
                    command_targets: Vec::new(),
                },
                RuntimeReport {
                    runtime: AgentRuntime::Codex,
                    config_path_display: "~/.codex/hooks.json".to_string(),
                    install: AdapterInstall::ConfigUnreadable {
                        reason: "malformed json".to_string(),
                    },
                    command_targets: Vec::new(),
                },
                RuntimeReport {
                    runtime: AgentRuntime::Kimi,
                    config_path_display: "~/.kimi-code/config.toml".to_string(),
                    install: AdapterInstall::Inspected {
                        wired: Vec::new(),
                        missing: KIMI_HOOK_EVENTS.iter().map(|e| (*e).to_string()).collect(),
                        commands: Vec::new(),
                    },
                    command_targets: Vec::new(),
                },
                RuntimeReport {
                    runtime: AgentRuntime::OpenCode,
                    config_path_display: "~/.config/opencode/plugins/notchtap.ts".to_string(),
                    install: AdapterInstall::PluginFile { present: false },
                    command_targets: Vec::new(),
                },
            ],
            kimi_note: None,
        };
        assert!(!setup_ok(&nothing));

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
        assert!(setup_ok(&partial));

        let mut plugin_only = nothing.clone();
        plugin_only.runtimes[1].install = AdapterInstall::ConfigMissing;
        plugin_only.runtimes[3].install = AdapterInstall::PluginFile { present: true };
        assert!(setup_ok(&plugin_only));
    }
}
