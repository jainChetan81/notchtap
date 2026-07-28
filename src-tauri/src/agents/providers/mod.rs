//! Plan 138 (v7 ticket 6 of 13, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
//! §4.1/§4.2): everything the `notchtap-agent` binary needs, split so the
//! binary itself (`src/bin/notchtap_agent.rs`) stays a thin argv-dispatch
//! shell:
//!
//! - [`claude_code`] — the real, pure Claude Code stdin-payload parser
//!   (§4.2), plus its committed redacted fixture tests.
//! - [`wire`] — the shared `NormalizedEvent` intermediate shape and the
//!   schema-v1 (§3.1) JSON body builder every provider parser feeds.
//! - [`delivery`] — the impure POST-and-fail-open half (§4.1): ≤750ms
//!   connect+read timeout, `NOTCHTAP_PORT` resolution, never surfaces a
//!   delivery failure as a process exit code.
//! - [`diagnostics`] — the bounded adapter log file (§4.1: "a bounded
//!   diagnostic to notchtap's adapter log, never stdout"). A dedicated
//!   file/module rather than reusing `crate::logging`, because that
//!   module is a private `mod` with a private `log_dir` — this ticket's
//!   surface is additive-only (see this crate's `agents/mod.rs` doc).
//! - [`codex`] — the real, pure Codex stdin-payload parser (ticket 139,
//!   §4.3), plus its committed redacted fixture tests.
//! - [`doctor`] — the read-only Adapter setup inspection behind
//!   `notchtap-agent doctor`: parses each runtime's hook config file and
//!   reports what is wired. Never writes.
//! - [`kimi`] — the real, pure Kimi Code stdin-payload parser (ticket
//!   140, §4.4), plus its committed redacted fixture tests.
//! - [`kimi_version`] — ticket 140's Kimi hook-support version gate: a
//!   pure decision function plus an isolated `kimi --version` probe.
//! - [`stub`] — the "not yet supported" stub path, now unused by
//!   Codex/Kimi (both real as of tickets 139/140) but kept for any
//!   future runtime's hook command to land on before its own parser
//!   ships.

pub mod claude_code;
pub mod codex;
pub mod delivery;
pub mod diagnostics;
pub mod doctor;
pub mod kimi;
pub mod kimi_version;
pub mod stub;
pub mod wire;
