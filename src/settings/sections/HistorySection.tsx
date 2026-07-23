import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { NOTCHTAP_EASE } from "../../animationTiming";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { CONTROL_ROW, ControlCopy, SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { Config, HistoryEntry, HistoryEspnMeta, HistoryRotationSpec } from "../types";
import { PRIORITY_LABELS } from "../types";

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
    <motion.li
      className="history-row grid min-w-0 grid-cols-[minmax(0,1fr)] gap-0.5 border-t border-border/60 py-2.5 first:border-t-0"
      style={{ overflow: "hidden" }}
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.18, ease: NOTCHTAP_EASE }}
    >
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
    </motion.li>
  );
}

// plan 089: read-only recent history, newest first (088's read_recent
// contract itself stays oldest -> newest; the reversal happens here at
// the display layer, not in the rust store). Same advisory mount-only
// fetch shape as DiagnosticsSection above. Not a card renderer — this
// deliberately does not import overlay components or presentation.ts;
// history is a plain scannable list.
export function HistorySection({ config }: { config: Config }) {
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
          {/* plan 126: initial={false} so the section's own first mount (and
              a load that happens to return the same entries) never
              cascades — only a row genuinely appearing or leaving animates.
              Clear collapses the rows it removes. */}
          <AnimatePresence initial={false}>
            {newestFirst.map((entry) => (
              <HistoryRow key={entry.event.id} entry={entry} />
            ))}
          </AnimatePresence>
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
