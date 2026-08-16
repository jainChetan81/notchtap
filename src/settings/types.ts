// Wire/config types shared by the settings window. SettingsApp.tsx re-exports
// everything here, so external import paths are unchanged.

export interface RssFeedConfig {
  url: string;
  source: string | null;
  category: string | null;
}

export type PriorityLevel = "low" | "medium" | "high";
export type SourceKind = "football" | "manual" | "news" | "agent";
export type RestingState = "rail" | "notch";

export interface AppearanceConfig {
  card_scale: number;
  card_radius: number;
  card_opacity: number;
}

// Mirrors rust's `AgentRuntimesConfig` (config.rs) — one enable flag per
// supported runtime.
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

// Mirrors rust's `SilenceConfig` (config.rs) — the `[silence]` block. `window`
// is a plain `"HH:MM-HH:MM"` (24h) string on the wire, never a structured
// `{start, end}` object.
export interface SilenceConfig {
  enabled: boolean;
  window: string;
}

// Mirrors rust's `AgentsConfig` (config.rs) — the `[agents]` config block.
export interface AgentsConfig {
  enabled: boolean;
  terminal_retention_secs: number;
  stale_after_secs: number;
  stale_retention_secs: number;
  informational_notifications: boolean;
  // Default `true` — a runtime fires a completion event per response
  // turn, so this is the operator's off switch for per-turn cards.
  completion_notifications: boolean;
  // Default `false`: a merely-working session never summons the Agent Board;
  // it appears only while something needs the operator. Presence only — once
  // up, the Board still lists working sessions. Rust owns the gate.
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
  // Always present on the wire (`#[serde(default)]` rust-side), so required
  // here, not optional.
  agents: AgentsConfig;
  rotation_order: SourceKind[];
  appearance: AppearanceConfig;
  resting_state: RestingState;
  history_enabled: boolean;
  // Always present on the wire (`#[serde(default)]` rust-side), so required
  // here, not optional.
  silence: SilenceConfig;
  // The configurable tmux-style Prefix (src-tauri/src/prefix.rs). A plain
  // `"⌃⇧" + key name` string on the wire, not a structured `{modifiers, key}`
  // object. Always present (`#[serde(default)]` rust-side), so required here.
  prefix_shortcut: string;
}

// Wire shape of get_history — mirrors HistoryEntry/Event in
// src-tauri/src/history.rs and event.rs. snake_case throughout; the one
// camelCase island is the optional `meta.espn` block (EspnMeta derives
// `rename_all = "camelCase"`), absent unless the espn live card populated it.
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

// Wire shape of get_queue — mirrors QueueItemSummary in src-tauri/src/queue.rs.
// Wire spellings match `PriorityLevel`/`SourceKind`, so those types and their
// label maps apply unchanged.
export interface QueueItemSummary {
  title: string;
  priority: PriorityLevel;
  source: SourceKind;
}

// Wire shape of get_about_info — mirrors AboutInfo in src-tauri/src/about.rs,
// camelCase. `bundleSizeBytes`/`disk*Bytes` are null when unavailable (dev
// build, no disk at "/") — never 0, so null can't collide with a real reading.
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

// Wire-token spelling (`agents::adapter::runtime_wire_label`) — kebab-case,
// distinct from `AgentsConfig.runtimes`'s snake_case config keys. Kept local
// rather than imported across the overlay/settings entry-point boundary.
export type AgentWireRuntime = "claude-code" | "codex" | "kimi" | "opencode";

// Wire shape of `get_agent_health` — mirrors `agents::board::AdapterHealthView`
// in src-tauri/src/agents/board.rs, camelCase.
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

export type TestSource = "football" | "news" | "manual" | "agent";

export const PRIORITY_LABELS: Record<PriorityLevel, string> = {
  low: "Low",
  medium: "Medium",
  high: "High",
};
export const PRIORITY_LEVELS: PriorityLevel[] = ["low", "medium", "high"];

export const SOURCE_LABELS: Record<SourceKind, string> = {
  football: "Football",
  manual: "Manual / CLI push",
  news: "News",
  agent: "Agent",
};

// Precomputed once so call sites don't rebuild the array every render.
export const PRIORITY_SEGMENT_OPTIONS: ReadonlyArray<{ label: string; value: PriorityLevel }> =
  PRIORITY_LEVELS.map((level) => ({ label: PRIORITY_LABELS[level], value: level }));

// Segmented's `optionTones` for every priority picker — mirrors the overlay's
// per-tier accent scheme (src/overlay/card-chrome.css) so selected priority
// reads as the notification card's own distinction. Literal Tailwind class
// strings only — see Segmented's `optionTones` doc for why.
export const PRIORITY_TONES: Record<PriorityLevel, string> = {
  low: "bg-muted-foreground/20 text-foreground",
  medium: "bg-overlay-teal/20 text-overlay-teal",
  high: "bg-overlay-coral/20 text-overlay-coral",
};
