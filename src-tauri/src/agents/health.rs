//! Plan 143 (v7 ticket 11 of 13, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
//! §4.6/§8/§10): per-runtime Adapter Health.
//!
//! Two halves, same split this crate already uses elsewhere
//! (`kimi_version.rs`'s own doc: "keep the pure decision logic...
//! separate from that subprocess call"):
//!
//! - pure derivation ([`declared_capabilities`], [`availability_for`],
//!   [`compatibility_message`], [`build_adapter_health`]) — unit-tested
//!   directly, no clock/subprocess/lock involved;
//! - [`HealthTracker`], the impure, shared bookkeeping [`http.rs`]'s
//!   `/agent/events` handler updates on every accepted/rejected event
//!   (last-accepted-event time, last bounded error category) and that
//!   caches the one genuinely impure input this module needs — Kimi's
//!   `kimi --version` hook-support probe (`providers::kimi_version`) —
//!   so a live health read (the `agent-state` publish path, and the
//!   Settings `get_agent_health` command) never shells out more than
//!   once per [`KIMI_PROBE_CACHE_TTL`].
//!
//! Spec §10's five Adapter Health fields land as [`AdapterHealth`]'s five
//! non-runtime fields: availability, declared capabilities, last
//! accepted event time, last bounded error category, and a setup
//! compatibility message — never a raw provider version string or a raw
//! error message (CLAUDE.md/spec §3.2: bounded categories, not free text,
//! for anything derived from untrusted wire input).

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use super::model::{AgentCapability, AgentRuntime};
use super::providers::kimi_version::{self, HookSupport};
use crate::config::AgentRuntimesConfig;

/// How long a cached `kimi --version` probe is trusted before
/// [`HealthTracker::kimi_hook_support`] shells out again. Kimi's
/// installed version does not change while notchtap is running in any
/// way this app can observe, so a coarse cache (rather than probing on
/// every 5s board tick, `board::DEFAULT_TICK_INTERVAL`) is enough —
/// see that constant's own doc for why the tick itself stays cheap.
pub const KIMI_PROBE_CACHE_TTL: Duration = Duration::from_secs(60);

/// spec §10's three-state Adapter Health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterAvailability {
    /// Enabled, and (for Kimi) hook-version-gated support confirmed.
    Available,
    /// Enabled and usable, but this runtime's own declared capability
    /// set has known gaps against the full reference set (spec §1's
    /// matrix) — Codex (no `input_required`/`failure` hook) and
    /// OpenCode (no `subagents`) both land here truthfully rather than
    /// being reported as fully `Available`.
    Partial,
    /// Administratively disabled (`[agents.runtimes.*]` toggle off) or,
    /// for Kimi specifically, the local install's hook surface is below
    /// [`kimi_version::MINIMUM_HOOK_VERSION`].
    Unavailable,
}

impl AdapterAvailability {
    pub fn label(self) -> &'static str {
        match self {
            AdapterAvailability::Available => "available",
            AdapterAvailability::Partial => "partial",
            AdapterAvailability::Unavailable => "unavailable",
        }
    }
}

/// A bounded, matchable category for a rejected `/agent/events` POST —
/// never the raw `AdapterError` display string, which could echo
/// untrusted wire content back into a health readout (CLAUDE.md: library
/// modules use `thiserror` variants a test/caller can match on, not
/// stringly-typed errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterErrorCategory {
    /// Malformed JSON, an unsupported `schemaVersion`, a missing
    /// required identity field, or an unrecognized enum value
    /// (`AdapterError::MalformedJson`/`UnsupportedSchemaVersion`/
    /// `MissingIdentity`/`MalformedEnum`).
    MalformedPayload,
    /// `AdapterError::UnsupportedRuntime` — the wire `runtime` string
    /// itself didn't name one of the four known runtimes, so this
    /// category is never attributable to a specific [`AgentRuntime`]
    /// card (see [`super::super::http`]'s handler: it's recorded only
    /// via the best-effort hint below, which itself only ever resolves
    /// to a *known* runtime).
    UnsupportedRuntime,
    /// Reserved for a future internal (non-wire) failure category —
    /// nothing in the current `/agent/events` handler produces one
    /// (parse failures are always `400`, an accepted event is always
    /// `202`), but spec §10 lists "last bounded error category" as an
    /// open-ended bounded set, not just the two wire-parse categories
    /// above, so this variant exists now rather than being a breaking
    /// addition later.
    #[allow(dead_code)]
    Internal,
}

impl AdapterErrorCategory {
    pub fn label(self) -> &'static str {
        match self {
            AdapterErrorCategory::MalformedPayload => "malformed_payload",
            AdapterErrorCategory::UnsupportedRuntime => "unsupported_runtime",
            AdapterErrorCategory::Internal => "internal",
        }
    }

    /// Maps a wire-parse [`super::adapter::AdapterError`] onto a bounded
    /// category. Every variant of that error type is exhaustively
    /// covered — a new `AdapterError` variant will fail to compile here
    /// until this match is updated too.
    pub fn from_adapter_error(error: &super::adapter::AdapterError) -> Self {
        use super::adapter::AdapterError;
        match error {
            AdapterError::MalformedJson(_)
            | AdapterError::UnsupportedSchemaVersion(_)
            | AdapterError::MissingIdentity(_)
            | AdapterError::MalformedEnum { .. } => AdapterErrorCategory::MalformedPayload,
            AdapterError::UnsupportedRuntime(_) => AdapterErrorCategory::UnsupportedRuntime,
        }
    }
}

/// One runtime's full Adapter Health snapshot (spec §10, §8's Settings
/// adapter cards).
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterHealth {
    pub runtime: AgentRuntime,
    /// Mirrors `[agents.runtimes.*]`'s own toggle directly — surfaced
    /// alongside `availability` because "administratively disabled" and
    /// "Kimi hook version too old" are both real reasons for
    /// `Unavailable`, and the Settings card needs to tell them apart
    /// (spec §4.6: "detected/undetected status").
    pub enabled: bool,
    pub availability: AdapterAvailability,
    pub capabilities: Vec<AgentCapability>,
    pub last_accepted_event_ms: Option<i64>,
    pub last_error_category: Option<AdapterErrorCategory>,
    pub compatibility_message: Option<String>,
}

/// Declaration order used everywhere a full four-runtime health snapshot
/// is built — matches [`AgentRuntime`]'s own declaration order (spec §0:
/// "Claude Code, Codex, Kimi, and OpenCode").
pub const ALL_RUNTIMES: [AgentRuntime; 4] = [
    AgentRuntime::ClaudeCode,
    AgentRuntime::Codex,
    AgentRuntime::Kimi,
    AgentRuntime::OpenCode,
];

/// Spec §1's per-runtime capability row, restricted (like every provider
/// parser's own `CAPABILITIES` const) to what that adapter actually
/// declares on the wire — `open_or_focus` is Host-dependent and never
/// part of a runtime's own declared set (same exclusion
/// `claude_code::CAPABILITIES`'s doc gives). Kept as its own small table
/// here (rather than importing each provider module's string-typed
/// `CAPABILITIES` const) because Health needs the typed
/// [`AgentCapability`] enum, not the wire-label strings those consts
/// carry — see this module's top doc for why a health snapshot never
/// serializes a raw provider string.
pub fn declared_capabilities(runtime: AgentRuntime) -> &'static [AgentCapability] {
    use AgentCapability::*;
    match runtime {
        // providers/claude_code.rs::CAPABILITIES
        AgentRuntime::ClaudeCode => &[
            SessionLifecycle,
            PermissionRequests,
            InputRequired,
            Completion,
            Failure,
            ToolDetails,
            Subagents,
        ],
        // providers/codex.rs::CAPABILITIES — no input_required/failure
        // (declared gap, that module's own top doc)
        AgentRuntime::Codex => &[
            SessionLifecycle,
            PermissionRequests,
            Completion,
            ToolDetails,
            Subagents,
        ],
        // providers/kimi.rs::CAPABILITIES — full set, gated by hook
        // version support (availability_for below), not by capability
        // completeness.
        AgentRuntime::Kimi => &[
            SessionLifecycle,
            PermissionRequests,
            InputRequired,
            Completion,
            Failure,
            ToolDetails,
            Subagents,
        ],
        // adapters/opencode/notchtap.ts::OPENCODE_CAPABILITIES — no
        // subagents (declared gap, that file's "Known gaps" doc).
        AgentRuntime::OpenCode => &[
            SessionLifecycle,
            PermissionRequests,
            InputRequired,
            Completion,
            Failure,
            ToolDetails,
        ],
    }
}

/// The full reference set every "fully `Available`" runtime must declare
/// in its entirety — Claude Code and (hook-supported) Kimi are the only
/// two that currently do.
const FULL_REFERENCE_CAPABILITY_COUNT: usize = 7;

/// Pure availability derivation (spec §10/§4.4). `kimi_hook` is `None`
/// for every runtime except Kimi, where it's the (possibly cached) probe
/// result — passing it in rather than probing inside this function keeps
/// it unit-testable without a real `kimi` binary on the test machine.
pub fn availability_for(
    runtime: AgentRuntime,
    enabled: bool,
    kimi_hook: Option<&HookSupport>,
) -> AdapterAvailability {
    if !enabled {
        return AdapterAvailability::Unavailable;
    }
    if runtime == AgentRuntime::Kimi {
        match kimi_hook {
            Some(HookSupport::Supported { .. }) | None => {}
            Some(HookSupport::Unavailable { .. }) => return AdapterAvailability::Unavailable,
        }
    }
    if declared_capabilities(runtime).len() >= FULL_REFERENCE_CAPABILITY_COUNT {
        AdapterAvailability::Available
    } else {
        AdapterAvailability::Partial
    }
}

/// Pure, human-readable setup-compatibility line (spec §4.6's "setup
/// compatibility message"). Never `None` for a disabled or gapped
/// runtime — only a fully healthy, ungapped runtime has nothing to add.
pub fn compatibility_message(
    runtime: AgentRuntime,
    enabled: bool,
    kimi_hook: Option<&HookSupport>,
) -> Option<String> {
    if !enabled {
        return Some("Disabled in Settings — enable this runtime to accept its events.".into());
    }
    if runtime == AgentRuntime::Kimi {
        match kimi_hook {
            Some(HookSupport::Unavailable { detected, minimum }) => {
                return Some(format!(
                    "Requires Kimi Code >= {minimum}; detected {}.",
                    detected.as_deref().unwrap_or("no local install found")
                ));
            }
            Some(HookSupport::Supported { detected }) => {
                return Some(format!("Kimi Code {detected} detected — hooks supported."));
            }
            None => {}
        }
    }
    match runtime {
        AgentRuntime::Codex => Some(
            "Codex's documented hook surface has no explicit-input-required or terminal-failure \
             event yet — those two states won't be reflected for Codex sessions until the \
             provider adds one."
                .into(),
        ),
        AgentRuntime::OpenCode => Some(
            "The OpenCode plugin does not yet declare subagent lifecycle — independent \
             sub-session rows may be incomplete until that's verified against a real session."
                .into(),
        ),
        AgentRuntime::ClaudeCode => None,
        AgentRuntime::Kimi => None,
    }
}

/// Pure combination of every input above into one [`AdapterHealth`] row
/// — the unit-testable "state derivation" half this ticket's checklist
/// names. `kimi_hook` is ignored for every runtime but Kimi.
pub fn build_adapter_health(
    runtime: AgentRuntime,
    enabled: bool,
    kimi_hook: Option<&HookSupport>,
    last_accepted_event_ms: Option<i64>,
    last_error_category: Option<AdapterErrorCategory>,
) -> AdapterHealth {
    AdapterHealth {
        runtime,
        enabled,
        availability: availability_for(runtime, enabled, kimi_hook),
        capabilities: declared_capabilities(runtime).to_vec(),
        last_accepted_event_ms,
        last_error_category,
        compatibility_message: compatibility_message(runtime, enabled, kimi_hook),
    }
}

/// Best-effort recovery of a *known* runtime from an otherwise-rejected
/// `/agent/events` body, used ONLY to attribute a bounded error category
/// to the right Adapter Health card (`http.rs`'s handler). Deliberately
/// separate from `adapter::parse_wire_event` (which already rejected the
/// body) — a second, tolerant, best-effort read of just the `runtime`
/// field, never surfaced to a caller as anything but a health-attribution
/// hint. Returns `None` for anything that isn't recognizable JSON with a
/// `runtime` string naming one of the four known runtimes — the
/// `UnsupportedRuntime` case in particular is expected to return `None`
/// here (see [`AdapterErrorCategory::UnsupportedRuntime`]'s own doc).
pub fn best_effort_runtime_hint(body: &[u8]) -> Option<AgentRuntime> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let raw = value.get("runtime")?.as_str()?;
    match raw {
        "claude-code" => Some(AgentRuntime::ClaudeCode),
        "codex" => Some(AgentRuntime::Codex),
        "kimi" => Some(AgentRuntime::Kimi),
        "opencode" => Some(AgentRuntime::OpenCode),
        _ => None,
    }
}

#[derive(Default)]
struct RuntimeRecord {
    last_accepted_event_ms: Option<i64>,
    last_error_category: Option<AdapterErrorCategory>,
}

struct TrackerInner {
    records: HashMap<AgentRuntime, RuntimeRecord>,
    kimi_cache: Option<(Instant, HookSupport)>,
}

/// The shared, impure half (this module's top doc). One instance is
/// app-managed (`lib.rs`, alongside `agent_registry`/`agent_board`) and
/// reached from both `http.rs`'s `/agent/events` handler (writes) and
/// the `agent-state` publish path + the Settings `get_agent_health`
/// command (reads via [`HealthTracker::snapshot`]).
pub struct HealthTracker {
    inner: StdMutex<TrackerInner>,
}

impl Default for HealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthTracker {
    pub fn new() -> Self {
        Self {
            inner: StdMutex::new(TrackerInner {
                records: HashMap::new(),
                kimi_cache: None,
            }),
        }
    }

    /// Records that a well-formed event from `runtime` was just accepted
    /// off the wire (parsed successfully) — called regardless of whether
    /// the runtime's own `[agents.runtimes.*]` toggle then went on to
    /// skip registry/Notification handling, since this field answers "is
    /// the adapter actually delivering", not "did notchtap act on it".
    pub fn record_accepted(&self, runtime: AgentRuntime, at_ms: i64) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .records
            .entry(runtime)
            .or_default()
            .last_accepted_event_ms = Some(at_ms);
    }

    /// Records a bounded error category for a known runtime — see
    /// [`best_effort_runtime_hint`]'s doc for why the caller can only
    /// ever supply a *known* `runtime` here, never the raw wire string.
    pub fn record_error(&self, runtime: AgentRuntime, category: AdapterErrorCategory) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .records
            .entry(runtime)
            .or_default()
            .last_error_category = Some(category);
    }

    #[cfg(test)]
    fn last_accepted(&self, runtime: AgentRuntime) -> Option<i64> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .get(&runtime)
            .and_then(|r| r.last_accepted_event_ms)
    }

    #[cfg(test)]
    fn last_error(&self, runtime: AgentRuntime) -> Option<AdapterErrorCategory> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .get(&runtime)
            .and_then(|r| r.last_error_category)
    }

    /// The one impure input this module needs: a (cached) Kimi hook-
    /// support read. `now` is caller-supplied (`Instant::now()` at the
    /// real call sites, a simulated clock in tests) so the cache TTL
    /// itself stays testable without a real 60-second sleep.
    pub fn kimi_hook_support(&self, now: Instant) -> HookSupport {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((probed_at, support)) = &guard.kimi_cache {
            if now.saturating_duration_since(*probed_at) < KIMI_PROBE_CACHE_TTL {
                return support.clone();
            }
        }
        let support = kimi_version::probe_hook_support();
        guard.kimi_cache = Some((now, support.clone()));
        support
    }

    /// Builds the full four-runtime health snapshot (declaration order,
    /// [`ALL_RUNTIMES`]) — the one call Settings' `get_agent_health` and
    /// the `agent-state` publish path both make.
    pub fn snapshot(&self, runtimes_cfg: &AgentRuntimesConfig, now: Instant) -> Vec<AdapterHealth> {
        let kimi_hook = self.kimi_hook_support(now);
        ALL_RUNTIMES
            .iter()
            .map(|&runtime| {
                let (last_accepted_event_ms, last_error_category) = {
                    let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    guard
                        .records
                        .get(&runtime)
                        .map(|r| (r.last_accepted_event_ms, r.last_error_category))
                        .unwrap_or_default()
                };
                build_adapter_health(
                    runtime,
                    runtimes_cfg.runtime_enabled(runtime),
                    if runtime == AgentRuntime::Kimi {
                        Some(&kimi_hook)
                    } else {
                        None
                    },
                    last_accepted_event_ms,
                    last_error_category,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_cfg() -> AgentRuntimesConfig {
        AgentRuntimesConfig::default()
    }

    // --- declared_capabilities / availability_for -------------------------

    #[test]
    fn claude_code_declares_the_full_seven_capability_set() {
        assert_eq!(declared_capabilities(AgentRuntime::ClaudeCode).len(), 7);
    }

    #[test]
    fn codex_is_missing_input_required_and_failure() {
        let caps = declared_capabilities(AgentRuntime::Codex);
        assert!(!caps.contains(&AgentCapability::InputRequired));
        assert!(!caps.contains(&AgentCapability::Failure));
    }

    #[test]
    fn opencode_is_missing_subagents() {
        let caps = declared_capabilities(AgentRuntime::OpenCode);
        assert!(!caps.contains(&AgentCapability::Subagents));
    }

    #[test]
    fn disabled_runtime_is_always_unavailable() {
        assert_eq!(
            availability_for(AgentRuntime::ClaudeCode, false, None),
            AdapterAvailability::Unavailable
        );
    }

    #[test]
    fn claude_code_enabled_is_available() {
        assert_eq!(
            availability_for(AgentRuntime::ClaudeCode, true, None),
            AdapterAvailability::Available
        );
    }

    #[test]
    fn codex_enabled_is_partial_not_available() {
        assert_eq!(
            availability_for(AgentRuntime::Codex, true, None),
            AdapterAvailability::Partial
        );
    }

    #[test]
    fn opencode_enabled_is_partial_not_available() {
        assert_eq!(
            availability_for(AgentRuntime::OpenCode, true, None),
            AdapterAvailability::Partial
        );
    }

    #[test]
    fn kimi_enabled_but_hook_unsupported_is_unavailable() {
        let hook = HookSupport::Unavailable {
            detected: Some("0.5.0".into()),
            minimum: "0.9.0",
        };
        assert_eq!(
            availability_for(AgentRuntime::Kimi, true, Some(&hook)),
            AdapterAvailability::Unavailable
        );
    }

    #[test]
    fn kimi_enabled_and_hook_supported_is_available() {
        let hook = HookSupport::Supported {
            detected: "1.0.0".into(),
        };
        assert_eq!(
            availability_for(AgentRuntime::Kimi, true, Some(&hook)),
            AdapterAvailability::Available
        );
    }

    // --- compatibility_message ---------------------------------------------

    #[test]
    fn disabled_message_mentions_settings() {
        let msg = compatibility_message(AgentRuntime::ClaudeCode, false, None).unwrap();
        assert!(msg.contains("Disabled in Settings"));
    }

    #[test]
    fn claude_code_enabled_has_no_compatibility_note() {
        assert_eq!(
            compatibility_message(AgentRuntime::ClaudeCode, true, None),
            None
        );
    }

    #[test]
    fn kimi_unavailable_message_reports_minimum_and_detected() {
        let hook = HookSupport::Unavailable {
            detected: Some("0.5.0".into()),
            minimum: "0.9.0",
        };
        let msg = compatibility_message(AgentRuntime::Kimi, true, Some(&hook)).unwrap();
        assert!(msg.contains("0.9.0"));
        assert!(msg.contains("0.5.0"));
    }

    #[test]
    fn kimi_supported_message_reports_detected_version() {
        let hook = HookSupport::Supported {
            detected: "1.2.3".into(),
        };
        let msg = compatibility_message(AgentRuntime::Kimi, true, Some(&hook)).unwrap();
        assert!(msg.contains("1.2.3"));
    }

    #[test]
    fn codex_and_opencode_always_carry_a_gap_note_when_enabled() {
        assert!(compatibility_message(AgentRuntime::Codex, true, None).is_some());
        assert!(compatibility_message(AgentRuntime::OpenCode, true, None).is_some());
    }

    // --- AdapterErrorCategory::from_adapter_error ---------------------------

    #[test]
    fn every_adapter_error_variant_maps_to_a_bounded_category() {
        use super::super::adapter::AdapterError;
        assert_eq!(
            AdapterErrorCategory::from_adapter_error(&AdapterError::MalformedJson("x".into())),
            AdapterErrorCategory::MalformedPayload
        );
        assert_eq!(
            AdapterErrorCategory::from_adapter_error(&AdapterError::UnsupportedSchemaVersion(9)),
            AdapterErrorCategory::MalformedPayload
        );
        assert_eq!(
            AdapterErrorCategory::from_adapter_error(&AdapterError::MissingIdentity("eventId")),
            AdapterErrorCategory::MalformedPayload
        );
        assert_eq!(
            AdapterErrorCategory::from_adapter_error(&AdapterError::MalformedEnum {
                field: "kind",
                value: "bogus".into(),
            }),
            AdapterErrorCategory::MalformedPayload
        );
        assert_eq!(
            AdapterErrorCategory::from_adapter_error(&AdapterError::UnsupportedRuntime(
                "bogus".into()
            )),
            AdapterErrorCategory::UnsupportedRuntime
        );
    }

    // --- best_effort_runtime_hint -------------------------------------------

    #[test]
    fn runtime_hint_recovers_a_known_runtime_from_an_otherwise_malformed_body() {
        let body = br#"{"runtime": "codex", "kind": "not-a-real-kind"}"#;
        assert_eq!(best_effort_runtime_hint(body), Some(AgentRuntime::Codex));
    }

    #[test]
    fn runtime_hint_is_none_for_an_unknown_runtime_string() {
        let body = br#"{"runtime": "gpt-cli"}"#;
        assert_eq!(best_effort_runtime_hint(body), None);
    }

    #[test]
    fn runtime_hint_is_none_for_unparseable_json() {
        assert_eq!(best_effort_runtime_hint(b"not json"), None);
    }

    // --- HealthTracker: last-seen / error bookkeeping -----------------------

    #[test]
    fn record_accepted_then_read_back_last_accepted() {
        let tracker = HealthTracker::new();
        assert_eq!(tracker.last_accepted(AgentRuntime::Codex), None);
        tracker.record_accepted(AgentRuntime::Codex, 1_000);
        assert_eq!(tracker.last_accepted(AgentRuntime::Codex), Some(1_000));
        // A later event overwrites, never accumulates.
        tracker.record_accepted(AgentRuntime::Codex, 2_000);
        assert_eq!(tracker.last_accepted(AgentRuntime::Codex), Some(2_000));
    }

    #[test]
    fn record_error_then_read_back_last_error() {
        let tracker = HealthTracker::new();
        assert_eq!(tracker.last_error(AgentRuntime::Kimi), None);
        tracker.record_error(AgentRuntime::Kimi, AdapterErrorCategory::MalformedPayload);
        assert_eq!(
            tracker.last_error(AgentRuntime::Kimi),
            Some(AdapterErrorCategory::MalformedPayload)
        );
    }

    #[test]
    fn per_runtime_bookkeeping_does_not_cross_contaminate() {
        let tracker = HealthTracker::new();
        tracker.record_accepted(AgentRuntime::ClaudeCode, 500);
        assert_eq!(tracker.last_accepted(AgentRuntime::Codex), None);
        assert_eq!(tracker.last_accepted(AgentRuntime::ClaudeCode), Some(500));
    }

    // --- HealthTracker::snapshot ---------------------------------------------

    #[test]
    fn snapshot_carries_all_four_runtimes_in_declaration_order() {
        let tracker = HealthTracker::new();
        let snapshot = tracker.snapshot(&enabled_cfg(), Instant::now());
        assert_eq!(snapshot.len(), 4);
        assert_eq!(
            snapshot.iter().map(|h| h.runtime).collect::<Vec<_>>(),
            ALL_RUNTIMES.to_vec()
        );
    }

    #[test]
    fn snapshot_reflects_a_disabled_runtime_toggle() {
        let tracker = HealthTracker::new();
        let mut cfg = enabled_cfg();
        cfg.codex.enabled = false;
        let snapshot = tracker.snapshot(&cfg, Instant::now());
        let codex = snapshot
            .iter()
            .find(|h| h.runtime == AgentRuntime::Codex)
            .unwrap();
        assert!(!codex.enabled);
        assert_eq!(codex.availability, AdapterAvailability::Unavailable);
    }

    #[test]
    fn snapshot_reflects_recorded_last_accepted_and_error() {
        let tracker = HealthTracker::new();
        tracker.record_accepted(AgentRuntime::ClaudeCode, 42);
        tracker.record_error(
            AgentRuntime::ClaudeCode,
            AdapterErrorCategory::MalformedPayload,
        );
        let snapshot = tracker.snapshot(&enabled_cfg(), Instant::now());
        let claude = snapshot
            .iter()
            .find(|h| h.runtime == AgentRuntime::ClaudeCode)
            .unwrap();
        assert_eq!(claude.last_accepted_event_ms, Some(42));
        assert_eq!(
            claude.last_error_category,
            Some(AdapterErrorCategory::MalformedPayload)
        );
    }

    #[test]
    fn kimi_hook_probe_is_cached_within_the_ttl() {
        let tracker = HealthTracker::new();
        let base = Instant::now();
        let first = tracker.kimi_hook_support(base);
        // Still within the TTL — must be byte-identical to the first
        // read without a second real subprocess call (can't assert "no
        // subprocess ran" directly, but a changed environment between
        // calls would be the only way these could differ, and the cache
        // must win regardless).
        let second = tracker.kimi_hook_support(base + Duration::from_secs(1));
        assert_eq!(first, second);
    }
}
