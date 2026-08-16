//! `notchtap-agent` — the shared Agent Adapter hook-delivery binary.
//!
//! ```text
//! notchtap-agent hook claude-code|codex|kimi
//! notchtap-agent test <runtime>
//! notchtap-agent status
//! notchtap-agent doctor
//! ```
//!
//! Kept thin on purpose: argv dispatch plus impure "generate an event
//! id / read the clock / read stdin" glue; every actual decision lives
//! in `notchtap_lib::agents::providers`, where it's unit-tested. No
//! arg-parsing crate — a few fixed subcommands, hand-matched.
//!
//! `hook` mode NEVER writes to stdout (providers may interpret hook
//! stdout as structured decision output) and ALWAYS exits 0 (fail open
//! — a provider session must never be blocked by notchtap's absence or
//! a delivery failure). `test`/`status` are interactive diagnostic
//! subcommands, not hook targets, and use stdout normally.

use std::io::{self, ErrorKind, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use notchtap_lib::agents::model::AgentRuntime;
use notchtap_lib::agents::providers::{
    claude_code, codex, delivery, diagnostics, doctor, kimi, kimi_version, wire,
};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("hook") => run_hook(it.next().map(String::as_str)).await,
        Some("test") => run_test(it.next().map(String::as_str)).await,
        Some("status") => run_status(),
        Some("doctor") => run_doctor(),
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage: notchtap-agent hook <claude-code|codex|kimi>\n       notchtap-agent test <runtime>\n       notchtap-agent status\n       notchtap-agent doctor"
    );
}

fn occurred_at_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `hook` reads exactly one native JSON payload from stdin, normalizes
/// and delivers it, and always exits 0. Stdin is fully drained up front
/// regardless of runtime/outcome, so a provider waiting on this
/// process's stdin pipe to close is never left hanging by an early
/// return.
async fn run_hook(runtime: Option<&str>) -> ExitCode {
    let mut buf = Vec::new();
    if let Err(e) = io::stdin().read_to_end(&mut buf) {
        diagnostics::log_diagnostic("hook", &format!("failed reading stdin: {e}"));
        return ExitCode::SUCCESS;
    }

    match runtime {
        Some("claude-code") => deliver_claude_code(&buf).await,
        Some("codex") => deliver_codex(&buf).await,
        Some("kimi") => deliver_kimi(&buf).await,
        Some(other) => {
            diagnostics::log_diagnostic("hook", &format!("unknown runtime: {other}"));
            ExitCode::SUCCESS
        }
        None => {
            diagnostics::log_diagnostic("hook", "missing runtime argument");
            ExitCode::SUCCESS
        }
    }
}

/// Shared body of `deliver_claude_code`/`deliver_codex`/`deliver_kimi`:
/// parse via the runtime's pure `normalize` fn, build the wire body,
/// resolve the port, deliver, log any failure. `runtime_label` is both
/// the diagnostic-log prefix and the wire `runtime` token. Kimi's
/// version gate runs BEFORE this helper (in `deliver_kimi`), not inside
/// it — the one per-runtime behavior this dedup deliberately keeps out.
async fn deliver_via<E: std::fmt::Display>(
    runtime_label: &str,
    stdin: &[u8],
    normalize: impl Fn(&[u8]) -> Result<wire::NormalizedEvent, E>,
) -> ExitCode {
    let normalized = match normalize(stdin) {
        Ok(n) => n,
        Err(e) => {
            diagnostics::log_diagnostic(&format!("{runtime_label} parse"), &e.to_string());
            return ExitCode::SUCCESS;
        }
    };

    let event_id = uuid::Uuid::new_v4().to_string();
    let body = wire::build_wire_body(
        runtime_label,
        &normalized,
        &event_id,
        occurred_at_ms(),
        None,
    );
    let port = delivery::resolve_port();

    if let delivery::DeliveryOutcome::Failed(reason) = delivery::deliver(body, port).await {
        diagnostics::log_diagnostic(&format!("{runtime_label} deliver"), &reason);
    }

    // Always 0 — delivery outcome never changes this process's exit
    // status.
    ExitCode::SUCCESS
}

async fn deliver_claude_code(stdin: &[u8]) -> ExitCode {
    deliver_via("claude-code", stdin, claude_code::normalize).await
}

/// Codex hook path — no version gate (Codex's hook surface has no
/// documented version-gating story the way Kimi's does), otherwise
/// identical shape to [`deliver_claude_code`].
async fn deliver_codex(stdin: &[u8]) -> ExitCode {
    deliver_via("codex", stdin, codex::normalize).await
}

/// Kimi hook path. Unlike Codex/Claude Code, refuses to deliver below
/// [`kimi_version::MINIMUM_HOOK_VERSION`] — still fail-open (always
/// exits 0, never blocks the provider), but the event is dropped with a
/// bounded diagnostic rather than posted; never "deliver anyway and let
/// the endpoint sort it out". The gate stays a Kimi-specific pre-check
/// ahead of [`deliver_via`].
async fn deliver_kimi(stdin: &[u8]) -> ExitCode {
    match kimi_version::probe_hook_support() {
        kimi_version::HookSupport::Unavailable { detected, minimum } => {
            diagnostics::log_diagnostic(
                "kimi hook",
                &format!(
                    "refused: kimi hook support requires >= {minimum}, detected {}",
                    detected.as_deref().unwrap_or("(kimi not found on PATH)")
                ),
            );
            return ExitCode::SUCCESS;
        }
        kimi_version::HookSupport::Supported { .. } => {}
    }

    deliver_via("kimi", stdin, kimi::normalize).await
}

const KNOWN_RUNTIMES: [&str; 4] = ["claude-code", "codex", "kimi", "opencode"];

/// `notchtap-agent test <runtime>` posts a synthetic, terminal
/// `completed` schema-v1 event so a user can verify their wiring
/// produces an actual VISIBLE card, not just a silent registry update.
/// Must stay `completed`+`terminal:true`: `informational` is suppressed
/// by default (the user would see nothing), and a `terminal:false`
/// event never sets `terminal_at`, so the registry's terminal sweep
/// (`agents::registry::AgentRegistry::tick`) would leave the test
/// session on the Agent Board forever, suppressing the idle face.
/// Unlike `hook`, this is interactive: it prints its outcome and
/// returns non-zero on failure.
async fn run_test(runtime: Option<&str>) -> ExitCode {
    let Some(runtime) = runtime else {
        eprintln!("usage: notchtap-agent test <claude-code|codex|kimi|opencode>");
        return ExitCode::from(2);
    };
    if !KNOWN_RUNTIMES.contains(&runtime) {
        eprintln!(
            "unknown runtime {runtime:?} — expected one of {}",
            KNOWN_RUNTIMES.join(", ")
        );
        return ExitCode::from(2);
    }

    let event_id = uuid::Uuid::new_v4().to_string();
    let session_id = format!("notchtap-agent-test-{event_id}");
    let body = serde_json::json!({
        "schemaVersion": 1,
        "eventId": event_id,
        "runtime": runtime,
        "sessionId": session_id,
        "occurredAtMs": occurred_at_ms(),
        "nativeEvent": "notchtap-agent test",
        "kind": "completed",
        "state": "completed",
        "summary": format!("Test event from `notchtap-agent test {runtime}` — turn completed"),
        "capabilities": ["session_lifecycle", "completion"],
        "terminal": true,
    });

    let port = delivery::resolve_port();
    match delivery::deliver(body, port).await {
        delivery::DeliveryOutcome::Delivered => {
            println!("delivered a test event for {runtime} to 127.0.0.1:{port}");
            ExitCode::SUCCESS
        }
        delivery::DeliveryOutcome::Failed(reason) => {
            eprintln!("failed to deliver test event: {reason}");
            ExitCode::FAILURE
        }
    }
}

/// `notchtap-agent status` — a quick "is anything listening on the
/// configured loopback port" check. A bare TCP connect, not an HTTP
/// request: `http.rs` exposes no health endpoint and a raw connect
/// answers "port in use" without inventing one.
fn run_status() -> ExitCode {
    let port = delivery::resolve_port();
    let listener_ok = match doctor::listener_reachable(port) {
        Ok(()) => {
            println!("notchtap: listening on 127.0.0.1:{port}");
            true
        }
        Err(e) => {
            println!("notchtap: not reachable on 127.0.0.1:{port} ({e})");
            false
        }
    };

    // CLI stand-in for the Settings compatibility surface.
    match kimi_version::probe_hook_support() {
        kimi_version::HookSupport::Supported { detected } => {
            println!(
                "kimi: available (detected {detected}, hooks require >= {})",
                kimi_version::MINIMUM_HOOK_VERSION_STR
            );
        }
        kimi_version::HookSupport::Unavailable { detected, minimum } => {
            println!(
                "kimi: unavailable (detected {}, hooks require >= {minimum})",
                detected.as_deref().unwrap_or("no kimi on PATH")
            );
        }
    }

    if listener_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `notchtap-agent doctor` — the read-only "is my Agent Adapter
/// actually wired?" report `status` can't give: Adapter Health only
/// reports what has been *received*, so un-wired and wired-but-idle
/// look identical there. Thin shell: reads the config files and asks
/// `providers::doctor` what they mean; every decision lives in that
/// module, where it's unit-testable.
///
/// **It never writes.** Repairing or installing a runtime's hooks is
/// not this command's job — notchtap never silently edits a user's
/// global provider configuration.
fn run_doctor() -> ExitCode {
    let Some(home) = dirs::home_dir() else {
        println!("notchtap doctor: cannot resolve a home directory");
        return ExitCode::FAILURE;
    };

    let port = delivery::resolve_port();
    let listener_error = doctor::listener_reachable(port).err();
    let listener_ok = listener_error.is_none();
    let path_dirs = doctor::path_dirs_from_env();

    type Inspector = fn(&str) -> doctor::AdapterInstall;
    const HOOK_CONFIGS: [(AgentRuntime, &str, Inspector); 3] = [
        (
            AgentRuntime::ClaudeCode,
            ".claude/settings.json",
            doctor::inspect_claude_code,
        ),
        (
            AgentRuntime::Codex,
            ".codex/hooks.json",
            doctor::inspect_codex,
        ),
        (
            AgentRuntime::Kimi,
            ".kimi-code/config.toml",
            doctor::inspect_kimi,
        ),
    ];

    let mut runtimes = Vec::with_capacity(HOOK_CONFIGS.len() + 1);
    let mut kimi_inspected = false;

    for (runtime, relative, inspect) in HOOK_CONFIGS {
        let path = home.join(relative);
        let install = match std::fs::read_to_string(&path) {
            Ok(contents) => inspect(&contents),
            Err(e) if e.kind() == ErrorKind::NotFound => doctor::AdapterInstall::ConfigMissing,
            // The `ErrorKind` debug name, never `e.to_string()` — the
            // latter can echo the user's absolute home path.
            Err(e) => doctor::AdapterInstall::ConfigUnreadable {
                reason: format!("{:?}", e.kind()),
            },
        };
        if runtime == AgentRuntime::Kimi
            && matches!(install, doctor::AdapterInstall::Inspected { .. })
        {
            kimi_inspected = true;
        }
        runtimes.push(doctor::RuntimeReport {
            runtime,
            config_path_display: doctor::display_path(&path, &home),
            command_targets: command_targets_for(&install, &path_dirs),
            install,
        });
    }

    // OpenCode ships a plugin file rather than hook entries, so
    // presence is the whole check; the path is rendered in the output.
    let opencode_path = home.join(".config/opencode/plugins/notchtap.ts");
    runtimes.push(doctor::RuntimeReport {
        runtime: AgentRuntime::OpenCode,
        config_path_display: doctor::display_path(&opencode_path, &home),
        install: doctor::inspect_plugin_file(opencode_path.is_file()),
        command_targets: Vec::new(),
    });

    // Only probe the version gate when Kimi is actually wired — no
    // subprocess or confusing line for users without Kimi.
    let kimi_note = kimi_inspected.then(|| match kimi_version::probe_hook_support() {
        kimi_version::HookSupport::Supported { detected } => format!(
            "kimi {detected} detected (hooks require >= {}) — supported",
            kimi_version::MINIMUM_HOOK_VERSION_STR
        ),
        // Split on `detected`: folding `None` into the same sentence
        // reads as "kimi no kimi on PATH detected".
        kimi_version::HookSupport::Unavailable {
            detected: Some(detected),
            minimum,
        } => format!("kimi {detected} detected (hooks require >= {minimum}) — unavailable"),
        kimi_version::HookSupport::Unavailable {
            detected: None,
            minimum,
        } => format!("kimi not found on PATH (hooks require >= {minimum}) — unavailable"),
    });

    let report = doctor::DoctorReport {
        listener_ok,
        listener_error,
        port,
        runtimes,
        kimi_note,
    };
    println!("{}", doctor::render(&report));

    if doctor::setup_ok(&report) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Maps every distinct command string an install reported through
/// [`doctor::classify_command`], against the real filesystem. Pure glue:
/// the classification itself is the library's decision.
fn command_targets_for(
    install: &doctor::AdapterInstall,
    path_dirs: &[PathBuf],
) -> Vec<(String, doctor::CommandTarget)> {
    let doctor::AdapterInstall::Inspected { commands, .. } = install else {
        return Vec::new();
    };
    commands
        .iter()
        .map(|command| {
            (
                command.clone(),
                doctor::classify_command(command, path_dirs, &doctor::is_executable_file),
            )
        })
        .collect()
}
