//! Pure Adapter Health derivation plus shared probe and bounded-error bookkeeping.

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use super::model::{AgentCapability, AgentRuntime};
use super::providers::kimi_version::{self, HookSupport};
use crate::config::AgentRuntimesConfig;

/// How long a cached `kimi --version` probe is trusted before [`HealthTracker::kimi_hook_support`]
/// shells out again.
pub const KIMI_PROBE_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterAvailability {
    Available,
    Partial,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterErrorCategory {
    MalformedPayload,
    /// `AdapterError::UnsupportedRuntime` — the wire `runtime` string itself didn't name one of the
    /// four known runtimes.
    UnsupportedRuntime,
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

    /// Maps a wire-parse [`super::adapter::AdapterError`] onto a bounded category.
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

#[derive(Debug, Clone, PartialEq)]
pub struct AdapterHealth {
    pub runtime: AgentRuntime,
    pub enabled: bool,
    pub availability: AdapterAvailability,
    pub capabilities: Vec<AgentCapability>,
    pub last_accepted_event_ms: Option<i64>,
    pub last_error_category: Option<AdapterErrorCategory>,
    pub compatibility_message: Option<String>,
}

/// Declaration order used everywhere a full four-runtime health snapshot is built — matches
/// [`AgentRuntime`]'s own declaration order: Claude Code, Codex, Kimi, OpenCode.
pub const ALL_RUNTIMES: [AgentRuntime; 4] = [
    AgentRuntime::ClaudeCode,
    AgentRuntime::Codex,
    AgentRuntime::Kimi,
    AgentRuntime::OpenCode,
];

/// The per-runtime capability row, restricted (like every provider parser's own `CAPABILITIES`
/// const) to what that adapter actually declares on the wire.
pub fn declared_capabilities(runtime: AgentRuntime) -> &'static [AgentCapability] {
    use AgentCapability::*;
    match runtime {
        AgentRuntime::ClaudeCode => &[
            SessionLifecycle,
            PermissionRequests,
            InputRequired,
            Completion,
            Failure,
            ToolDetails,
            Subagents,
        ],
        AgentRuntime::Codex => &[
            SessionLifecycle,
            PermissionRequests,
            Completion,
            ToolDetails,
            Subagents,
        ],
        AgentRuntime::Kimi => &[
            SessionLifecycle,
            PermissionRequests,
            InputRequired,
            Completion,
            Failure,
            ToolDetails,
            Subagents,
        ],
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

/// The full reference set every "fully `Available`" runtime must declare in its entirety — Claude
/// Code and (hook-supported) Kimi are the only two that currently do.
const FULL_REFERENCE_CAPABILITY_COUNT: usize = 7;

/// Pure availability derivation.
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

/// Pure, human-readable setup-compatibility line. Never `None` for a disabled or gapped runtime —
/// only a fully healthy, ungapped runtime has nothing to add.
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

/// Pure combination of every input above into one [`AdapterHealth`] row — the unit-testable
/// state-derivation half.
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

/// The shared, impure half (this module's top doc).
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

    /// Records that a well-formed event from `runtime` was just accepted off the wire (parsed
    /// successfully).
    pub fn record_accepted(&self, runtime: AgentRuntime, at_ms: i64) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .records
            .entry(runtime)
            .or_default()
            .last_accepted_event_ms = Some(at_ms);
    }

    /// Records a bounded error category for a known runtime — see [`best_effort_runtime_hint`]'s
    /// doc for why the caller can only ever supply a *known* `runtime` here.
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

    /// The one impure input this module needs: a (cached) Kimi hook- support read.
    pub fn kimi_hook_support(&self, now: Instant) -> HookSupport {
        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((probed_at, support)) = &guard.kimi_cache {
                if now.saturating_duration_since(*probed_at) < KIMI_PROBE_CACHE_TTL {
                    return support.clone();
                }
            }
        } // guard dropped HERE — the probe below spawns a process, and
          // Holding it across a subprocess would stall ingestion, not just health reads.
        let support = kimi_version::probe_hook_support();
        // Deliberate, benign race: two callers arriving together on an expired cache may both
        // probe.
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.kimi_cache = Some((now, support.clone()));
        support
    }

    /// Builds the full four-runtime health snapshot (declaration order, [`ALL_RUNTIMES`]) — the one
    /// call Settings' `get_agent_health` and the `agent-state` publish path both make.
    pub fn snapshot(&self, runtimes_cfg: &AgentRuntimesConfig, now: Instant) -> Vec<AdapterHealth> {
        // A user who never installed Kimi should not pay a process spawn every
        // `KIMI_PROBE_CACHE_TTL` forever.
        let kimi_hook = if runtimes_cfg.runtime_enabled(AgentRuntime::Kimi) {
            Some(self.kimi_hook_support(now))
        } else {
            None
        };
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
                        kimi_hook.as_ref()
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

    #[test]
    fn record_accepted_then_read_back_last_accepted() {
        let tracker = HealthTracker::new();
        assert_eq!(tracker.last_accepted(AgentRuntime::Codex), None);
        tracker.record_accepted(AgentRuntime::Codex, 1_000);
        assert_eq!(tracker.last_accepted(AgentRuntime::Codex), Some(1_000));
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
        let second = tracker.kimi_hook_support(base + Duration::from_secs(1));
        assert_eq!(first, second);
    }

    #[test]
    fn snapshot_skips_the_kimi_probe_when_kimi_is_disabled() {
        let tracker = HealthTracker::new();
        let mut cfg = enabled_cfg();
        cfg.kimi.enabled = false;
        let snapshot = tracker.snapshot(&cfg, Instant::now());
        let kimi = snapshot
            .iter()
            .find(|h| h.runtime == AgentRuntime::Kimi)
            .unwrap();
        assert!(!kimi.enabled);
        assert_eq!(kimi.availability, AdapterAvailability::Unavailable);
        assert_eq!(
            kimi.compatibility_message.as_deref(),
            Some("Disabled in Settings — enable this runtime to accept its events.")
        );
    }
}
