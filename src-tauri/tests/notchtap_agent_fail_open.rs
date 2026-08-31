use std::io::Write;
use std::net::TcpListener;
use std::process::{Command, Stdio};

const VALID_SESSION_START: &str = r#"{
    "session_id": "sess-fail-open-test",
    "hook_event_name": "SessionStart",
    "cwd": "/tmp/notchtap-fail-open-test",
    "source": "startup"
}"#;

#[test]
fn hook_claude_code_exits_0_with_empty_stdout_when_the_port_is_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let bin = env!("CARGO_BIN_EXE_notchtap-agent");
    let mut child = Command::new(bin)
        .args(["hook", "claude-code"])
        .env("NOTCHTAP_PORT", port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn notchtap-agent");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(VALID_SESSION_START.as_bytes())
        .unwrap();

    let output = child
        .wait_with_output()
        .expect("failed to wait on notchtap-agent");

    assert!(
        output.status.success(),
        "hook mode must exit 0 even when delivery fails (fail-open), got {:?}",
        output.status
    );
    assert!(
        output.stdout.is_empty(),
        "hook mode must never write to stdout, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn hook_claude_code_exits_0_with_empty_stdout_on_malformed_stdin() {
    let bin = env!("CARGO_BIN_EXE_notchtap-agent");
    let mut child = Command::new(bin)
        .args(["hook", "claude-code"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn notchtap-agent");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"not json at all")
        .unwrap();

    let output = child
        .wait_with_output()
        .expect("failed to wait on notchtap-agent");

    assert!(
        output.status.success(),
        "malformed stdin must still fail open (exit 0)"
    );
    assert!(
        output.stdout.is_empty(),
        "hook mode must never write to stdout"
    );
}

#[test]
fn hook_codex_and_kimi_exit_0_with_empty_stdout_on_malformed_stdin() {
    let bin = env!("CARGO_BIN_EXE_notchtap-agent");
    for runtime in ["codex", "kimi"] {
        let mut child = Command::new(bin)
            .args(["hook", runtime])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn notchtap-agent hook {runtime}: {e}"));

        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"whatever\": true}")
            .unwrap();

        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{runtime} must fail open (exit 0) on malformed stdin"
        );
        assert!(
            output.stdout.is_empty(),
            "{runtime} must never write to stdout"
        );
    }
}
