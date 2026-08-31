use super::diagnostics;

/// Logs a bounded diagnostic naming which runtime's hook fired.
pub fn handle_stub(runtime_label: &str, stdin: &[u8]) {
    diagnostics::log_diagnostic(
        "hook stub",
        &format!(
            "{runtime_label} hook support not yet implemented — received {} byte payload, discarded",
            stdin.len()
        ),
    );
}
