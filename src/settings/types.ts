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
export type SourceKind = "football" | "manual" | "news" | "cmux" | "weather";
export type Units = "celsius" | "fahrenheit";
export type RestingState = "rail" | "notch";

export interface AppearanceConfig {
  card_scale: number;
  card_radius: number;
  card_opacity: number;
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
  rss_poll_secs: number;
  rss_priority: PriorityLevel;
  rss_ttl_secs: number;
  rss_max_per_poll: number;
  manual_default_priority: PriorityLevel;
  cmux_priority: PriorityLevel;
  cmux_ttl_secs: number;
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
  connectors: {
    telegram: {
      enabled: boolean;
    };
  };
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
}

export interface SecretStatus {
  openrouter_api_key: string | null;
  telegram_bot_token: string | null;
  telegram_chat_id: string | null;
}

// Wire shape of get_connector_health (plan 076) — mirrors
// ConnectorHealthDto in src-tauri/src/settings.rs: elapsed-ms timestamps,
// not instants.
export interface ConnectorHealthDto {
  lastAttemptMs: number | null;
  lastSuccessMs: number | null;
  consecutiveFailures: number;
}

// Wire shape of get_history (plan 089) — mirrors HistoryEntry/Event in
// src-tauri/src/history.rs and event.rs. Unlike ConnectorHealthDto above
// and unlike the camelCase SlotState wire (useSlotState.ts), this shape
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

export type SecretField = keyof SecretStatus;

export type TestSource = "football" | "news" | "cmux" | "manual" | "weather";

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
  cmux: "Cmux (agent relay)",
  manual: "Manual / CLI push",
  news: "News",
  weather: "Weather",
};

// Segmented option lists for the priority and units controls (plan 119:
// precomputed once so call sites don't rebuild the array every render,
// matching the static module-level arrays the old per-control components
// closed over).
export const PRIORITY_SEGMENT_OPTIONS: ReadonlyArray<{ label: string; value: PriorityLevel }> =
  PRIORITY_LEVELS.map((level) => ({ label: PRIORITY_LABELS[level], value: level }));

export const UNITS_SEGMENT_OPTIONS: ReadonlyArray<{ label: string; value: Units }> =
  UNITS_OPTIONS.map((unit) => ({ label: UNITS_LABELS[unit], value: unit }));
