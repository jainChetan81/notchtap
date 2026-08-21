import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { MetaChip } from "@/components/ui/meta-chip";
import { NOTCHTAP_EASE } from "@/lib/constants";
import { formatClockTime, formatRecordedAt } from "@/lib/format";
import { isString } from "@/lib/guards";
import { SOURCE_ORIGIN_COLORS, type SourceOriginToken } from "@/lib/sourceColors";
import { cn } from "@/lib/utils";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { CONTROL_ROW, ControlCopy, SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { Config, HistoryEntry, HistoryEspnMeta, HistoryRotationSpec } from "../types";
import { PRIORITY_LABELS } from "../types";

// Row formatting helpers — deliberately duplicated rather than imported
// from lib/presentation.ts: history is a plain scannable list, not a
// card renderer.
const HISTORY_EVENT_TYPE_LABELS = new Map<string, string>([
  ["generic", "Generic"],
  ["score_update", "Score update"],
  ["match_state", "Match state"],
  ["news_item", "News item"],
]);

// event_type is a plain wire string here (HistoryEvent.event_type),
// unlike rust's closed `EventType` enum — an unrecognized value (a future
// type landing on one side before the other) falls back to the raw
// string rather than throwing.
function historyEventTypeLabel(eventType: string): string {
  return HISTORY_EVENT_TYPE_LABELS.get(eventType) ?? eventType;
}

// `event.priority` crosses the tauri IPC boundary as untyped JSON
// (`get_history`'s `invoke` return is cast to `HistoryEntry[]`, not
// runtime-validated) — a value the rust side hasn't sent yet, or a typo
// in a future variant, must render legibly rather than as a blank chip
// (`PRIORITY_LABELS[unknownValue]` is `undefined`, which React silently
// renders as nothing). Falls back to the raw wire value, same "total
// lookup" shape as `historyEventTypeLabel` just above.
function historyPriorityLabel(priority: string): string {
  // SAFETY: hasOwn is the runtime check that priority is a known PriorityLevel — the cast only narrows the lookup after that check.
  // biome-ignore lint/suspicious/noPrototypeBuiltins: Object.hasOwn needs ES2022 lib; tsconfig targets ES2020.
  return Object.prototype.hasOwnProperty.call(PRIORITY_LABELS, priority)
    ? PRIORITY_LABELS[priority as keyof typeof PRIORITY_LABELS]
    : priority;
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
  // SAFETY: the cast intentionally widens a statically-`never` union to an unknown-shaped read; the cast doesn't assert the union — the following isString check is what establishes `kind` before use.
  const raw = rotation as { kind?: unknown };
  return isString(raw.kind) ? raw.kind : "unknown rotation";
}

// Text only, no crest artwork — compact one-line score/clock/cards
// summary for the expandable details.
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

// Absent on null/undefined OR whitespace-only — a blank source/category
// reads as absent. `event.origin` is an untyped wire string: only
// manual/football/agent/news have a colour; an unrecognized origin
// (hand-edited history file only) renders with no inline style.
function historyOriginColor(origin: string): string | undefined {
  // SAFETY: hasOwn is the runtime check that origin is a known SourceOriginToken — the cast only narrows the lookup after that check.
  // biome-ignore lint/suspicious/noPrototypeBuiltins: Object.hasOwn needs ES2022 lib; tsconfig targets ES2020.
  return Object.prototype.hasOwnProperty.call(SOURCE_ORIGIN_COLORS, origin)
    ? SOURCE_ORIGIN_COLORS[origin as SourceOriginToken]
    : undefined;
}

function historyNonBlank(value: string | null | undefined): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

// One recorded entry: metadata row (source/category when present; priority,
// event_type, rotation always render — they're Event's required fields)
// plus a conditional native `<details>` for the optional richness (subtitle,
// topic, published time, espn summary, `details[]` pairs, link).
// `signal`/`id` never render — internal identifiers (`id` is only the key).
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

  // Detail label/value classes — `link` is escaped text, never an <a href>.
  const detailLabelClass =
    "history-detail-label text-fs-caption tracking-[0.04em] text-muted-foreground uppercase";
  const detailValueClass =
    "history-detail-value min-w-0 text-fs-body text-muted-foreground [overflow-wrap:anywhere]";

  return (
    <motion.li
      className="history-row grid min-w-0 grid-cols-[minmax(0,1fr)] gap-0.5 border-t border-border/60 py-2.5 first:border-t-0"
      style={{ overflow: "hidden" }}
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.18, ease: NOTCHTAP_EASE }}
    >
      <span className="history-time font-mono text-fs-secondary leading-none font-bold text-muted-foreground">
        {formatRecordedAt(entry.recorded_at_ms)}
      </span>
      <span
        className="history-origin ml-1.5 font-mono text-fs-secondary leading-none font-bold text-muted-foreground uppercase"
        style={
          historyOriginColor(event.origin) ? { color: historyOriginColor(event.origin) } : undefined
        }
      >
        {event.origin}
      </span>
      <span className="history-title text-fs-body font-[590] text-foreground">
        {event.payload.title}
      </span>
      <div className="history-meta-row mt-1 flex min-w-0 flex-wrap items-center gap-[5px]">
        {source !== null && (
          <MetaChip className="history-meta-chip [overflow-wrap:anywhere]">{source}</MetaChip>
        )}
        {category !== null && (
          <MetaChip className="history-meta-chip [overflow-wrap:anywhere]">{category}</MetaChip>
        )}
        <MetaChip className="history-meta-chip [overflow-wrap:anywhere]">
          {historyPriorityLabel(event.priority)}
        </MetaChip>
        <MetaChip className="history-meta-chip [overflow-wrap:anywhere]">
          {historyEventTypeLabel(event.event_type)}
        </MetaChip>
        <MetaChip className="history-meta-chip [overflow-wrap:anywhere]">
          {historyRotationLabel(event.rotation)}
        </MetaChip>
      </div>
      <span className="history-body min-w-0 text-fs-body text-muted-foreground [overflow-wrap:anywhere]">
        {event.payload.body}
      </span>
      {hasExpandable && (
        <details className="history-details mt-1.5 min-w-0">
          <summary className="cursor-pointer text-fs-caption font-[650] text-muted-foreground transition-transform duration-150 ease-out active:scale-[0.97]">
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
                  {formatClockTime(event.meta.published_at_ms)}
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
                {/* untrusted feed data (RSS/ESPN) — literal, selectable
                    TEXT, never an <a href> or in-webview navigation. */}
                <span className={cn(detailValueClass, "history-link-text select-text")}>
                  {link}
                </span>
              </div>
            )}
          </div>
        </details>
      )}
    </motion.li>
  );
}

// Read-only recent history, newest first — reversal happens here at the
// display layer; the rust store stays oldest -> newest. Advisory
// mount-only fetch, same as DiagnosticsSection. Not a card renderer —
// deliberately no overlay/presentation.ts imports: plain scannable list.
export function HistorySection({ config }: { config: Config }) {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [confirmingClear, setConfirmingClear] = useState(false);
  // Load and clear are independent operations: distinct status instances,
  // distinct UI locations, distinct announce behavior. No manual Refresh
  // control — the only read attempt is the passive mount fetch below.
  const loadStatus = useActionStatus("history-load");
  const clearStatus = useActionStatus("history-clear");

  function refresh() {
    void loadStatus.run(
      () => settingsInvoke("get_history").then((fetched) => setEntries(fetched)),
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
    await clearStatus.run(() => settingsInvoke("clear_history"), {
      announce: true,
      okMessage: "History cleared",
      errorMessage: (reason) => {
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
      ) : (
        <>
          {/* Sibling of the <ul> below, not a replacement — see the
              AnimatePresence comment inside the <ul>. */}
          {newestFirst.length === 0 && (
            <p className="history-empty m-0 py-3 text-fs-body text-muted-foreground">
              {config.history_enabled
                ? "History is on, but nothing has been recorded yet."
                : 'History is off. Turn on "Record notification history" in General to start recording.'}
            </p>
          )}
          <ul
            className={cn("history-list flex flex-col", newestFirst.length > 0 && "py-1 pb-[11px]")}
          >
            {/* initial={false}: first mount and unchanged re-fetch never
                cascade — only rows genuinely appearing/leaving animate;
                Clear collapses removed rows. <ul> + AnimatePresence stay
                mounted while empty; the empty-state <p> is the sibling
                above, gated directly on `newestFirst.length === 0`, so it
                appears while last rows still exit. `py-1 pb-[11px]` gated
                on `length > 0` so an empty <ul> adds no gap. */}
            <AnimatePresence initial={false}>
              {newestFirst.map((entry) => (
                <HistoryRow key={entry.event.id} entry={entry} />
              ))}
            </AnimatePresence>
          </ul>
        </>
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
          // The <label htmlFor="clear-history"> above would freeze the
          // accessible name at "Clear history"; aria-label keeps it in
          // sync with the visible text.
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
