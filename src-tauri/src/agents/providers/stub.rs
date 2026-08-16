//! Fallback `hook` target for runtimes without a real parser: behaves
//! like a well-formed hook (drain stdin, never block the provider
//! session, never write to stdout, exit 0) rather than erroring.

use super::diagnostics;

/// Logs a bounded diagnostic naming which runtime's hook fired.
/// `stdin` is already fully drained by the caller
/// (`src/bin/notchtap_agent.rs`); accepted only so the call site reads
/// as "payload seen and discarded".
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
