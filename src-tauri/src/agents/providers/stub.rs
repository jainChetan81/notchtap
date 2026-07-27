//! Plan 138: the Codex/Kimi `hook` stub path. Their real per-provider
//! parsers are tickets 139/140 (spec §4.3/§4.4) — until then,
//! `notchtap-agent hook codex` / `notchtap-agent hook kimi` must still
//! behave like a well-formed hook target (drain stdin, never block the
//! provider session, never write to stdout, exit 0) rather than error
//! out or do nothing silently.

use super::diagnostics;

/// `runtime_label` is the exact CLI token (`"codex"`/`"kimi"`) so the
/// bounded diagnostic names which runtime's hook fired. `stdin` has
/// already been fully drained by the caller (`src/bin/notchtap_agent.rs`)
/// before this runs — accepted here only so the call site reads as
/// "the stub saw the payload and discarded it", not "the stub never
/// looked".
pub fn handle_stub(runtime_label: &str, stdin: &[u8]) {
    diagnostics::log_diagnostic(
        "hook stub",
        &format!(
            "{runtime_label} hook support not yet implemented — received {} byte payload, discarded",
            stdin.len()
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_does_not_panic_on_empty_or_garbage_stdin() {
        handle_stub("codex", b"");
        handle_stub("kimi", b"not json at all");
    }
}
