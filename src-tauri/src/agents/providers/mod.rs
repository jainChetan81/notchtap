//! Everything the `notchtap-agent` binary needs, split so the binary
//! itself (`src/bin/notchtap_agent.rs`) stays a thin argv-dispatch
//! shell: pure per-provider stdin parsers ([`claude_code`], [`codex`],
//! [`kimi`]), the shared [`wire`] `NormalizedEvent` shape + schema-v1
//! body builder, the impure fail-open [`delivery`] POST, the bounded
//! [`diagnostics`] adapter log (a dedicated module because
//! `crate::logging` keeps `log_dir` private), the read-only [`doctor`]
//! setup inspection, the [`kimi_version`] hook-support gate, and the
//! [`stub`] fallback hook path for future runtimes.

pub mod claude_code;
pub mod codex;
pub mod delivery;
pub mod diagnostics;
pub mod doctor;
pub mod kimi;
pub mod kimi_version;
pub mod stub;
pub mod wire;
