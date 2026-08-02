// Wire/config types shared by the settings window (plan 119: extracted
// from SettingsApp.tsx so sections, controls, and the ipc map can import
// them without pulling in the whole shell). SettingsApp.tsx re-exports
// everything here, so external import paths (the test suite) are
// unchanged.

export interface RssFeedConfig {
  url: string;
  source: string | null;
  category: string | null;
}

export type PriorityLevel = "low" | "medium" | "high";
// plan 137 (spec §7/§12): "cmux" is gone — migrated onto "agent"
// (rust's `SourceKind::Cmux` was removed and its slot taken by
// `SourceKind::Agent`, the v7 Agent Adapter's origin).
export type SourceKind = "football" | "manual" | "news" | "weather" | "agent";
export type Units = "celsius" | "fahrenheit";
export type RestingState = "rail" | "notch";

export interface AppearanceConfig {
  card_scale: number;
  card_radius: number;
  card_opacity: number;
}

// Plan 143 (v7 ticket 11 of 13): mirrors rust's `AgentRuntimesConfig`
// (config.rs) — one enable flag per supported runtime.
export interface AgentRuntimeToggle {
  enabled: boolean;
}

export interface AgentRuntimesConfig {
  claude_code: AgentRuntimeToggle;
  codex: AgentRuntimeToggle;
  kimi: AgentRuntimeToggle;
  opencode: AgentRuntimeToggle;
}

export type AgentAdapterRuntime = "claude_code" | "codex" | "kimi" | "opencode";

// plan 146a: mirrors rust's `SilenceConfig` (config.rs) — the `[silence]`
// block. `window` is a plain `"HH:MM-HH:MM"` (24h) string on the wire —
// `silence::Window`'s own `Serialize`/`Deserialize` impls round-trip it
// through `Display`/`parse`, never a structured `{start, end}` object.
export interface SilenceConfig {
  enabled: boolean;
  window: string;
}

// Mirrors rust's `AgentsConfig` (config.rs) — the `[agents]` v7 config
// block: global enable, registry retention/staleness, the two per-kind
// card toggles (informational, completion), the Agent Board's own
// presence gate (board_show_working), four per-kind Notification
// priorities, and the four per-runtime enable flags above.
export interface AgentsConfig {
  enabled: boolean;
  terminal_retention_secs: number;
  stale_after_secs: number;
  stale_retention_secs: number;
  informational_notifications: boolean;
  // Default `true` — a runtime fires a completion event per response
  // turn, so this is the operator's off switch for per-turn cards.
  completion_notifications: boolean;
  // Operator decision 2026-08-02. Default `false`, INCLUDING for a
  // config written before the key existed — a session that is merely
  // working no longer summons the Agent Board at all; the Board appears
  // only while something needs the operator (waiting for permission or
  // input, failed, or a completed session still inside its retention
  // window). Presence only: once the Board is up it still lists the
  // working sessions. Rust owns the whole gate
  // (`agents::board::AgentBoardPublisher::gate_presence`) — the overlay
  // never sees this flag, it just reads the published snapshot.
  board_show_working: boolean;
  permission_priority: PriorityLevel;
  input_priority: PriorityLevel;
  failure_priority: PriorityLevel;
  completion_priority: PriorityLevel;
  runtimes: AgentRuntimesConfig;
}

export interface Config {
  port: number;
  default_ttl: number;
  max_queued_per_tier: number;
  detect_path: string;
  start_paused: boolean;
  espn_enabled: boolean;
  espn_leagues: string[];
  espn_poll_secs: number;
  espn_priority: PriorityLevel;
  espn_ttl_secs: number;
  espn_live_card: boolean;
  espn_rich_events: boolean;
  rss_enabled: boolean;
  rss_feeds: RssFeedConfig[];
  rss_topics: string[];
  rss_poll_secs: number;
  rss_priority: PriorityLevel;
  rss_ttl_secs: number;
  rss_max_per_poll: number;
  manual_default_priority: PriorityLevel;
  agent_priority: PriorityLevel;
  agent_ttl_secs: number;
  // plan 143 (v7 ticket 11 of 13): the `[agents]` config block — see
  // `AgentsConfig`'s own doc. Always present on the wire
  // (`#[serde(default)]` on the rust side), so this field is required,
  // not optional, here.
  agents: AgentsConfig;
  weather_enabled: boolean;
  weather_lat: number;
  weather_lon: number;
  weather_units: Units;
  weather_poll_secs: number;
  weather_rain_threshold_pct: number;
  weather_rain_lookahead_mins: number;
  weather_temp_hot_c: number;
  weather_temp_cold_c: number;
  weather_priority: PriorityLevel;
  rotation_order: SourceKind[];
  appearance: AppearanceConfig;
  resting_state: RestingState;
  history_enabled: boolean;
  // plan 104: the panel-editable toggle only. The rust-side kill-switch
  // field and the adapter install-dir field are deliberately OMITTED from
  // this type — a done criterion for this plan forbids this file from
  // naming the kill switch at all. The real config object the settings
  // window round-trips (`get_config`/`save_config_and_relaunch`) still
  // carries both fields at runtime regardless of this type's shape (TS
  // types are erased, not enforced against the actual JSON payload), and
  // the rust save path pins both to the booted value server-side either
  // way (`settings.rs`'s `pin_uneditable_fields`) — so omitting them here
  // costs nothing functionally, unlike `detect_path` above, which stays
  // in this type only because nothing in this plan required removing it.
  now_playing_enabled: boolean;
  // plan 146a: the `[silence]` block — always present on the wire
  // (`#[serde(default)]` on the rust side), same "required, not optional"
  // discipline as `agents` above.
  silence: SilenceConfig;
  // plan 171 (tab-notch redesign, slice J; spec §9): the configurable
  // tmux-style prefix (`src-tauri/src/prefix.rs`'s `PrefixState`). A
  // plain `"⌃⇧" + one more key name` string on the wire — mirrors this
  // app's own shipped `⌃⇧`-combo display convention
  // (`ShortcutsSection.tsx`'s `⌃⇧N`/`⌃⇧O`/etc. table), not a structured
  // `{modifiers, key}` object. Always present
  // (`#[serde(default = "default_prefix_shortcut")]` on the rust side),
  // same "required, not optional" discipline as `silence`/`agents` above.
  // Data only in this slice — not yet wired to a live key grab.
  prefix_shortcut: string;
}

export interface SecretStatus {
  openrouter_api_key: string | null;
}

// Wire shape of get_history (plan 089) — mirrors HistoryEntry/Event in
// src-tauri/src/history.rs and event.rs. Unlike AboutInfo/AdapterHealthDto
// below and unlike the camelCase SlotState wire (useSlotState.ts), this shape
// is snake_case throughout, INCLUDING `meta` — the one camelCase island
// is the optional `meta.espn` block (EspnMeta derives
// `rename_all = "camelCase"`), absent entirely unless the espn live card
// populated it. Verified against a live serde_json::to_string print of a
// real HistoryEntry, not derived from the SlotState convention.
export interface HistoryDetailItem {
  label: string;
  value: string;
}

export interface HistoryEspnMeta {
  league: string;
  homeAbbrev: string;
  awayAbbrev: string;
  homeScore: number;
  awayScore: number;
  clock: string;
  homeCards: [number, number];
  awayCards: [number, number];
  homeCrest: string | null;
  awayCrest: string | null;
}

export interface HistoryEventMeta {
  source: string | null;
  category: string | null;
  published_at_ms: number | null;
  link: string | null;
  subtitle: string | null;
  details: HistoryDetailItem[];
  espn?: HistoryEspnMeta;
}

export type HistoryRotationSpec =
  | { kind: "one_shot"; ttl_secs: number }
  | { kind: "recurring"; display_secs: number };

export interface HistoryEvent {
  id: string;
  event_type: string;
  priority: PriorityLevel;
  rotation: HistoryRotationSpec;
  topic: string | null;
  payload: { title: string; body: string };
  meta: HistoryEventMeta;
  signal: string;
  origin: SourceKind;
}

export interface HistoryEntry {
  recorded_at_ms: number;
  event: HistoryEvent;
}

// Wire shape of get_queue (plan 121) — mirrors QueueItemSummary in
// src-tauri/src/queue.rs. `priority`/`source` are plain lowercase
// strings on the rust side, produced by an exhaustive match rather than
// serialized from the `Priority`/`SourceKind` enums directly — but the
// wire spelling is identical to `PriorityLevel`/`SourceKind` elsewhere
// in this file, so those existing types (and their label maps) apply
// here unchanged rather than duplicating a third "priority string"
// type.
export interface QueueItemSummary {
  title: string;
  priority: PriorityLevel;
  source: SourceKind;
}

// Wire shape of get_about_info (About section) — mirrors AboutInfo in
// src-tauri/src/about.rs. camelCase throughout, same convention as
// AdapterHealthDto below. The two "None on best-effort failure" fields
// (bundleSizeBytes for a dev build, disk*Bytes if no disk mounts at "/")
// stay nullable here rather than defaulting to 0 — a real zero-byte
// bundle isn't a state this app can be in, so `null` unambiguously means
// "not available" instead of colliding with a real reading.
export interface AboutInfo {
  version: string;
  bundleId: string;
  bundleSizeBytes: number | null;
  platform: string;
  arch: string;
  processMemoryBytes: number;
  systemMemoryUsedBytes: number;
  systemMemoryTotalBytes: number;
  diskUsedBytes: number | null;
  diskTotalBytes: number | null;
  uptimeSecs: number;
}

// Plan 143 (v7 ticket 11 of 13): the wire-token spelling
// (`agents::adapter::runtime_wire_label`) — kebab-case, distinct from
// `AgentsConfig.runtimes`'s snake_case config-field keys above. Mirrors
// `src/useAgentState.ts`'s own `AgentRuntime` union (kept as a separate
// local type here rather than importing across the overlay/settings
// entry-point boundary — see `vite.config.ts`'s two-entry split).
export type AgentWireRuntime = "claude-code" | "codex" | "kimi" | "opencode";

// Wire shape of `get_agent_health` (plan 143) — mirrors
// `agents::board::AdapterHealthView` in src-tauri/src/agents/board.rs,
// the same conversion the `agent-state` overlay channel's Adapter Health
// rows use (`health_to_view`). camelCase throughout, same convention as
// `AboutInfo` above.
export type AdapterAvailability = "available" | "partial" | "unavailable";
export type AdapterErrorCategory = "malformed_payload" | "unsupported_runtime" | "internal";

export interface AdapterHealthDto {
  runtime: AgentWireRuntime;
  status: AdapterAvailability;
  enabled: boolean;
  capabilities: string[];
  lastAcceptedEventMs: number | null;
  lastErrorCategory: AdapterErrorCategory | null;
  compatibilityMessage: string | null;
}

export type SecretField = keyof SecretStatus;

export type TestSource = "football" | "news" | "manual" | "weather" | "agent";

export const PRIORITY_LABELS: Record<PriorityLevel, string> = {
  low: "Low",
  medium: "Medium",
  high: "High",
};
export const PRIORITY_LEVELS: PriorityLevel[] = ["low", "medium", "high"];

export const UNITS_LABELS: Record<Units, string> = {
  celsius: "Celsius",
  fahrenheit: "Fahrenheit",
};
export const UNITS_OPTIONS: Units[] = ["celsius", "fahrenheit"];

export const SOURCE_LABELS: Record<SourceKind, string> = {
  football: "Football",
  manual: "Manual / CLI push",
  news: "News",
  weather: "Weather",
  agent: "Agent",
};

// Segmented option lists for the priority and units controls (plan 119:
// precomputed once so call sites don't rebuild the array every render,
// matching the static module-level arrays the old per-control components
// closed over).
export const PRIORITY_SEGMENT_OPTIONS: ReadonlyArray<{ label: string; value: PriorityLevel }> =
  PRIORITY_LEVELS.map((level) => ({ label: PRIORITY_LABELS[level], value: level }));

export const UNITS_SEGMENT_OPTIONS: ReadonlyArray<{ label: string; value: Units }> =
  UNITS_OPTIONS.map((unit) => ({ label: UNITS_LABELS[unit], value: unit }));
