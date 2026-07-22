import { invoke } from "@tauri-apps/api/core";
import {
  ChevronDown,
  ChevronUp,
  CloudSun,
  Command,
  History,
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
import {
  type CSSProperties,
  type FormEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch as UiSwitch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { StatusRailCard } from "../components/StatusRailCard";
import { PREVIEW_SAMPLES } from "./previewFixtures";

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
  | "diagnostics"
  | "history";

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
  { id: "history", label: "History", icon: History },
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
  history: {
    index: "10",
    title: "History",
    description: "Review and clear recorded past notifications.",
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

// Shared visible-outcome mechanism (plan 108). Every operation that can
// silently fail — send-test, appearance hot-apply, history read/clear,
// diagnostics read, connector-health read, defaults fetch — reports
// through one instance of this hook, rendered through the ActionStatus
// component below. Two knobs are per-ATTEMPT, not per-component: `announce`
// (aria-live is for user-initiated attempts only — a passive mount read or
// a background poll must never speak up on its own) and `showPending`
// (high-frequency actions like a slider drag should never flicker a
// "working" state). `error` is sticky until the next attempt settles it;
// `ok` auto-clears.
type ActionState = "idle" | "pending" | "ok" | "error";

interface ActionStatusValue {
  state: ActionState;
  message?: string;
  announce: boolean;
}

interface RunOptions {
  /** true only for a user-initiated attempt whose result should be announced via aria-live. */
  announce: boolean;
  /** message to show (and, if announce, speak) on success; omit to skip the ok phase entirely — a silent success. */
  okMessage?: string;
  /** ms before an ok status clears back to idle. */
  okClearMs?: number;
  /** whether to surface a pending status at all while the action is in flight (default true). */
  showPending?: boolean;
  /** derive the user-facing message from the rejection reason; defaults to describeActionError. */
  errorMessage?: (reason: unknown) => string;
}

const DEFAULT_OK_CLEAR_MS = 2500;

function describeActionError(reason: unknown): string {
  if (Array.isArray(reason)) return reason.map(String).join(", ");
  if (typeof reason === "string") return reason;
  return "Something went wrong";
}

// `label` is optional and purely a debug/test seam: a genuine state
// transition (not a deduped repeat) logs once via console.debug. This is
// what makes the connector-health poll's transition-only behavior
// observable in tests without inventing a second render-tracking
// mechanism — see the done criteria in plan 108.
function useActionStatus(label?: string) {
  const [status, setStatus] = useState<ActionStatusValue>({ state: "idle", announce: false });
  const clearTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (clearTimerRef.current !== null) window.clearTimeout(clearTimerRef.current);
    };
  }, []);

  function clearOkTimer() {
    if (clearTimerRef.current !== null) {
      window.clearTimeout(clearTimerRef.current);
      clearTimerRef.current = null;
    }
  }

  function applyStatus(next: ActionStatusValue) {
    setStatus((prev) => {
      // Dedup: an identical status is a no-op, not a new render — this is
      // what makes repeated identical poll failures collapse to a single
      // transition, and steady-state success polling stay silent.
      if (
        prev.state === next.state &&
        prev.message === next.message &&
        prev.announce === next.announce
      ) {
        return prev;
      }
      if (label) {
        console.debug(`[action-status:${label}]`, prev.state, "->", next.state);
      }
      return next;
    });
  }

  async function run<T>(action: () => Promise<T>, options: RunOptions): Promise<T | undefined> {
    const {
      announce,
      okMessage,
      okClearMs = DEFAULT_OK_CLEAR_MS,
      showPending = true,
      errorMessage,
    } = options;
    clearOkTimer();
    if (showPending) applyStatus({ state: "pending", announce });
    try {
      const result = await action();
      if (okMessage) {
        applyStatus({ state: "ok", message: okMessage, announce });
        clearTimerRef.current = window.setTimeout(() => {
          applyStatus({ state: "idle", announce: false });
        }, okClearMs);
      } else {
        applyStatus({ state: "idle", announce: false });
      }
      return result;
    } catch (reason) {
      const message = errorMessage ? errorMessage(reason) : describeActionError(reason);
      applyStatus({ state: "error", message, announce });
      return undefined;
    }
  }

  return { status, run };
}

// Renders the current ActionStatus. `announce` on the status value (set
// per-attempt by the caller of `run`, not hardcoded per component) decides
// whether this instance carries aria-live — never on a passive/pending
// render. `showPending` lets a high-frequency action (e.g. the appearance
// sliders) opt out of a "working" flicker entirely.
//
// plan 112 Step 3: markup/behavior (which element, when aria-live is
// present, dedup/announce policy) is unchanged from Plan 108 — only the
// per-state color moves to utilities, following the token table verbatim
// (pending -> text-muted-foreground, ok -> text-overlay-teal, error ->
// text-destructive). The two old ancestor-selector overrides
// (`.test-button-wrap .action-status` / `.settings-footer .action-status`,
// both just `margin-top: 0`, the wrap variant also `text-align: right`)
// have no single-element utility equivalent from inside this component, so
// callers in those two contexts pass the override through `className`;
// `cn` (clsx + tailwind-merge) resolves the conflicting `mt-*` in the
// caller's favor since it's applied last.
function ActionStatus({
  status,
  className,
  showPending = true,
}: {
  status: ActionStatusValue;
  className?: string;
  showPending?: boolean;
}) {
  if (status.state === "idle") return null;
  const stateClasses =
    status.state === "pending"
      ? "text-muted-foreground"
      : status.state === "ok"
        ? "text-overlay-teal"
        : "text-destructive";
  const classes = cn(
    "action-status",
    `is-${status.state}`,
    "mt-1.5 text-fs-secondary leading-[1.4]",
    stateClasses,
    className,
  );
  if (status.state === "pending") {
    if (!showPending) return null;
    return <div className={classes}>Working…</div>;
  }
  if (!status.message) return null;
  return status.announce ? (
    <div className={classes} aria-live="polite">
      {status.message}
    </div>
  ) : (
    <div className={classes}>{status.message}</div>
  );
}

// plan 112 Step 4 (General): shared row shell for every control kind
// (toggle, number, priority/units fieldset, the diagnostics/history
// footer-style rows). Old settings.css used an adjacent-sibling selector
// (".control-row + .control-row") so only a row PRECEDED BY another row
// got a top divider; `first-child:border-t-0` reproduces the same
// visible result (every row but the first in its group gets the
// divider) without depending on sibling order in the stylesheet. This
// single className is reused at every ".control-row" call site across
// the whole file (control-row is a shared layout idiom, not a
// per-section one) — migrated once, here, rather than duplicated at each
// of the ten sections that render one.
const CONTROL_ROW =
  "control-row grid min-h-[58px] grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-t border-border/60 py-2.5 first:border-t-0";

// plan 112 Step 4 (General): shadcn Card replaces the old
// .settings-group/.group-heading/.group-controls box. gap-0/py-0/ring-0
// strip Card's own spacing/ring defaults (they'd otherwise double up
// with the explicit padding below); the border-bottom divider between
// heading and controls is the only piece Card's own subcomponents don't
// give for free, so it's added directly on CardHeader.
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
    <Card
      // Card's own default className carries `text-sm` (14px/20px
      // line-height) — harmless for the title/description text below
      // (both set their own explicit text-fs-* size), but it's an
      // INHERITED property, so left alone it silently reaches every
      // descendant that doesn't set its own font-size, including the
      // Appearance section's Plan 111 preview subtree nested inside this
      // same Card (`.appearance-preview`/`.preview-stage`/`.card-root`
      // never declared their own font-size — they relied on inheriting
      // the browser's 16px/normal default, same as before this
      // migration). `text-base leading-[normal]` restores exactly that
      // inherited baseline so the preview subtree's computed styles stay
      // byte-identical (caught by the settings_capture.js preview-
      // equivalence harness before this fix landed; plan 115 renamed
      // this from the equivalent `text-[16px]` arbitrary onto the
      // `text-base` scale utility — 16px either way, pixel-identical).
      className="gap-0 overflow-hidden rounded-md border border-border bg-card py-0 text-base leading-[normal] ring-0"
    >
      <CardHeader
        // CardHeader's own default className carries a self-triggering
        // `[.border-b]:pb-(--card-spacing)` rule keyed on the literal
        // presence of the "border-b" token — adding it for the divider
        // below silently re-widens padding-bottom to Card's own 16px
        // spacing unit, fighting the `pb-[11px]` needed to match the old
        // `.group-heading { padding: 12px 13px 11px }`. The trailing `!`
        // forces this pb to win regardless of that rule's higher
        // selector specificity (caught by the settings_capture.js
        // preview-equivalence harness — it grew "Card shape"'s box by
        // enough to shift the Appearance preview gallery below it).
        className="gap-[5px] border-b border-border/60 px-[13px] pt-3 pb-[11px]!"
      >
        <CardTitle className="text-fs-body leading-[1.25] font-[640] text-foreground">
          {title}
        </CardTitle>
        {description ? (
          <CardDescription className="text-fs-secondary leading-[1.45] text-muted-foreground">
            {description}
          </CardDescription>
        ) : null}
      </CardHeader>
      <CardContent className="px-[13px]">{children}</CardContent>
    </Card>
  );
}

function ControlCopy({ htmlFor, name, help }: { htmlFor: string; name: string; help: string }) {
  return (
    <div className="control-copy min-w-0">
      {/* id lets a sibling <fieldset role=group> (PriorityToggle,
          UnitsToggle) point aria-labelledby back at this same visible
          text — <label for> alone doesn't associate with a fieldset,
          since fieldset isn't a "labelable" HTML element. */}
      <label
        className="control-name block text-fs-body leading-[1.3] font-[590] text-foreground"
        id={`${htmlFor}-label`}
        htmlFor={htmlFor}
      >
        {name}
      </label>
      <span className="control-help mt-[3px] block text-fs-secondary leading-[1.4] text-muted-foreground">
        {help}
      </span>
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
  step,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  /** HTML `step` attribute. Defaults to `1` (integer fields); pass
   *  `"any"` for decimal fields (e.g. latitude/longitude) so a partial
   *  value like `12.5` isn't flagged as a `stepMismatch`. */
  step?: number | "any";
  onChange: (value: number) => void;
}) {
  // Local raw-string mirror of `value` (2026-07-23 review): a plain
  // `value={value} onChange={(e) => onChange(Number(e.target.value))}`
  // pair fights the user on two fronts — `Number("")` coerces a
  // cleared field straight to `0`, and a controlled numeric `value`
  // snaps an in-progress decimal like `"12."` back to `"12"` on every
  // keystroke because `String(12) !== "12."`. Keeping the input's own
  // in-progress text in state (and only reconciling it with the
  // external `value` when that value actually changes, e.g. Reset)
  // lets the user clear-and-retype or type a trailing `.`/leading `-`
  // without the control fighting back.
  const [raw, setRaw] = useState(() => String(value));

  useEffect(() => {
    setRaw(String(value));
  }, [value]);

  return (
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={id} name={name} help={help} />
      <div className="number-field relative w-24 flex-none">
        <Input
          id={id}
          type="number"
          min={min}
          max={max}
          step={step ?? 1}
          value={raw}
          inputMode="numeric"
          onChange={(event) => {
            const next = event.currentTarget.value;
            setRaw(next);
            // Empty (clearing to retype) or a bare sign/decimal point
            // mid-entry: don't coerce to 0 and don't propagate yet —
            // leave the last-committed config value alone until the
            // input reads as a real number.
            if (next === "" || next === "-" || next === "." || next === "-.") {
              return;
            }
            const parsed = Number(next);
            if (!Number.isNaN(parsed)) {
              onChange(parsed);
            }
          }}
          onBlur={() => {
            // Leaving the field on an invalid/empty in-progress value
            // (e.g. the user cleared it and clicked away) restores the
            // last-committed value rather than leaving the box blank.
            if (raw === "" || Number.isNaN(Number(raw))) {
              setRaw(String(value));
            }
          }}
          className={cn(
            "h-[31px] rounded-sm border-input bg-input/20 text-right font-mono text-fs-body font-[650] text-foreground",
            unit ? "pr-10" : "pr-2.5",
          )}
        />
        {unit ? (
          <span className="unit pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-fs-caption font-bold tracking-[0.05em] text-muted-foreground">
            {unit}
          </span>
        ) : null}
      </div>
    </div>
  );
}

// plan 112 Step 4 (General): the bespoke checkbox+track Switch is gone —
// role="switch"/aria-checked shadcn Switch (radix-ui's real <button
// type="button" role="switch">) plus a visually-hidden shadcn Label
// (ControlCopy already renders the visible name for this row) replace
// it. This is the plan's one authorized behavioral change: `checked` on
// an HTMLInputElement becomes `aria-checked` on a native button —
// `screen.getByLabelText` still resolves it via the label[for] ->
// button-id association (button is a labelable element), so accessible
// name is unchanged; only the test assertions that read `.checked`
// needed updating (see SettingsApp.test.tsx).
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
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={id} name={name} help={help} />
      <Label htmlFor={id} className="sr-only">
        {label}
      </Label>
      <UiSwitch id={id} checked={checked} onCheckedChange={onChange} />
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
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={id} name={name} help={help} />
      <fieldset
        // plan 115: rounded-[7px] is intentionally off-scale (sits
        // between --radius-sm/6px and --radius-md/8px, no scale rung
        // matches) — left as a literal arbitrary value; snapping either
        // way would visibly shift this control's corner radius.
        className="priority-toggle grid h-[31px] w-36 min-w-0 flex-none grid-cols-3 gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]"
        id={id}
        aria-labelledby={`${id}-label`}
      >
        {/* accessible-name only — ControlCopy already renders the
            visible label for this group via the htmlFor above. */}
        <legend className="sr-only">{name}</legend>
        {PRIORITY_LEVELS.map((level) => (
          <button
            key={level}
            type="button"
            className={cn(
              // plan 115: rounded-[4px] is intentionally off-scale (no
              // --radius-* rung is 4px; --radius-sm is 6px) — left as a
              // literal arbitrary value rather than shifting the
              // visible corner radius.
              "priority-toggle-button rounded-[4px] border-0 bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:shadow-[0_0_0_2px_var(--ring)]",
              value === level &&
                "is-selected bg-accent text-foreground shadow-[var(--shadow-selected)]",
            )}
            aria-pressed={value === level}
            onClick={() => onChange(level)}
          >
            {PRIORITY_LABELS[level]}
          </button>
        ))}
      </fieldset>
    </div>
  );
}

const UNITS_LABELS: Record<Units, string> = {
  celsius: "Celsius",
  fahrenheit: "Fahrenheit",
};
const UNITS_OPTIONS: Units[] = ["celsius", "fahrenheit"];

// plan 040 Part B: the two-button sibling of PriorityToggle for weather
// display units — same fieldset/legend button-row shape keyed off
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
    <div className={CONTROL_ROW}>
      <ControlCopy htmlFor={id} name={name} help={help} />
      <fieldset
        // plan 115: rounded-[7px] is intentionally off-scale (sits
        // between --radius-sm/6px and --radius-md/8px, no scale rung
        // matches) — left as a literal arbitrary value; snapping either
        // way would visibly shift this control's corner radius.
        className="priority-toggle grid h-[31px] w-36 min-w-0 flex-none grid-cols-3 gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]"
        id={id}
        aria-labelledby={`${id}-label`}
      >
        {/* accessible-name only — ControlCopy already renders the
            visible label for this group via the htmlFor above. */}
        <legend className="sr-only">{name}</legend>
        {UNITS_OPTIONS.map((unit) => (
          <button
            key={unit}
            type="button"
            className={cn(
              // plan 115: rounded-[4px] is intentionally off-scale (no
              // --radius-* rung is 4px; --radius-sm is 6px) — left as a
              // literal arbitrary value rather than shifting the
              // visible corner radius.
              "priority-toggle-button rounded-[4px] border-0 bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:shadow-[0_0_0_2px_var(--ring)]",
              value === unit &&
                "is-selected bg-accent text-foreground shadow-[var(--shadow-selected)]",
            )}
            aria-pressed={value === unit}
            onClick={() => onChange(unit)}
          >
            {UNITS_LABELS[unit]}
          </button>
        ))}
      </fieldset>
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
    <ul
      className="rotation-order-list m-0 list-none px-0 pt-1 pb-[11px]"
      aria-label="Rotation order"
    >
      {order.map((source, index) => (
        <li
          className="rotation-order-row grid grid-cols-[16px_minmax(0,1fr)_auto] items-center gap-2.5 border-t border-border/60 py-2.5 first:border-t-0"
          key={source}
        >
          <span className="rotation-order-rank font-mono text-fs-secondary font-bold text-muted-foreground">
            {index + 1}
          </span>
          {/* still a bespoke class rather than a plain utility set — a
              deliberate test tripwire (plan 112 Step 4 explicit
              carve-out): rotationOrderRowNames() in SettingsApp.test.tsx
              locates each row's label text via
              `row.querySelector(".rotation-order-name")`. */}
          <span className="rotation-order-name min-w-0 text-fs-body font-[590] text-foreground">
            {SOURCE_LABELS[source]}
          </span>
          <div className="rotation-order-controls inline-flex flex-none gap-1">
            <Button
              type="button"
              variant="outline"
              size="icon-xs"
              className="text-muted-foreground"
              aria-label={`Move ${SOURCE_LABELS[source]} earlier`}
              disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              <ChevronUp className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon-xs"
              className="text-muted-foreground"
              aria-label={`Move ${SOURCE_LABELS[source]} later`}
              disabled={index === order.length - 1}
              onClick={() => move(index, 1)}
            >
              <ChevronDown className="size-4" />
            </Button>
          </div>
        </li>
      ))}
    </ul>
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
    // plan 112 Step 4 (Football): the shared control-row divider rhythm
    // landed in General's commit already — this section's own turn is
    // the textarea + caption styling below (Football is the first
    // section that actually renders one; News reuses the same
    // component unchanged).
    <div className="textarea-control border-t border-border/60 pt-[11px] pb-3 first:border-t-0">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <Textarea
        id={id}
        spellCheck={false}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
        className="mt-2 min-h-[73px] resize-y rounded-md border-input bg-input/20 px-2.5 py-2 font-mono text-fs-secondary font-[560] leading-[1.55] text-foreground"
      />
      <div className="field-caption mt-[5px] text-fs-caption font-bold tracking-[0.08em] text-muted-foreground uppercase">
        {caption}
      </div>
    </div>
  );
}

// plan 112 Step 3: keeps its motion.div lifecycle verbatim (AnimatePresence
// enter/exit is application state, not a CSS concern Tailwind replaces) —
// only the box styling is ported to utilities, Card-equivalent rather than
// a forced <Card> wrapper swap. border/bg follow the token table's
// "error -> text-destructive / border-destructive/40" row; bg-destructive/10
// is a decorative background-opacity composition (allowed — the restriction
// is on TEXT opacity, which would threaten contrast; this is a tinted panel
// fill, and text-destructive against it still measures ~7:1, unchanged from
// before).
function ErrorPanel({ errors }: { errors: string[] }) {
  return (
    <AnimatePresence initial={false}>
      {errors.length > 0 ? (
        <motion.div
          className="error-panel mx-4 mt-2.5 rounded-sm border border-destructive/40 bg-destructive/10 px-2.5 py-2.5 text-destructive"
          role="alert"
          aria-live="assertive"
          initial={{ opacity: 0, y: -3 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -3 }}
        >
          <div className="error-title mb-[5px] text-fs-caption font-bold tracking-[0.1em] uppercase">
            Config rejected
          </div>
          <ul className="m-0 pl-[15px] text-fs-secondary leading-[1.45]">
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
    <div className="secret-row border-t border-border/60 py-[11px] pb-3 first:border-t-0">
      <div className="secret-meta mb-[7px] flex items-center justify-between gap-2.5">
        <label
          className="secret-label block text-fs-body leading-[1.3] font-[590] text-foreground"
          htmlFor={id}
        >
          {label}
        </label>
        <Badge
          aria-live="polite"
          variant="outline"
          className={cn(
            // plan 115: rounded-[4px] is intentionally off-scale (no
            // --radius-* rung is 4px; --radius-sm is 6px) — left as a
            // literal arbitrary value rather than shifting the visible
            // corner radius.
            "status-chip h-auto flex-none rounded-[4px] border-input px-[5px] py-[3px] font-mono text-fs-caption font-bold tracking-[0.06em] text-muted-foreground uppercase",
            status && "is-set border-ring/40 bg-input/40 text-foreground",
          )}
        >
          {status ?? "unset"}
        </Badge>
      </div>
      <div className="secret-controls grid grid-cols-[minmax(0,1fr)_auto] gap-[7px]">
        <Input
          id={id}
          type="password"
          autoComplete="new-password"
          placeholder={placeholder}
          value={value}
          onChange={(event) => setValue(event.currentTarget.value)}
          className="secret-input h-[31px] rounded-sm border-input bg-input/20 font-mono text-fs-secondary font-[560] text-foreground"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          aria-label={`Save ${label}`}
          disabled={saving || value.trim().length === 0}
          onClick={() => void saveSecret()}
        >
          {saving ? "Saving…" : "Save"}
        </Button>
      </div>
      {error ? (
        <div
          className="secret-error mt-1.5 text-fs-secondary leading-[1.4] text-destructive"
          role="alert"
        >
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
        <ToggleControl
          id="history-enabled"
          name="Record notification history"
          help="Records notification content (including cmux payloads) to ~/.config/notchtap/history.jsonl. Applies after Save & Relaunch."
          label="Record notification history"
          checked={config.history_enabled}
          onChange={(history_enabled) => patchConfig({ history_enabled })}
        />
        <ToggleControl
          id="now-playing-enabled"
          name="Now playing"
          help="Show what's currently playing (Music, a browser tab, etc.) in the idle hover peek. Requires the vendored adapter installed via `just build-media-adapter` — see VENDORED.md. Applies after Save & Relaunch."
          label="Enable now playing"
          checked={config.now_playing_enabled}
          onChange={(now_playing_enabled) => patchConfig({ now_playing_enabled })}
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
        step="any"
        onChange={(weather_lat) => patchConfig({ weather_lat })}
      />
      <NumberControl
        id="weather-lon"
        name="Longitude"
        help="Decimal degrees, e.g. 77.59 for Bangalore."
        value={config.weather_lon}
        min={-180}
        max={180}
        step="any"
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
  return (
    <div className="relaunch-note mt-[-2px] mb-[11px] text-fs-caption tracking-[0.06em] text-muted-foreground uppercase">
      {text}
    </div>
  );
}

function ConnectorsSection({
  config,
  secretStatus,
  connectorHealth,
  connectorHealthStatus,
  patchConfig,
  refreshSecretStatus,
}: {
  config: Config;
  secretStatus: SecretStatus | null;
  connectorHealth: ConnectorHealthDto | null;
  connectorHealthStatus: ActionStatusValue;
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
        <div className="relaunch-note mt-[-2px] mb-[11px] text-fs-caption tracking-[0.06em] text-muted-foreground uppercase">
          Config change · applied after relaunch
        </div>
        <ConnectorHealthLine health={connectorHealth} />
        {/* Transition-only (plan 108): renders only on an ok<->failed flip,
            never aria-live — a passive setInterval poll must never chant. */}
        <ActionStatus
          status={connectorHealthStatus}
          className="connector-health-status"
          showPending={false}
        />
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

// plan 112 Step 4 (Shortcuts): the table STAYS a real native
// table/thead/tbody/th/td (Plan 109's contract, pinned by the "the
// shortcuts cheatsheet is a real <table>..." test) — only utility
// classes land on it, using
// shared-ui/playground/src/components/ui/table.tsx purely as a STYLING
// reference for which utility groups to reach for (row border/hover,
// header padding/weight), not as a component to swap in; generating or
// importing a shadcn Table primitive here would wrap the semantics in a
// non-table container div and was explicitly ruled out. `-mx-[13px]`
// bleeds the table to the Card's own edge (matching the old `.shortcut-
// table { margin: 0 -13px }`, since CardContent carries `px-[13px]`),
// and each cell's own `px-[13px]` restores the visual inset.
const SHORTCUT_CELL = "border-b border-border/60 px-[13px] py-2.5 text-left align-middle";

function ShortcutsSection() {
  return (
    <SettingsGroup
      title="Global shortcuts"
      description="These work while notchtap is running, regardless of which app has focus."
    >
      <table
        className="shortcut-table -mx-[13px] w-[calc(100%+26px)] border-collapse"
        aria-label="Keyboard shortcuts"
      >
        <thead>
          <tr>
            <th
              scope="col"
              className="px-[13px] pb-[7px] text-left font-mono text-fs-caption font-bold tracking-[0.08em] text-muted-foreground uppercase"
            >
              Keys
            </th>
            <th
              scope="col"
              className="px-[13px] pb-[7px] text-left font-mono text-fs-caption font-bold tracking-[0.08em] text-muted-foreground uppercase"
            >
              Action
            </th>
            <th
              scope="col"
              className="px-[13px] pb-[7px] text-left font-mono text-fs-caption font-bold tracking-[0.08em] text-muted-foreground uppercase"
            >
              Status
            </th>
          </tr>
        </thead>
        <tbody>
          {shortcuts.map((shortcut, index) => (
            <tr className="shortcut-row" key={shortcut.action}>
              <td className={cn(SHORTCUT_CELL, index === shortcuts.length - 1 && "border-b-0")}>
                <kbd className="inline-flex min-h-[25px] items-center justify-center rounded-[5px] border border-border bg-input/30 px-[5px] font-mono text-fs-body leading-none font-semibold text-foreground shadow-[0_1px_0_var(--border)]">
                  {shortcut.keys}
                </kbd>
              </td>
              <th
                scope="row"
                className={cn(
                  SHORTCUT_CELL,
                  "shortcut-action font-normal text-fs-secondary leading-[1.3] text-foreground",
                  index === shortcuts.length - 1 && "border-b-0",
                )}
              >
                {shortcut.action}
              </th>
              <td className={cn(SHORTCUT_CELL, index === shortcuts.length - 1 && "border-b-0")}>
                <span
                  className={cn(
                    "shortcut-status inline-block w-max rounded-[3px] border border-border/80 px-1 py-0.5 font-mono text-fs-caption font-bold tracking-[0.06em] text-muted-foreground uppercase",
                    shortcut.status === "active" && "active border-ring/40 text-foreground",
                  )}
                >
                  {shortcut.status === "active" ? "active" : "planned · not implemented"}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
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
  const { status, run } = useActionStatus("diagnostics");

  // `announce` is explicit per call, not a static prop: the mount-time read
  // below is passive (announce: false), the Refresh button's own call
  // further down is interactive (announce: true) — same operation, two
  // distinct attempt origins.
  function refresh(announce: boolean) {
    void run(
      () => invoke<string[]>("get_recent_log_lines").then((fetched) => setLogLines(fetched)),
      {
        announce,
        showPending: false,
        errorMessage: () => "Couldn't read log lines",
      },
    );
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_recent_log_lines on every render.
  useEffect(() => {
    refresh(false);
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
      {/* plan 112 Step 3: was a raw inline style={{...}} (the sole
          CSS-in-JSX color literal the Step 0 inventory found —
          rgba(0,0,0,0.25)); ported to utilities, bg-black/25 reproduces
          the same composited color without a raw hex/rgb literal in
          code. fontSize stays a literal 11px (not the fs-body token) —
          it was already decoupled from the type-scale system before
          this migration, a fixed size for the monospace log viewer. */}
      <pre className="m-0 max-h-[320px] overflow-auto rounded-lg bg-black/25 p-3 font-mono text-[11px] leading-[1.5] whitespace-pre-wrap break-all select-text">
        {logText}
      </pre>
      <ActionStatus status={status} className="diagnostics-status" showPending={false} />
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="refresh-log-lines"
          name="Refresh"
          help="Re-read the log file. New lines appear as the app writes them."
        />
        <Button
          id="refresh-log-lines"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          onClick={() => refresh(true)}
        >
          Refresh
        </Button>
      </div>
    </SettingsGroup>
  );
}

// plan 110 (Step A): local formatting helpers for the history row's
// metadata chips + expandable details — deliberately duplicated rather
// than imported from lib/presentation.ts, per HistorySection's own
// "plain scannable list, not a card renderer" rule below.
const HISTORY_EVENT_TYPE_LABELS: Record<string, string> = {
  generic: "Generic",
  score_update: "Score update",
  match_state: "Match state",
  news_item: "News item",
};

// event_type is a plain wire string here (HistoryEvent.event_type),
// unlike rust's closed `EventType` enum — an unrecognized value (a future
// type landing on one side before the other) falls back to the raw
// string rather than throwing.
function historyEventTypeLabel(eventType: string): string {
  return HISTORY_EVENT_TYPE_LABELS[eventType] ?? eventType;
}

// `event.priority` crosses the tauri IPC boundary as untyped JSON
// (`get_history`'s `invoke` return is cast to `HistoryEntry[]`, not
// runtime-validated) — a value the rust side hasn't sent yet, or a typo
// in a future variant, must render legibly rather than as a blank chip
// (`PRIORITY_LABELS[unknownValue]` is `undefined`, which React silently
// renders as nothing). Falls back to the raw wire value, same "total
// lookup" shape as `historyEventTypeLabel` just above.
function historyPriorityLabel(priority: string): string {
  return (PRIORITY_LABELS as Record<string, string>)[priority] ?? priority;
}

function historyRotationLabel(rotation: HistoryRotationSpec): string {
  if (rotation.kind === "one_shot") {
    return `TTL ${rotation.ttl_secs}s`;
  }
  if (rotation.kind === "recurring") {
    return `every ${rotation.display_secs}s`;
  }
  // Same runtime-untrusted-IPC defense as `historyPriorityLabel` above:
  // `HistoryRotationSpec` is a closed two-member union at the type
  // level, so TS narrows `rotation` to `never` past both checks — but an
  // actual malformed/future payload isn't guaranteed to match either
  // member. Read the field back off `unknown` rather than crash or
  // render nothing.
  const raw = rotation as unknown as { kind?: unknown };
  return typeof raw.kind === "string" ? raw.kind : "unknown rotation";
}

// Same HH:MM shape as lib/presentation.ts's publishedLabel (local
// getHours/getMinutes, not toLocaleTimeString, so this stays
// deterministic under a mocked Date in tests) — duplicated locally rather
// than imported, per this section's no-presentation.ts rule.
function historyPublishedLabel(publishedAtMs: number): string {
  const published = new Date(publishedAtMs);
  const hours = published.getHours().toString().padStart(2, "0");
  const minutes = published.getMinutes().toString().padStart(2, "0");
  return `${hours}:${minutes}`;
}

// Text only, no crest artwork (Step A's explicit field disposition) — a
// compact one-line score/clock/cards summary for the expandable details.
function historyEspnSummary(espn: HistoryEspnMeta): string {
  const cardsClean =
    espn.homeCards[0] === 0 &&
    espn.homeCards[1] === 0 &&
    espn.awayCards[0] === 0 &&
    espn.awayCards[1] === 0;
  const cards = cardsClean
    ? ""
    : ` · ${espn.homeAbbrev} ${espn.homeCards[0]}Y${espn.homeCards[1]}R · ${espn.awayAbbrev} ${espn.awayCards[0]}Y${espn.awayCards[1]}R`;
  return `${espn.league}: ${espn.homeAbbrev} ${espn.homeScore}–${espn.awayScore} ${espn.awayAbbrev} (${espn.clock})${cards}`;
}

// "Absent when null/undefined OR blank after trim" (Step A §1) — a
// source/category string of only whitespace reads as absent, same as null.
function historyNonBlank(value: string | null | undefined): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

// plan 110 (Step A): one recorded entry — the always-present metadata row
// (source when present, category when present, priority, event_type, and
// the formatted rotation window — rotation, priority, and event_type are
// Event's own required fields, so they always render) plus a conditional
// native `<details>` for the optional richness (subtitle, topic,
// published time, an espn score/clock/cards summary, each `details[]`
// pair, and the link). `signal`/`id` are intentionally never rendered —
// internal/debug identifiers, not user-facing content (`id` is used only
// as the list key, which isn't rendering).
function HistoryRow({ entry }: { entry: HistoryEntry }) {
  const { event } = entry;
  const source = historyNonBlank(event.meta.source);
  const category = historyNonBlank(event.meta.category);
  const subtitle = historyNonBlank(event.meta.subtitle);
  const topic = historyNonBlank(event.topic);
  const link = historyNonBlank(event.meta.link);
  const details = event.meta.details;
  const hasExpandable =
    subtitle !== null ||
    details.length > 0 ||
    link !== null ||
    topic !== null ||
    event.meta.published_at_ms !== null ||
    event.meta.espn !== undefined;

  // plan 112 Step 4 (History): utilities only, over the native
  // li/details/summary structure Plan 110 landed — the semantics,
  // metadata gate, and the escaped-text (never an <a href>) discipline
  // for `link` all stay verbatim.
  const detailLabelClass =
    "history-detail-label text-fs-caption tracking-[0.04em] text-muted-foreground uppercase";
  const detailValueClass =
    "history-detail-value min-w-0 text-fs-body text-muted-foreground [overflow-wrap:anywhere]";

  return (
    <li className="history-row grid min-w-0 grid-cols-[minmax(0,1fr)] gap-0.5 border-t border-border/60 py-2.5 first:border-t-0">
      <span className="history-time font-mono text-fs-secondary leading-none font-bold text-muted-foreground">
        {new Date(entry.recorded_at_ms).toLocaleString()}
      </span>
      <span className="history-origin ml-1.5 font-mono text-fs-secondary leading-none font-bold text-muted-foreground uppercase">
        {event.origin}
      </span>
      <span className="history-title text-fs-body font-[590] text-foreground">
        {event.payload.title}
      </span>
      <div className="history-meta-row mt-1 flex min-w-0 flex-wrap items-center gap-[5px]">
        {source !== null && (
          <span className="history-meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground [overflow-wrap:anywhere]">
            {source}
          </span>
        )}
        {category !== null && (
          <span className="history-meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground [overflow-wrap:anywhere]">
            {category}
          </span>
        )}
        <span className="history-meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground [overflow-wrap:anywhere]">
          {historyPriorityLabel(event.priority)}
        </span>
        <span className="history-meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground [overflow-wrap:anywhere]">
          {historyEventTypeLabel(event.event_type)}
        </span>
        <span className="history-meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground [overflow-wrap:anywhere]">
          {historyRotationLabel(event.rotation)}
        </span>
      </div>
      <span className="history-body min-w-0 text-fs-body text-muted-foreground [overflow-wrap:anywhere]">
        {event.payload.body}
      </span>
      {hasExpandable && (
        <details className="history-details mt-1.5 min-w-0">
          <summary className="cursor-pointer text-fs-caption font-[650] text-muted-foreground">
            More details
          </summary>
          <div className="history-details-content mt-1.5 flex min-w-0 flex-col gap-1.5 pl-0.5">
            {subtitle !== null && (
              <div className="history-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
                <span className={detailLabelClass}>Subtitle</span>
                <span className={detailValueClass}>{subtitle}</span>
              </div>
            )}
            {topic !== null && (
              <div className="history-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
                <span className={detailLabelClass}>Topic</span>
                <span className={detailValueClass}>{topic}</span>
              </div>
            )}
            {event.meta.published_at_ms !== null && (
              <div className="history-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
                <span className={detailLabelClass}>Published</span>
                <span className={detailValueClass}>
                  {historyPublishedLabel(event.meta.published_at_ms)}
                </span>
              </div>
            )}
            {event.meta.espn !== undefined && (
              <div className="history-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
                <span className={detailLabelClass}>Match</span>
                <span className={detailValueClass}>{historyEspnSummary(event.meta.espn)}</span>
              </div>
            )}
            {details.map((detail) => (
              <div
                className="history-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px"
                key={`${detail.label}:${detail.value}`}
              >
                <span className={detailLabelClass}>{detail.label}</span>
                <span className={detailValueClass}>{detail.value}</span>
              </div>
            ))}
            {link !== null && (
              <div className="history-detail-field grid min-w-0 grid-cols-[minmax(0,1fr)] gap-px">
                <span className={detailLabelClass}>Link</span>
                {/* plan 110 (Step A): untrusted feed data (RSS/ESPN) —
                    literal, selectable TEXT, never an <a href> or
                    in-webview navigation. No vetted external-open
                    precedent exists in this webview; adding one is a
                    separate IPC/capability plan. */}
                <span className={cn(detailValueClass, "history-link-text select-text")}>
                  {link}
                </span>
              </div>
            )}
          </div>
        </details>
      )}
    </li>
  );
}

// plan 089: read-only recent history, newest first (088's read_recent
// contract itself stays oldest -> newest; the reversal happens here at
// the display layer, not in the rust store). Same advisory mount-only
// fetch shape as DiagnosticsSection above. Not a card renderer — this
// deliberately does not import overlay components or presentation.ts;
// history is a plain scannable list.
function HistorySection({ config }: { config: Config }) {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [confirmingClear, setConfirmingClear] = useState(false);
  // History load and clear are independent operations (plan 108): distinct
  // status instances, distinct UI locations, distinct announce behavior.
  // There is deliberately no manual Refresh control for history — the only
  // read attempt is the passive mount fetch below.
  const loadStatus = useActionStatus("history-load");
  const clearStatus = useActionStatus("history-clear");

  function refresh() {
    void loadStatus.run(
      () => invoke<HistoryEntry[]>("get_history").then((fetched) => setEntries(fetched)),
      {
        announce: false,
        showPending: false,
        errorMessage: () => "Couldn't load history",
      },
    );
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_history on every render.
  useEffect(() => {
    refresh();
  }, []);

  async function handleClearClick() {
    if (!confirmingClear) {
      // step one of the in-component two-step confirmation — never a
      // browser confirm()/alert(), which would block the webview.
      setConfirmingClear(true);
      return;
    }
    await clearStatus.run(() => invoke("clear_history"), {
      announce: true,
      okMessage: "History cleared",
      errorMessage: (reason) => {
        // Errors are surfaced inline now, but the console line costs
        // nothing and helps a dev watching the console too.
        console.error("clear_history failed:", reason);
        return "Couldn't clear history";
      },
    });
    setConfirmingClear(false);
    refresh();
  }

  const newestFirst = entries === null ? null : [...entries].reverse();

  return (
    <SettingsGroup
      title="Recorded notifications"
      description="The most recent notifications recorded to ~/.config/notchtap/history.jsonl, newest first."
    >
      <ActionStatus
        status={loadStatus.status}
        className="history-load-status"
        showPending={false}
      />
      {newestFirst === null ? (
        <p className="history-empty m-0 py-3 text-fs-body text-muted-foreground">Loading…</p>
      ) : newestFirst.length === 0 ? (
        <p className="history-empty m-0 py-3 text-fs-body text-muted-foreground">
          {config.history_enabled
            ? "History is on, but nothing has been recorded yet."
            : 'History is off. Turn on "Record notification history" in General to start recording.'}
        </p>
      ) : (
        <ul className="history-list flex flex-col py-1 pb-[11px]">
          {newestFirst.map((entry) => (
            <HistoryRow key={entry.event.id} entry={entry} />
          ))}
        </ul>
      )}
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="clear-history"
          name="Clear history"
          help="Permanently deletes every recorded notification. This cannot be undone."
        />
        <Button
          id="clear-history"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          disabled={clearStatus.status.state === "pending"}
          // the <label htmlFor="clear-history"> above would otherwise
          // become this button's accessible name via native label
          // association, freezing it at "Clear history" even once the
          // visible text flips to "Really clear?" — aria-label takes
          // precedence and keeps the accessible name in sync with what's
          // on screen.
          aria-label={confirmingClear ? "Really clear?" : "Clear history"}
          onClick={() => void handleClearClick()}
        >
          {confirmingClear ? "Really clear?" : "Clear history"}
        </Button>
      </div>
      <ActionStatus status={clearStatus.status} className="history-clear-status" />
    </SettingsGroup>
  );
}

function TestButton({ source }: { source: TestSource }) {
  const { status, run } = useActionStatus("send-test");
  const pending = status.state === "pending";

  async function send() {
    await run(() => invoke("send_test_notification", { source }), {
      announce: true,
      okMessage: "Queued",
      errorMessage: (reason) => {
        // Errors are surfaced inline now, but the console line costs
        // nothing and helps a dev watching the console too.
        console.error("send_test_notification failed:", reason);
        return describeActionError(reason);
      },
    });
  }

  return (
    <div className="test-button-wrap flex flex-col items-end gap-0.5">
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="text-fs-secondary"
        disabled={pending}
        onClick={() => void send()}
      >
        {pending ? "Sending…" : "Send test notification"}
      </Button>
      <ActionStatus status={status} className="test-button-status mt-0 text-right" />
    </div>
  );
}

function TestButtonRow({ name, help, source }: { name: string; help: string; source: TestSource }) {
  return (
    <div className={CONTROL_ROW}>
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
    <div className={CONTROL_ROW}>
      <div className="control-copy min-w-0">
        {/* not a form-control label (the fieldset/legend below supplies
            the group's accessible name) — a plain span avoids an
            orphaned <label for="…">. */}
        <span className="control-name block text-fs-body leading-[1.3] font-[590] text-foreground">
          {label}
        </span>
      </div>
      <fieldset
        // plan 115: rounded-[7px] is intentionally off-scale (sits
        // between --radius-sm/6px and --radius-md/8px, no scale rung
        // matches) — left as a literal arbitrary value; snapping either
        // way would visibly shift this control's corner radius.
        className="segmented-control grid h-[31px] w-[180px] min-w-0 flex-none grid-cols-3 gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]"
      >
        <legend className="sr-only">{label}</legend>
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className={cn(
              // plan 115: rounded-[4px] is intentionally off-scale (no
              // --radius-* rung is 4px; --radius-sm is 6px) — left as a
              // literal arbitrary value rather than shifting the
              // visible corner radius.
              "segmented-control-button rounded-[4px] border-0 bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:shadow-[0_0_0_2px_var(--ring)]",
              value === option.value &&
                "is-selected bg-accent text-foreground shadow-[var(--shadow-selected)]",
            )}
            aria-pressed={value === option.value}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        ))}
      </fieldset>
    </div>
  );
}

function AppearanceSection({
  config,
  patchConfig,
  applyAppearanceLive,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
  // Owned by SettingsApp, not this component (plan 108 step A): this
  // component remounts on every Reset/Reset-to-defaults (formGeneration
  // key, plan 027) AND doesn't even exist while another section is open —
  // but Reset/Reset to defaults are footer buttons, clickable from any
  // section. So the function lives one level up, and its status renders in
  // the footer (always visible), not here — see the settings-footer JSX.
  applyAppearanceLive: (scale: number, radius: number, opacity: number) => void;
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
    applyAppearanceLive(next.card_scale, next.card_radius, next.card_opacity);
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
              {/* plan 111: `.card-root` scopes the shared card-shape
                  stylesheet (overlay-card.css) — each sample gets its OWN
                  scope (one wrapper per card, matching the overlay's
                  one-wrapper-per-card shape), and `.preview-stage` is
                  already the per-sample frame box, so the scope class
                  composes onto it rather than adding a further nested
                  element. `.appearance-preview` itself stays frame chrome
                  only (settings.css) — never the scope host. */}
              <div className="preview-stage card-root">
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
  // Owned here, not inside AppearanceSection (which remounts on every
  // Reset/Reset-to-defaults via the formGeneration key, plan 027) — so a
  // reset's own live-apply failure survives the remount it triggers. See
  // AppearanceSection's applyAppearanceLive prop.
  const appearanceStatus = useActionStatus("appearance-live-apply");
  // Defaults-fetch and connector-health are both passive, mount/poll-only
  // reads (plan 108) — never announced.
  const defaultsStatus = useActionStatus("defaults");
  const connectorHealthStatus = useActionStatus("connector-health");

  function runAppearanceApply(scale: number, radius: number, opacity: number) {
    void appearanceStatus.run(() => invoke("set_appearance", { scale, radius, opacity }), {
      announce: true,
      showPending: false,
      errorMessage: () => "Live preview couldn't update — will apply on Save & Relaunch",
    });
  }

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
    // just stays disabled (see the footer's `disabled={!defaults || saving}`)
    // — but now the disabled state carries a visible reason (plan 108).
    void defaultsStatus.run(
      () =>
        invoke<Config>("get_default_config").then((loadedDefaults) => {
          if (active) setDefaults(copyConfig(loadedDefaults));
        }),
      {
        announce: false,
        showPending: false,
        errorMessage: () => "Defaults unavailable — reset disabled",
      },
    );
    return () => {
      active = false;
    };
  }, []);

  // Connector health is advisory like get_default_config — fetched on its
  // own, isolated from the critical panel load, and refreshed on a light
  // polling interval so a run of drops surfaces without a window reopen.
  // A failed or unknown fetch leaves the data-driven line hidden, same as
  // before — but the read failure itself is now transition-only inline
  // status (plan 108): connectorHealthStatus's dedup (see useActionStatus)
  // means back-to-back identical poll failures collapse into a single
  // render/transition, never aria-live (a poll is never user-initiated).
  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only poll setup — connectorHealthStatus.run is a fresh closure every render, adding it would tear down and restart the interval on every render.
  useEffect(() => {
    let active = true;
    const fetchHealth = () => {
      void connectorHealthStatus.run(
        () =>
          invoke<ConnectorHealthDto>("get_connector_health").then((health) => {
            if (active && health) setConnectorHealth(health);
          }),
        {
          announce: false,
          showPending: false,
          errorMessage: () => "Health unavailable",
        },
      );
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
    if (!lastLoadedConfig) return;
    // Form state and live-apply are separate concerns (plan 108 step A):
    // the form always resets from lastLoadedConfig regardless of whether
    // the live overlay could be updated to match — a failed live-apply
    // reports through appearanceStatus, it doesn't block the form reset.
    applyForm(lastLoadedConfig);
    const { card_scale, card_radius, card_opacity } = lastLoadedConfig.appearance;
    runAppearanceApply(card_scale, card_radius, card_opacity);
  }

  function resetDefaults() {
    if (!defaults) return;
    applyForm(defaults);
    const { card_scale, card_radius, card_opacity } = defaults.appearance;
    runAppearanceApply(card_scale, card_radius, card_opacity);
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
      <main
        className="settings-window grid h-full w-full min-h-[480px] grid-cols-[140px_minmax(0,1fr)] overflow-hidden bg-background max-[430px]:grid-cols-[122px_minmax(0,1fr)]"
        aria-labelledby="section-title"
      >
        <aside
          className="settings-sidebar grid min-w-0 grid-rows-[auto_minmax(0,1fr)_auto] border-r border-border bg-sidebar"
          aria-label="Settings sections"
        >
          <div className="sidebar-brand flex min-h-[71px] items-center gap-2 border-b border-border/60 px-3.5 pt-[17px] pb-3.5 font-mono text-fs-body leading-none font-bold tracking-[0.09em] text-foreground uppercase not-italic max-[430px]:px-[10px]">
            <span
              className="brand-slot h-2 w-[17px] flex-none rounded-tl-[2px] rounded-tr-[2px] rounded-br-[5px] rounded-bl-[5px] bg-foreground opacity-[0.86]"
              aria-hidden="true"
            />
            <span>notchtap</span>
          </div>
          <nav className="sidebar-nav flex flex-col gap-[3px] px-2 py-3">
            {navigation.map((item) => {
              const Icon = item.icon;
              const selected = item.id === activeSection;
              return (
                <button
                  key={item.id}
                  className={cn(
                    "nav-item relative grid min-h-[38px] min-w-0 grid-cols-[16px_minmax(0,1fr)] items-center gap-2 rounded-md border-0 border-l-2 border-l-transparent bg-transparent py-[7px] pr-2 pl-[6px] text-left text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
                    selected && "is-active border-l-primary bg-primary/15 text-foreground",
                  )}
                  type="button"
                  aria-current={selected ? "page" : undefined}
                  onClick={() => setActiveSection(item.id)}
                >
                  <Icon aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={1.75} />
                  <span className="min-w-0 overflow-hidden font-[560] text-fs-body text-ellipsis leading-[1.25]">
                    {item.label}
                  </span>
                </button>
              );
            })}
          </nav>
          <div className="sidebar-meta border-t border-border/60 px-3.5 pt-[11px] pb-[13px] font-mono text-fs-secondary font-bold tracking-[0.1em] text-muted-foreground uppercase max-[430px]:px-[10px]">
            settings / v5
          </div>
        </aside>

        <div className="settings-pane grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] bg-background">
          {config ? (
            <form
              id="settings-form"
              className="settings-form grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)] grid-rows-[auto_auto_minmax(0,1fr)] overflow-hidden"
              noValidate
              onSubmit={(event) => void saveConfig(event)}
            >
              <header className="content-header border-b border-border bg-background px-5 pt-[22px] pb-[17px] max-[430px]:px-4">
                <div className="section-index mb-2 font-mono text-fs-secondary font-bold tracking-[0.11em] text-muted-foreground uppercase">
                  Settings / {currentSection.index}
                </div>
                <h1
                  id="section-title"
                  className="m-0 text-fs-title leading-[1.15] font-[650] tracking-[-0.018em] text-foreground"
                >
                  {currentSection.title}
                </h1>
                <p className="mt-1.5 mr-0 mb-0 ml-0 max-w-[290px] text-fs-body leading-[1.45] text-muted-foreground">
                  {currentSection.description}
                </p>
              </header>

              <ErrorPanel errors={errors} />

              <div className="section-scroll min-h-0 overflow-y-auto overscroll-contain">
                <AnimatePresence mode="wait" initial={false}>
                  <motion.div
                    className="section-content min-h-full px-4 pt-4 pb-6 max-[430px]:px-3"
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
                        connectorHealthStatus={connectorHealthStatus.status}
                        patchConfig={patchConfig}
                        refreshSecretStatus={refreshSecretStatus}
                      />
                    ) : null}
                    {activeSection === "shortcuts" ? <ShortcutsSection /> : null}
                    {activeSection === "diagnostics" ? <DiagnosticsSection /> : null}
                    {activeSection === "history" ? <HistorySection config={config} /> : null}
                    {activeSection === "appearance" ? (
                      // keyed on formGeneration so Reset/Reset-to-defaults remounts the
                      // section — its controls seed local state from config.appearance at
                      // mount and would otherwise show stale values (plan 027). The
                      // live-apply status itself renders in the footer, not here — this
                      // section doesn't even exist while another section is open, but
                      // Reset/Reset to defaults (which also drive this same status) are
                      // footer buttons reachable from every section.
                      <AppearanceSection
                        key={formGeneration}
                        config={config}
                        patchConfig={patchConfig}
                        applyAppearanceLive={runAppearanceApply}
                      />
                    ) : null}
                  </motion.div>
                </AnimatePresence>
              </div>
            </form>
          ) : (
            <div
              className="loading-state grid min-h-0 place-items-center text-fs-secondary leading-none font-bold tracking-[0.1em] text-muted-foreground uppercase not-italic"
              role="status"
            >
              Loading settings…
            </div>
          )}

          <footer className="settings-footer flex min-h-[57px] items-center gap-1.5 border-t border-border bg-background px-3.5 py-3 max-[430px]:px-[10px]">
            <Button
              type="submit"
              form="settings-form"
              disabled={!config || saving}
              className="text-fs-secondary max-[430px]:px-[7px] max-[430px]:text-fs-caption"
            >
              {saving ? "Relaunching…" : "Save & Relaunch"}
            </Button>
            <Button
              variant="outline"
              type="button"
              disabled={!lastLoadedConfig || saving}
              onClick={resetLoaded}
              className="text-fs-secondary max-[430px]:px-[7px] max-[430px]:text-fs-caption"
            >
              Reset
            </Button>
            <Button
              variant="outline"
              type="button"
              disabled={!defaults || saving}
              onClick={resetDefaults}
              className="text-fs-secondary max-[430px]:px-[7px] max-[430px]:text-fs-caption"
            >
              Reset to defaults
            </Button>
            <ActionStatus
              status={defaultsStatus.status}
              className="defaults-status mt-0"
              showPending={false}
            />
            {/* Always visible regardless of active section (plan 108 step A):
                Reset and Reset to defaults are footer buttons reachable from
                any section, and the appearance sliders are high-frequency —
                pending/ok never render here, only a deduplicated error,
                cleared by the next successful apply. */}
            <ActionStatus
              status={appearanceStatus.status}
              className="appearance-live-status mt-0"
              showPending={false}
            />
          </footer>
        </div>
      </main>
    </MotionConfig>
  );
}
