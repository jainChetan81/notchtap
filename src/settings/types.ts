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

export interface SilenceConfig {
  enabled: boolean;
  window: string;
}

export interface AgentsConfig {
  enabled: boolean;
  terminal_retention_secs: number;
  stale_after_secs: number;
  stale_retention_secs: number;
  informational_notifications: boolean;
  completion_notifications: boolean;
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
  agents: AgentsConfig;
  rotation_order: SourceKind[];
  appearance: AppearanceConfig;
  resting_state: RestingState;
  history_enabled: boolean;
  silence: SilenceConfig;
  prefix_shortcut: string;
}

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

export interface QueueItemSummary {
  title: string;
  priority: PriorityLevel;
  source: SourceKind;
}

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

export type AgentWireRuntime = "claude-code" | "codex" | "kimi" | "opencode";

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

export const PRIORITY_LABELS = {
  low: "Low",
  medium: "Medium",
  high: "High",
} satisfies Record<PriorityLevel, string>;
export const PRIORITY_LEVELS: PriorityLevel[] = ["low", "medium", "high"];

export const SOURCE_LABELS = {
  football: "Football",
  manual: "Manual / CLI push",
  news: "News",
  agent: "Agent",
} satisfies Record<SourceKind, string>;

export const PRIORITY_SEGMENT_OPTIONS: ReadonlyArray<{ label: string; value: PriorityLevel }> =
  PRIORITY_LEVELS.map((level) => ({ label: PRIORITY_LABELS[level], value: level }));

export const PRIORITY_TONES = {
  low: "bg-muted-foreground/20 text-foreground",
  medium: "bg-overlay-teal/20 text-overlay-teal",
  high: "bg-overlay-coral/20 text-overlay-coral",
} satisfies Record<PriorityLevel, string>;
