import { invoke } from "@tauri-apps/api/core";
import {
  CloudSun,
  Command,
  KeyRound,
  type LucideIcon,
  Newspaper,
  Palette,
  ScrollText,
  SlidersHorizontal,
  Terminal,
  Trophy,
} from "lucide-react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { type CSSProperties, type FormEvent, type ReactNode, useEffect, useState } from "react";
import { StatusRailCard } from "../components/StatusRailCard";
import type { SlotState } from "../useSlotState";
import "./preview-overlay.css";

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

type SecretField = keyof SecretStatus;
type SectionId =
  | "general"
  | "football"
  | "news"
  | "cmux"
  | "weather"
  | "connectors"
  | "shortcuts"
  | "appearance"
  | "diagnostics";

const navigation: ReadonlyArray<{
  id: SectionId;
  label: string;
  icon: LucideIcon;
}> = [
  { id: "general", label: "General", icon: SlidersHorizontal },
  { id: "football", label: "Football", icon: Trophy },
  { id: "news", label: "News", icon: Newspaper },
  { id: "cmux", label: "Cmux", icon: Terminal },
  { id: "weather", label: "Weather", icon: CloudSun },
  { id: "connectors", label: "Connectors & Keys", icon: KeyRound },
  { id: "shortcuts", label: "Shortcuts", icon: Command },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "diagnostics", label: "Diagnostics", icon: ScrollText },
];

const sectionCopy: Record<SectionId, { index: string; title: string; description: string }> = {
  general: {
    index: "01",
    title: "General",
    description: "Control startup, the local listener, and how notifications rotate.",
  },
  football: {
    index: "02",
    title: "Football",
    description: "Choose the leagues and cadence used for live score checks.",
  },
  news: {
    index: "03",
    title: "News",
    description: "Manage RSS sources and the pace of headline delivery.",
  },
  cmux: {
    index: "04",
    title: "Cmux",
    description: "Set the priority and rotation time for notifications relayed by cmux.",
  },
  weather: {
    index: "05",
    title: "Weather",
    description:
      "Show ambient conditions in the idle rail and alert on rain and temperature thresholds.",
  },
  connectors: {
    index: "06",
    title: "Connectors & Keys",
    description: "Configure outbound Telegram delivery and write-only credentials.",
  },
  shortcuts: {
    index: "07",
    title: "Shortcuts",
    description: "A reference for the global controls available while notchtap runs.",
  },
  appearance: {
    index: "08",
    title: "Appearance",
    description: "Preview the overlay's shape and animations, and send live test notifications.",
  },
  diagnostics: {
    index: "09",
    title: "Diagnostics",
    description: "Read the app's recent log lines without leaving settings.",
  },
};

const secretRows: ReadonlyArray<{
  field: SecretField;
  id: string;
  label: string;
  placeholder: string;
}> = [
  {
    field: "openrouter_api_key",
    id: "openrouter-key",
    label: "OpenRouter API key",
    placeholder: "Enter a new key",
  },
  {
    field: "telegram_bot_token",
    id: "telegram-token",
    label: "Telegram bot token",
    placeholder: "Enter a replacement token",
  },
  {
    field: "telegram_chat_id",
    id: "telegram-chat-id",
    label: "Telegram chat ID",
    placeholder: "Enter a new chat ID",
  },
];

const shortcuts = [
  {
    keys: "⌃⇧N",
    action: "Expand or collapse the slot (manual)",
    status: "active",
  },
  { keys: "⌃⇧O", action: "Open the current story's link", status: "active" },
  {
    keys: "⌃⇧X",
    action: "Dismiss the visible notification now",
    status: "active",
  },
  { keys: "⌃⇧P", action: "Pause or resume promotion", status: "active" },
  { keys: "⌃⇧]", action: "Skip to the next waiting item", status: "active" },
  { keys: "⌃⇧,", action: "Open settings", status: "active" },
] as const;

function copyConfig(config: Config): Config {
  return {
    ...config,
    espn_leagues: [...config.espn_leagues],
    rss_feeds: config.rss_feeds.map((feed) => ({ ...feed })),
    rotation_order: [...config.rotation_order],
    connectors: { telegram: { ...config.connectors.telegram } },
  };
}

function lines(value: string): string[] {
  return value
    .split("\n")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

// Normalized match key for the rss_feeds rebuild (plan 021): strips the
// hash and a single trailing slash so a cosmetic edit (trailing "/", a
// fragment) still matches the old entry and keeps its source/category.
// A substantive change (different path/host) still resets metadata —
// correct, it IS a different feed. Limitation: this can't distinguish
// "same feed, meaningfully re-hosted" from "different feed" — that's
// out of scope without a source/category editing UI (see plan notes).
function feedKey(url: string): string {
  try {
    const u = new URL(url);
    u.hash = "";
    return u.href.replace(/\/$/, "");
  } catch {
    return url.trim();
  }
}

function errorList(error: unknown): string[] {
  if (Array.isArray(error)) {
    return error.map(String);
  }
  return [typeof error === "string" ? error : "settings could not be saved"];
}

function SettingsGroup({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="settings-group">
      <div className="group-heading">
        <h2>{title}</h2>
        {description ? <p>{description}</p> : null}
      </div>
      <div className="group-controls">{children}</div>
    </section>
  );
}

function Switch({
  id,
  label,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="switch-control" htmlFor={id}>
      <input
        id={id}
        type="checkbox"
        aria-label={label}
        checked={checked}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span className="switch" aria-hidden="true" />
    </label>
  );
}

function ControlCopy({ htmlFor, name, help }: { htmlFor: string; name: string; help: string }) {
  return (
    <div className="control-copy">
      <label className="control-name" htmlFor={htmlFor}>
        {name}
      </label>
      <span className="control-help">{help}</span>
    </div>
  );
}

function NumberControl({
  id,
  name,
  help,
  value,
  min,
  max,
  unit,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  onChange: (value: number) => void;
}) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <div className="number-field">
        <input
          id={id}
          type="number"
          min={min}
          max={max}
          value={value}
          inputMode="numeric"
          onChange={(event) => onChange(Number(event.currentTarget.value))}
        />
        {unit ? <span className="unit">{unit}</span> : null}
      </div>
    </div>
  );
}

function ToggleControl({
  id,
  name,
  help,
  label,
  checked,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <Switch id={id} label={label} checked={checked} onChange={onChange} />
    </div>
  );
}

const PRIORITY_LABELS: Record<PriorityLevel, string> = {
  low: "Low",
  medium: "Medium",
  high: "High",
};
const PRIORITY_LEVELS: PriorityLevel[] = ["low", "medium", "high"];

function PriorityToggle({
  id,
  name,
  help,
  value,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: PriorityLevel;
  onChange: (value: PriorityLevel) => void;
}) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={id} name={name} help={help} />
      {/* biome-ignore lint/a11y/useSemanticElements: role="group" is queried by tests (findByRole("group")) and styled via .priority-toggle; migrating to <fieldset> is a separate a11y-markup task with visual-regression risk, not a mechanical lint fix. */}
      <div className="priority-toggle" id={id} role="group" aria-label={name}>
        {PRIORITY_LEVELS.map((level) => (
          <button
            key={level}
            type="button"
            className={`priority-toggle-button${value === level ? " is-selected" : ""}`}
            aria-pressed={value === level}
            onClick={() => onChange(level)}
          >
            {PRIORITY_LABELS[level]}
          </button>
        ))}
      </div>
    </div>
  );
}

const UNITS_LABELS: Record<Units, string> = {
  celsius: "Celsius",
  fahrenheit: "Fahrenheit",
};
const UNITS_OPTIONS: Units[] = ["celsius", "fahrenheit"];

// plan 040 Part B: the two-button sibling of PriorityToggle for weather
// display units — same role="group" button-row shape keyed off
// `value === unit`.
function UnitsToggle({
  id,
  name,
  help,
  value,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: Units;
  onChange: (value: Units) => void;
}) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={id} name={name} help={help} />
      {/* biome-ignore lint/a11y/useSemanticElements: same role="group" shape as PriorityToggle (styled via .priority-toggle); migrating to <fieldset> is a separate a11y-markup task with visual-regression risk, not a mechanical lint fix. */}
      <div className="priority-toggle" id={id} role="group" aria-label={name}>
        {UNITS_OPTIONS.map((unit) => (
          <button
            key={unit}
            type="button"
            className={`priority-toggle-button${value === unit ? " is-selected" : ""}`}
            aria-pressed={value === unit}
            onClick={() => onChange(unit)}
          >
            {UNITS_LABELS[unit]}
          </button>
        ))}
      </div>
    </div>
  );
}

const SOURCE_LABELS: Record<SourceKind, string> = {
  football: "Football",
  cmux: "Cmux (agent relay)",
  manual: "Manual / CLI push",
  news: "News",
  weather: "Weather",
};

function RotationOrderList({
  order,
  onChange,
}: {
  order: SourceKind[];
  onChange: (order: SourceKind[]) => void;
}) {
  function move(index: number, delta: number) {
    const next = [...order];
    const target = index + delta;
    [next[index], next[target]] = [next[target], next[index]];
    onChange(next);
  }

  return (
    // biome-ignore lint/a11y/useSemanticElements: role="list" is queried by tests and styled via .rotation-order-list; migrating to <ul>/<ol> is a separate a11y-markup task with visual-regression risk, not a mechanical lint fix.
    <div className="rotation-order-list" role="list" aria-label="Rotation order">
      {order.map((source, index) => (
        // biome-ignore lint/a11y/useSemanticElements: role="listitem" is queried by tests (getAllByRole("listitem")) and styled via .rotation-order-row; migrating to <li> is a separate a11y-markup task, not a mechanical lint fix.
        <div className="rotation-order-row" role="listitem" key={source}>
          <span className="rotation-order-rank">{index + 1}</span>
          <span className="rotation-order-name">{SOURCE_LABELS[source]}</span>
          <div className="rotation-order-controls">
            <button
              type="button"
              className="rotation-order-button"
              aria-label={`Move ${SOURCE_LABELS[source]} earlier`}
              disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              ▲
            </button>
            <button
              type="button"
              className="rotation-order-button"
              aria-label={`Move ${SOURCE_LABELS[source]} later`}
              disabled={index === order.length - 1}
              onClick={() => move(index, 1)}
            >
              ▼
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

function TextareaControl({
  id,
  name,
  help,
  value,
  caption,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: string;
  caption: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="textarea-control">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <textarea
        id={id}
        spellCheck={false}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
      <div className="field-caption">{caption}</div>
    </div>
  );
}

function ErrorPanel({ errors }: { errors: string[] }) {
  return (
    <AnimatePresence initial={false}>
      {errors.length > 0 ? (
        <motion.div
          className="error-panel"
          role="alert"
          aria-live="assertive"
          initial={{ opacity: 0, y: -3 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -3 }}
        >
          <div className="error-title">Config rejected</div>
          <ul>
            {errors.map((error) => (
              <li key={error}>{error}</li>
            ))}
          </ul>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

function SecretRow({
  field,
  id,
  label,
  placeholder,
  status,
  onSaved,
}: {
  field: SecretField;
  id: string;
  label: string;
  placeholder: string;
  status: string | null;
  onSaved: () => Promise<void>;
}) {
  const [value, setValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function saveSecret() {
    setSaving(true);
    setError(null);
    try {
      await invoke("set_secret", { field, value });
      setValue("");
      await onSaved();
    } catch (reason) {
      setError(typeof reason === "string" ? reason : "secret could not be saved");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="secret-row">
      <div className="secret-meta">
        <label className="secret-label" htmlFor={id}>
          {label}
        </label>
        <span className={`status-chip${status ? " is-set" : ""}`} aria-live="polite">
          {status ?? "unset"}
        </span>
      </div>
      <div className="secret-controls">
        <input
          id={id}
          className="secret-input"
          type="password"
          autoComplete="new-password"
          placeholder={placeholder}
          value={value}
          onChange={(event) => setValue(event.currentTarget.value)}
        />
        <button
          className="secondary-button secret-save"
          type="button"
          aria-label={`Save ${label}`}
          disabled={saving || value.trim().length === 0}
          onClick={() => void saveSecret()}
        >
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
      {error ? (
        <div className="secret-error" role="alert">
          {error}
        </div>
      ) : null}
    </div>
  );
}

function GeneralSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  return (
    <div className="section-stack">
      <SettingsGroup title="Engine">
        <ToggleControl
          id="start-paused"
          name="Start paused"
          help="Launch with promotion paused. The tray will read Resume."
          label="Start paused"
          checked={config.start_paused}
          onChange={(start_paused) => patchConfig({ start_paused })}
        />
        <ToggleControl
          id="hide-when-idle"
          name="Hide overlay when idle"
          help="Resting state shows the bare notch instead of the clock and status dots. Notifications, rotation, and shortcuts are unaffected. Applies after Save & Relaunch."
          label="Hide overlay when idle"
          checked={config.resting_state === "notch"}
          onChange={(hideWhenIdle) =>
            patchConfig({ resting_state: hideWhenIdle ? "notch" : "rail" })
          }
        />
        <NumberControl
          id="port"
          name="Listener port"
          help="Local loopback port used by the notchtap CLI."
          value={config.port}
          min={1024}
          max={65535}
          unit="PORT"
          onChange={(port) => patchConfig({ port })}
        />
        <TestButtonRow
          name="Test notification"
          help="Send a manual push to the overlay."
          source="manual"
        />
      </SettingsGroup>

      <SettingsGroup
        title="Rotation and priority"
        description="Waiting items promote high → medium → low. Priority chooses the next turn; it never interrupts the visible item."
      >
        <NumberControl
          id="default-ttl"
          name="Rotation seconds"
          help="How long a one-shot notification occupies the slot."
          value={config.default_ttl}
          min={1}
          max={3600}
          unit="SEC"
          onChange={(default_ttl) => patchConfig({ default_ttl })}
        />
        <NumberControl
          id="queue-cap"
          name="Queue cap per priority tier"
          help="Maximum waiting items kept independently in each priority tier."
          value={config.max_queued_per_tier}
          min={1}
          max={1000}
          unit="ITEMS"
          onChange={(max_queued_per_tier) => patchConfig({ max_queued_per_tier })}
        />
        <PriorityToggle
          id="manual-default-priority"
          name="Manual push priority"
          help="Fallback for a CLI push that doesn't set its own priority."
          value={config.manual_default_priority}
          onChange={(manual_default_priority) => patchConfig({ manual_default_priority })}
        />
      </SettingsGroup>

      <SettingsGroup
        title="Rotation order"
        description="Same-tier tie-break, checked before arrival order. Priority still decides which tier goes first."
      >
        <RotationOrderList
          order={config.rotation_order}
          onChange={(rotation_order) => patchConfig({ rotation_order })}
        />
      </SettingsGroup>
    </div>
  );
}

function FootballSection({
  config,
  leaguesText,
  patchConfig,
  setLeaguesText,
}: {
  config: Config;
  leaguesText: string;
  patchConfig: (patch: Partial<Config>) => void;
  setLeaguesText: (value: string) => void;
}) {
  return (
    <SettingsGroup title="Score polling">
      <ToggleControl
        id="espn-enabled"
        name="ESPN scores"
        help="Poll watched leagues for score and match-state changes."
        label="Enable ESPN scores"
        checked={config.espn_enabled}
        onChange={(espn_enabled) => patchConfig({ espn_enabled })}
      />
      <TextareaControl
        id="espn-leagues"
        name="Leagues"
        help="Use one ESPN league code per line."
        value={leaguesText}
        caption="one league code per line"
        onChange={setLeaguesText}
      />
      <NumberControl
        id="espn-poll-secs"
        name="Poll interval"
        help="How often enabled leagues are checked."
        value={config.espn_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(espn_poll_secs) => patchConfig({ espn_poll_secs })}
      />
      <NumberControl
        id="espn-ttl-secs"
        name="Rotation seconds"
        help="How long a score card occupies the slot once shown."
        value={config.espn_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(espn_ttl_secs) => patchConfig({ espn_ttl_secs })}
      />
      <ToggleControl
        id="espn-live-card"
        name="Live match card"
        help="Show one live match as a single updating card instead of a burst of one-shot cards."
        label="Consolidate live match updates"
        checked={config.espn_live_card}
        onChange={(espn_live_card) => patchConfig({ espn_live_card })}
      />
      <ToggleControl
        id="espn-rich-events"
        name="Richer match events"
        help="Poll for fouls, offsides, VAR checks, and substitutions in addition to goals and cards. Heavier polling — opt in per match."
        label="Show richer match events"
        checked={config.espn_rich_events}
        onChange={(espn_rich_events) => patchConfig({ espn_rich_events })}
      />
      <PriorityToggle
        id="espn-priority"
        name="Priority"
        help="Which tier a waiting score/match-state update promotes in."
        value={config.espn_priority}
        onChange={(espn_priority) => patchConfig({ espn_priority })}
      />
      <TestButtonRow
        name="Test football notification"
        help="Send a one-off football notification to the overlay."
        source="football"
      />
    </SettingsGroup>
  );
}

function NewsSection({
  config,
  feedsText,
  patchConfig,
  setFeedsText,
}: {
  config: Config;
  feedsText: string;
  patchConfig: (patch: Partial<Config>) => void;
  setFeedsText: (value: string) => void;
}) {
  return (
    <SettingsGroup title="RSS polling">
      <ToggleControl
        id="rss-enabled"
        name="RSS news"
        help="Poll configured feeds and rotate fresh headlines through the slot."
        label="Enable RSS news"
        checked={config.rss_enabled}
        onChange={(rss_enabled) => patchConfig({ rss_enabled })}
      />
      <TextareaControl
        id="rss-feeds"
        name="Feeds"
        help="Use one complete HTTP(S) feed URL per line."
        value={feedsText}
        caption="one feed URL per line"
        onChange={setFeedsText}
      />
      <NumberControl
        id="rss-poll-secs"
        name="Poll interval"
        help="How often configured feeds are checked."
        value={config.rss_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(rss_poll_secs) => patchConfig({ rss_poll_secs })}
      />
      <NumberControl
        id="rss-ttl-secs"
        name="Headline rotation"
        help="How long each headline occupies the slot."
        value={config.rss_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(rss_ttl_secs) => patchConfig({ rss_ttl_secs })}
      />
      <NumberControl
        id="rss-max-per-poll"
        name="Maximum per poll"
        help="New headlines accepted from a single poll pass."
        value={config.rss_max_per_poll}
        min={1}
        max={100}
        unit="ITEMS"
        onChange={(rss_max_per_poll) => patchConfig({ rss_max_per_poll })}
      />
      <PriorityToggle
        id="rss-priority"
        name="Priority"
        help="Which tier a waiting headline promotes in."
        value={config.rss_priority}
        onChange={(rss_priority) => patchConfig({ rss_priority })}
      />
      <TestButtonRow
        name="Test news notification"
        help="Send a one-off news headline to the overlay."
        source="news"
      />
    </SettingsGroup>
  );
}

function CmuxSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  return (
    <SettingsGroup
      title="Cmux relay"
      description="Cmux's notification command already calls the notchtap CLI, which auto-detects relayed pushes through CMUX_NOTIFICATION_BODY. Nothing needs enabling here; these controls set how relayed notifications are promoted and rotated."
    >
      <PriorityToggle
        id="cmux-priority"
        name="Priority"
        help="Which tier a waiting cmux-relayed notification promotes in. Only applies when the request omits its own priority — cmux's built-in notification-command setting currently always passes --priority high explicitly, which overrides this. Drop that flag from cmux's own settings (not this app) to let this control take effect."
        value={config.cmux_priority}
        onChange={(cmux_priority) => patchConfig({ cmux_priority })}
      />
      <NumberControl
        id="cmux-ttl-secs"
        name="Rotation seconds"
        help="How long a cmux-relayed notification occupies the slot once shown."
        value={config.cmux_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(cmux_ttl_secs) => patchConfig({ cmux_ttl_secs })}
      />
      <TestButtonRow
        name="Test cmux notification"
        help="Send a one-off cmux notification to the overlay."
        source="cmux"
      />
    </SettingsGroup>
  );
}

function WeatherSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  return (
    <SettingsGroup
      title="Weather"
      description="Keyless Open-Meteo polling — set your coordinates once. The idle rail shows current conditions; rain and temperature thresholds send alert cards."
    >
      <ToggleControl
        id="weather-enabled"
        name="Weather"
        help="Poll Open-Meteo for current conditions and threshold alerts."
        label="Enable weather"
        checked={config.weather_enabled}
        onChange={(weather_enabled) => patchConfig({ weather_enabled })}
      />
      <NumberControl
        id="weather-lat"
        name="Latitude"
        help="Decimal degrees, e.g. 12.97 for Bangalore."
        value={config.weather_lat}
        min={-90}
        max={90}
        onChange={(weather_lat) => patchConfig({ weather_lat })}
      />
      <NumberControl
        id="weather-lon"
        name="Longitude"
        help="Decimal degrees, e.g. 77.59 for Bangalore."
        value={config.weather_lon}
        min={-180}
        max={180}
        onChange={(weather_lon) => patchConfig({ weather_lon })}
      />
      <UnitsToggle
        id="weather-units"
        name="Units"
        help="Display units for the idle chip. Alert thresholds below are always in Celsius."
        value={config.weather_units}
        onChange={(weather_units) => patchConfig({ weather_units })}
      />
      <NumberControl
        id="weather-poll-secs"
        name="Poll interval"
        help="How often conditions are refreshed."
        value={config.weather_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(weather_poll_secs) => patchConfig({ weather_poll_secs })}
      />
      <NumberControl
        id="weather-rain-threshold-pct"
        name="Rain threshold"
        help="Alert when the chance of rain reaches this."
        value={config.weather_rain_threshold_pct}
        min={0}
        max={100}
        unit="%"
        onChange={(weather_rain_threshold_pct) => patchConfig({ weather_rain_threshold_pct })}
      />
      <NumberControl
        id="weather-rain-lookahead-mins"
        name="Rain lookahead"
        help="How far ahead the rain check looks."
        value={config.weather_rain_lookahead_mins}
        min={5}
        max={120}
        unit="MIN"
        onChange={(weather_rain_lookahead_mins) => patchConfig({ weather_rain_lookahead_mins })}
      />
      <NumberControl
        id="weather-temp-hot-c"
        name="Hot threshold"
        help="Alert when the temperature reaches this, always in Celsius."
        value={config.weather_temp_hot_c}
        min={-50}
        max={60}
        unit="°C"
        onChange={(weather_temp_hot_c) => patchConfig({ weather_temp_hot_c })}
      />
      <NumberControl
        id="weather-temp-cold-c"
        name="Cold threshold"
        help="Alert when the temperature drops to this, always in Celsius."
        value={config.weather_temp_cold_c}
        min={-50}
        max={60}
        unit="°C"
        onChange={(weather_temp_cold_c) => patchConfig({ weather_temp_cold_c })}
      />
      <PriorityToggle
        id="weather-priority"
        name="Priority"
        help="Which tier a waiting weather alert promotes in."
        value={config.weather_priority}
        onChange={(weather_priority) => patchConfig({ weather_priority })}
      />
      <TestButtonRow
        name="Test weather notification"
        help="Send a one-off weather alert to the overlay."
        source="weather"
      />
    </SettingsGroup>
  );
}

function formatDeliveryAgo(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return `${Math.floor(hours / 24)} d ago`;
}

// Read-only delivery-health line for the Telegram connector (plan 076) —
// advisory, like the SecretStatus fetch: a failed/unknown fetch renders
// nothing rather than an error.
function ConnectorHealthLine({ health }: { health: ConnectorHealthDto | null }) {
  if (!health) return null;
  let text: string;
  if (health.consecutiveFailures > 0) {
    text = `${health.consecutiveFailures} consecutive failure${health.consecutiveFailures === 1 ? "" : "s"} — check your bot token`;
  } else if (health.lastSuccessMs !== null) {
    text = `Last delivered: ${formatDeliveryAgo(health.lastSuccessMs)}`;
  } else {
    text = "No deliveries yet.";
  }
  return <div className="relaunch-note">{text}</div>;
}

function ConnectorsSection({
  config,
  secretStatus,
  connectorHealth,
  patchConfig,
  refreshSecretStatus,
}: {
  config: Config;
  secretStatus: SecretStatus | null;
  connectorHealth: ConnectorHealthDto | null;
  patchConfig: (patch: Partial<Config>) => void;
  refreshSecretStatus: () => Promise<void>;
}) {
  return (
    <div className="section-stack">
      <SettingsGroup title="Telegram">
        <ToggleControl
          id="telegram-enabled"
          name="Enable connector"
          help="Forward every accepted event after Save & Relaunch."
          label="Enable Telegram connector"
          checked={config.connectors.telegram.enabled}
          onChange={(enabled) => patchConfig({ connectors: { telegram: { enabled } } })}
        />
        <div className="relaunch-note">Config change · applied after relaunch</div>
        <ConnectorHealthLine health={connectorHealth} />
      </SettingsGroup>

      <SettingsGroup
        title="Write-only keys"
        description="Values never come back across IPC. Status reveals only whether a value is set and, when safe, its masked suffix."
      >
        {secretRows.map((row) => (
          <SecretRow
            key={row.field}
            {...row}
            status={secretStatus?.[row.field] ?? null}
            onSaved={refreshSecretStatus}
          />
        ))}
      </SettingsGroup>
    </div>
  );
}

function ShortcutsSection() {
  return (
    <SettingsGroup
      title="Global shortcuts"
      description="These work while notchtap is running, regardless of which app has focus."
    >
      {/* biome-ignore lint/a11y/useSemanticElements: role="table" is queried by tests (getByRole("table")) and styled via .shortcut-table; migrating to a semantic table element is a separate a11y-markup task with visual-regression risk, not a mechanical lint fix. */}
      <div className="shortcut-table" role="table" aria-label="Keyboard shortcuts">
        {shortcuts.map((shortcut) => (
          // biome-ignore lint/a11y/useFocusableInteractive: display-only cheatsheet row — not meant to be keyboard-focusable; the interactive-role markup here is part of the role="table" cheatsheet, a separate a11y-markup task.
          // biome-ignore lint/a11y/useSemanticElements: role="row" is part of the role="table" cheatsheet markup styled via .shortcut-row; migrating to <tr> is a separate a11y-markup task, not a mechanical lint fix.
          <div className="shortcut-row" role="row" key={shortcut.action}>
            {/* biome-ignore lint/a11y/noInteractiveElementToNoninteractiveRole: the kbd is display-only (shortcut keys), part of the role="table" cheatsheet markup — separate a11y-markup task. */}
            {/* biome-ignore lint/a11y/useSemanticElements: role="cell" is part of the role="table" cheatsheet markup; migrating to <td> is a separate a11y-markup task, not a mechanical lint fix. */}
            <kbd role="cell">{shortcut.keys}</kbd>
            {/* biome-ignore lint/a11y/useSemanticElements: role="cell" is part of the role="table" cheatsheet markup; migrating to <td> is a separate a11y-markup task, not a mechanical lint fix. */}
            <span className="shortcut-action" role="cell">
              {shortcut.action}
            </span>
            {/* biome-ignore lint/a11y/useSemanticElements: role="cell" is part of the role="table" cheatsheet markup; migrating to <td> is a separate a11y-markup task, not a mechanical lint fix. */}
            <span className={`shortcut-status ${shortcut.status}`} role="cell">
              {shortcut.status === "active" ? "active" : "planned · not implemented"}
            </span>
          </div>
        ))}
      </div>
    </SettingsGroup>
  );
}

type TestSource = "football" | "news" | "cmux" | "manual" | "weather";

// plan 077: read-only tail of the active log file. Fetched on section-open
// (this component mounts only while the Diagnostics section is active), not
// on app load — the same advisory, isolated-from-panel-load pattern as
// get_default_config / get_connector_health. No live tail; the Refresh
// button re-invokes manually.
function DiagnosticsSection() {
  const [logLines, setLogLines] = useState<string[] | null>(null);

  function refresh() {
    invoke<string[]>("get_recent_log_lines")
      .then((fetched) => setLogLines(fetched))
      .catch(() => {
        // advisory fetch — a failed read leaves the previous lines shown
      });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_recent_log_lines on every render.
  useEffect(() => {
    refresh();
  }, []);

  const logText =
    logLines === null
      ? "Loading…"
      : logLines.length === 0
        ? "No log lines yet."
        : logLines.join("\n");

  return (
    <SettingsGroup
      title="Recent log lines"
      description="The last 200 lines of ~/Library/Logs/notchtap/notchtap.log. Read-only; rotated backups are available via Console.app."
    >
      <pre
        style={{
          margin: 0,
          padding: "12px",
          maxHeight: "320px",
          overflow: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
          fontSize: "11px",
          lineHeight: 1.5,
          background: "rgba(0, 0, 0, 0.25)",
          borderRadius: "8px",
          userSelect: "text",
        }}
      >
        {logText}
      </pre>
      <div className="control-row">
        <ControlCopy
          htmlFor="refresh-log-lines"
          name="Refresh"
          help="Re-read the log file. New lines appear as the app writes them."
        />
        <button
          id="refresh-log-lines"
          type="button"
          className="secondary-button test-button"
          onClick={refresh}
        >
          Refresh
        </button>
      </div>
    </SettingsGroup>
  );
}

function TestButton({ source }: { source: TestSource }) {
  async function send() {
    try {
      await invoke("send_test_notification", { source });
    } catch (reason) {
      // Errors are fire-and-forget from the settings panel; the overlay owns the real queue state.
      console.error("send_test_notification failed:", reason);
    }
  }

  return (
    <button type="button" className="secondary-button test-button" onClick={() => void send()}>
      Send test notification
    </button>
  );
}

function TestButtonRow({ name, help, source }: { name: string; help: string; source: TestSource }) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={name.replace(/\s+/g, "-")} name={name} help={help} />
      <TestButton source={source} />
    </div>
  );
}

type SegmentedOption = { label: string; value: number };

function SegmentedControl({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: SegmentedOption[];
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="control-row">
      <div className="control-copy">
        {/* biome-ignore lint/a11y/noLabelWithoutControl: this label names a button group, not a form control — the group below carries aria-label={label}; rewiring htmlFor/id is a separate a11y-markup task. */}
        <label className="control-name">{label}</label>
      </div>
      {/* biome-ignore lint/a11y/useSemanticElements: role="group" is styled via .segmented-control; migrating to <fieldset> is a separate a11y-markup task with visual-regression risk, not a mechanical lint fix. */}
      <div className="segmented-control" role="group" aria-label={label}>
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className={`segmented-control-button${value === option.value ? " is-selected" : ""}`}
            aria-pressed={value === option.value}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}

type ShowingSlotState = Extract<SlotState, { state: "showing" }>;

const PREVIEW_SAMPLES: ReadonlyArray<{
  label: string;
  slot: ShowingSlotState;
}> = [
  {
    label: "Goal (High priority, football)",
    slot: {
      state: "showing",
      id: "preview-goal",
      title: "GOAL",
      body: "Arsenal 2-0",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
    },
  },
  {
    label: "Red card (High priority, football)",
    slot: {
      state: "showing",
      id: "preview-red-card",
      title: "Red Card",
      body: "Chelsea down to 10",
      eventType: "match_state",
      priority: "high",
      signal: "red_card",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 1,
      ttlMs: 8000,
      remainingMs: 5000,
    },
  },
  {
    label: "Generic alert (High priority, cmux)",
    slot: {
      state: "showing",
      id: "preview-cmux",
      title: "Agent needs input",
      body: "run `git push origin master`?",
      eventType: "generic",
      priority: "high",
      signal: "generic",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      // preview samples keep subtitle/details empty (plan 035): the render
      // path for populated cells is exercised in StatusRailCard.test.tsx.
      subtitle: null,
      details: [],
      queueTotal: 3,
      queueDone: 2,
      ttlMs: 8000,
      remainingMs: 5000,
    },
  },
  {
    label: "News headline (Low priority)",
    slot: {
      state: "showing",
      id: "preview-news",
      title: "Parliament passes the landmark digital rights bill",
      body: "The measure passed after a late-night vote.",
      eventType: "news_item",
      priority: "low",
      signal: "generic",
      expanded: true,
      source: "NDTV",
      category: "politics",
      publishedAtMs: null,
      link: "https://example.com/digital-rights",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 5000,
    },
  },
];

function AppearanceSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  const initial = config.appearance;
  const [scale, setScale] = useState(initial.card_scale);
  const [radius, setRadius] = useState(initial.card_radius);
  const [opacity, setOpacity] = useState(initial.card_opacity);

  function updateAppearance(partial: Partial<AppearanceConfig>) {
    const next = {
      ...config.appearance,
      card_scale: partial.card_scale ?? scale,
      card_radius: partial.card_radius ?? radius,
      card_opacity: partial.card_opacity ?? opacity,
    };
    setScale(next.card_scale);
    setRadius(next.card_radius);
    setOpacity(next.card_opacity);
    invoke("set_appearance", {
      scale: next.card_scale,
      radius: next.card_radius,
      opacity: next.card_opacity,
    }).catch((reason) => {
      console.error("set_appearance failed:", reason);
    });
    patchConfig({ appearance: next });
  }

  function updateScale(next: number) {
    updateAppearance({ card_scale: next });
  }

  function updateRadius(next: number) {
    updateAppearance({ card_radius: next });
  }

  function updateOpacity(next: number) {
    updateAppearance({ card_opacity: next });
  }

  const previewStyle: CSSProperties = {
    "--card-scale": scale,
    "--card-radius": `${radius}px`,
    "--card-opacity": opacity,
  } as CSSProperties;

  return (
    <div className="section-stack">
      <SettingsGroup
        title="Card shape"
        description="Adjust the overlay card size, corner radius, and opacity. Changes apply immediately."
      >
        <SegmentedControl
          label="Scale"
          options={[
            { label: "Small", value: 0.85 },
            { label: "Medium", value: 1.0 },
            { label: "Large", value: 1.15 },
          ]}
          value={scale}
          onChange={updateScale}
        />
        <SegmentedControl
          label="Radius"
          options={[
            { label: "Square", value: 0 },
            { label: "Soft", value: 8 },
            { label: "Round", value: 16 },
          ]}
          value={radius}
          onChange={updateRadius}
        />
        <SegmentedControl
          label="Opacity"
          options={[
            { label: "Glass", value: 0.7 },
            { label: "Default", value: 0.9 },
            { label: "Solid", value: 1.0 },
          ]}
          value={opacity}
          onChange={updateOpacity}
        />
        <TestButtonRow
          name="Live check"
          help="Send a one-off manual notification to the overlay."
          source="manual"
        />
      </SettingsGroup>

      <SettingsGroup
        title="Overlay animations"
        description="These are the built-in card styles the overlay renders. The preview reflects the shape settings above."
      >
        <div className="appearance-preview" style={previewStyle}>
          {PREVIEW_SAMPLES.map(({ label, slot }) => (
            <div className="preview-row" key={slot.id}>
              <div className="preview-label">{label}</div>
              <div className="preview-stage">
                <StatusRailCard slot={slot} />
              </div>
            </div>
          ))}
        </div>
      </SettingsGroup>
    </div>
  );
}

export function SettingsApp() {
  const [activeSection, setActiveSection] = useState<SectionId>("general");
  const [config, setConfig] = useState<Config | null>(null);
  const [lastLoadedConfig, setLastLoadedConfig] = useState<Config | null>(null);
  const [defaults, setDefaults] = useState<Config | null>(null);
  const [secretStatus, setSecretStatus] = useState<SecretStatus | null>(null);
  const [connectorHealth, setConnectorHealth] = useState<ConnectorHealthDto | null>(null);
  const [espnLeaguesText, setEspnLeaguesText] = useState("");
  const [rssFeedsText, setRssFeedsText] = useState("");
  const [errors, setErrors] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const [formGeneration, setFormGeneration] = useState(0);

  function applyForm(nextConfig: Config) {
    const next = copyConfig(nextConfig);
    setConfig(next);
    setEspnLeaguesText(next.espn_leagues.join("\n"));
    setRssFeedsText(next.rss_feeds.map((feed) => feed.url).join("\n"));
    setErrors([]);
    setFormGeneration((n) => n + 1);
  }

  async function refreshSecretStatus() {
    setSecretStatus(await invoke<SecretStatus>("get_secret_status"));
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only config loader — applyForm is re-created every render, so adding it would re-invoke get_config on every render.
  useEffect(() => {
    let active = true;
    Promise.all([invoke<Config>("get_config"), invoke<SecretStatus>("get_secret_status")])
      .then(([loadedConfig, loadedStatus]) => {
        if (active) {
          const loaded = copyConfig(loadedConfig);
          setLastLoadedConfig(loaded);
          applyForm(loaded);
          setSecretStatus(loadedStatus);
        }
      })
      .catch((reason: unknown) => {
        if (active) setErrors(errorList(reason));
      });
    // defaults are advisory (Reset-to-defaults only) — isolate their failure
    // so it can never block the rest of the panel from loading; the button
    // just stays disabled (see the footer's `disabled={!defaults || saving}`).
    invoke<Config>("get_default_config")
      .then((loadedDefaults) => {
        if (active) setDefaults(copyConfig(loadedDefaults));
      })
      .catch(() => {
        // leave defaults null — Reset to defaults stays disabled
      });
    return () => {
      active = false;
    };
  }, []);

  // Connector health is advisory like get_default_config — fetched on its
  // own, isolated from the critical panel load, and refreshed on a light
  // polling interval so a run of drops surfaces without a window reopen.
  // A failed or unknown fetch leaves the line hidden rather than erroring.
  useEffect(() => {
    let active = true;
    const fetchHealth = () => {
      invoke<ConnectorHealthDto>("get_connector_health")
        .then((health) => {
          if (active && health) setConnectorHealth(health);
        })
        .catch(() => {
          // leave health null — the Telegram section just shows no line
        });
    };
    fetchHealth();
    const interval = setInterval(fetchHealth, 5000);
    return () => {
      active = false;
      clearInterval(interval);
    };
  }, []);

  function patchConfig(patch: Partial<Config>) {
    setConfig((current) => (current ? { ...current, ...patch } : current));
  }

  function resetLoaded() {
    if (lastLoadedConfig) applyForm(lastLoadedConfig);
  }

  function resetDefaults() {
    if (defaults) applyForm(defaults);
  }

  async function saveConfig(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!config) return;
    const submittedConfig: Config = {
      ...config,
      espn_leagues: lines(espnLeaguesText),
      rss_feeds: lines(rssFeedsText).map((url) => {
        // Match by normalized key, but keep the url the user actually typed
        // — only source/category carry over from the old entry (plan 021:
        // "the saved value stays what the user typed").
        const match = config.rss_feeds.find((feed) => feedKey(feed.url) === feedKey(url));
        return match
          ? { url, source: match.source, category: match.category }
          : { url, source: null, category: null };
      }),
    };
    setSaving(true);
    setErrors([]);
    try {
      await invoke("save_config_and_relaunch", { config: submittedConfig });
    } catch (reason) {
      setErrors(errorList(reason));
      setSaving(false);
    }
  }

  const currentSection = sectionCopy[activeSection];

  return (
    <MotionConfig reducedMotion="user" transition={{ duration: 0.16, ease: [0.22, 1, 0.36, 1] }}>
      <main className="settings-window" aria-labelledby="section-title">
        <aside className="settings-sidebar" aria-label="Settings sections">
          <div className="sidebar-brand">
            <span className="brand-slot" aria-hidden="true" />
            <span>notchtap</span>
          </div>
          <nav className="sidebar-nav">
            {navigation.map((item) => {
              const Icon = item.icon;
              const selected = item.id === activeSection;
              return (
                <button
                  key={item.id}
                  className={`nav-item${selected ? " is-active" : ""}`}
                  type="button"
                  aria-current={selected ? "page" : undefined}
                  onClick={() => setActiveSection(item.id)}
                >
                  <Icon aria-hidden="true" />
                  <span>{item.label}</span>
                </button>
              );
            })}
          </nav>
          <div className="sidebar-meta">settings / v5</div>
        </aside>

        <div className="settings-pane">
          {config ? (
            <form
              id="settings-form"
              className="settings-form"
              noValidate
              onSubmit={(event) => void saveConfig(event)}
            >
              <header className="content-header">
                <div className="section-index">Settings / {currentSection.index}</div>
                <h1 id="section-title">{currentSection.title}</h1>
                <p>{currentSection.description}</p>
              </header>

              <ErrorPanel errors={errors} />

              <div className="section-scroll">
                <AnimatePresence mode="wait" initial={false}>
                  <motion.div
                    className="section-content"
                    key={activeSection}
                    initial={{ opacity: 0, x: 3 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: -2 }}
                  >
                    {activeSection === "general" ? (
                      <GeneralSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "football" ? (
                      <FootballSection
                        config={config}
                        leaguesText={espnLeaguesText}
                        patchConfig={patchConfig}
                        setLeaguesText={setEspnLeaguesText}
                      />
                    ) : null}
                    {activeSection === "news" ? (
                      <NewsSection
                        config={config}
                        feedsText={rssFeedsText}
                        patchConfig={patchConfig}
                        setFeedsText={setRssFeedsText}
                      />
                    ) : null}
                    {activeSection === "cmux" ? (
                      <CmuxSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "weather" ? (
                      <WeatherSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "connectors" ? (
                      <ConnectorsSection
                        config={config}
                        secretStatus={secretStatus}
                        connectorHealth={connectorHealth}
                        patchConfig={patchConfig}
                        refreshSecretStatus={refreshSecretStatus}
                      />
                    ) : null}
                    {activeSection === "shortcuts" ? <ShortcutsSection /> : null}
                    {activeSection === "diagnostics" ? <DiagnosticsSection /> : null}
                    {activeSection === "appearance" ? (
                      // keyed on formGeneration so Reset/Reset-to-defaults remounts the
                      // section — its controls seed local state from config.appearance at
                      // mount and would otherwise show stale values (plan 027)
                      <AppearanceSection
                        key={formGeneration}
                        config={config}
                        patchConfig={patchConfig}
                      />
                    ) : null}
                  </motion.div>
                </AnimatePresence>
              </div>
            </form>
          ) : (
            <div className="loading-state" role="status">
              Loading settings…
            </div>
          )}

          <footer className="settings-footer">
            <button
              className="primary-button"
              type="submit"
              form="settings-form"
              disabled={!config || saving}
            >
              {saving ? "Relaunching…" : "Save & Relaunch"}
            </button>
            <button
              className="footer-button"
              type="button"
              disabled={!lastLoadedConfig || saving}
              onClick={resetLoaded}
            >
              Reset
            </button>
            <button
              className="footer-button"
              type="button"
              disabled={!defaults || saving}
              onClick={resetDefaults}
            >
              Reset to defaults
            </button>
          </footer>
        </div>
      </main>
    </MotionConfig>
  );
}
