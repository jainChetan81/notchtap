import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { MetaChip } from "@/components/ui/meta-chip";
import { cn } from "@/lib/utils";
import { NOTCHTAP_EASE } from "../../animationTiming";
import { ActionStatus, useActionStatus } from "../actionStatus";
import { CONTROL_ROW, ControlCopy, SettingsGroup } from "../controls/controls";
import { settingsInvoke } from "../ipc";
import type { QueueItemSummary } from "../types";
import { PRIORITY_LABELS } from "../types";

// `get_queue` returns summaries with no id and a refresh replaces the
// whole array — row content is the only stable identity. `occurrenceIndex`
// disambiguates duplicate identical summaries (same priority/source/title)
// so rows keep distinct React keys; the running Map stays deterministic
// across an identical refetch, so rows don't remount and AnimatePresence
// can tell "still here" from "left".
function withQueueRowKeys(
  items: QueueItemSummary[],
): Array<{ item: QueueItemSummary; rowKey: string }> {
  const seen = new Map<string, number>();
  return items.map((item) => {
    const base = `${item.priority}:${item.source}:${item.title}`;
    const occurrenceIndex = seen.get(base) ?? 0;
    seen.set(base, occurrenceIndex + 1);
    return { item, rowKey: `${base}:${occurrenceIndex}` };
  });
}

// Read-only view of the WAITING items behind the visible card (the overlay
// only exposes queue_total/queue_done dots) plus two controls: Skip current
// (dismiss visible card, promote next waiting item — via skip_visible's
// semantics) and Clear queue (drop every waiting item; visible card
// finishes its normal ttl/rotation). Fetch-on-open + manual Refresh, same
// as DiagnosticsSection. Titles are UNTRUSTED wire data — plain text only:
// no dangerouslySetInnerHTML, no <a href>, no markdown rendering.
export function QueueSection() {
  const [items, setItems] = useState<QueueItemSummary[] | null>(null);
  const loadStatus = useActionStatus("queue-load");
  const skipStatus = useActionStatus("queue-skip");
  const clearStatus = useActionStatus("queue-clear");

  // `announce` is explicit per call, not a static prop — same split as
  // DiagnosticsSection's own `refresh`: the mount-time read is passive,
  // the Refresh button's call is a user-initiated attempt.
  function refresh(announce: boolean) {
    void loadStatus.run(() => settingsInvoke("get_queue").then((fetched) => setItems(fetched)), {
      announce,
      showPending: false,
      errorMessage: () => "Couldn't load the queue",
    });
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only fetch on section-open — refresh is re-created every render, so adding it would re-invoke get_queue on every render.
  useEffect(() => {
    refresh(false);
  }, []);

  async function handleSkipClick() {
    await skipStatus.run(() => settingsInvoke("skip_current"), {
      announce: true,
      okMessage: "Skipped",
      errorMessage: () => "Couldn't skip the current notification",
    });
    refresh(false);
  }

  async function handleClearClick() {
    await clearStatus.run(() => settingsInvoke("clear_queue"), {
      announce: true,
      okMessage: "Queue cleared",
      errorMessage: () => "Couldn't clear the queue",
    });
    refresh(false);
  }

  return (
    <SettingsGroup
      title="Waiting notifications"
      description="What's queued behind the visible card, and controls to skip or clear it."
    >
      <ActionStatus status={loadStatus.status} className="queue-load-status" showPending={false} />
      {items === null ? (
        // Reuses `loadStatus.status.state` — the same signal the ActionStatus
        // banner above reads — so this never disagrees with the banner
        // about whether the last attempt failed.
        loadStatus.status.state === "error" ? (
          <p className="queue-empty m-0 py-3 text-fs-body text-muted-foreground">
            Couldn't load the queue — Refresh to retry.
          </p>
        ) : (
          <p className="queue-empty m-0 py-3 text-fs-body text-muted-foreground">Loading…</p>
        )
      ) : (
        <>
          {/* Sibling of the <ul> below, not a replacement — see the
              AnimatePresence comment inside the <ul>. */}
          {items.length === 0 && (
            <p className="queue-empty m-0 py-3 text-fs-body text-muted-foreground">
              Queue is empty.
            </p>
          )}
          <ul className={cn("queue-list flex flex-col", items.length > 0 && "py-1 pb-[11px]")}>
            {/* initial={false}: first mount and an unchanged Refresh swap
                (rows keep their keys) never cascade — only rows genuinely
                appearing/leaving animate; Skip reads as "that one row left",
                Clear collapses removed rows. <ul> + AnimatePresence stay
                mounted while empty; the empty-state <p> is the sibling
                above, gated on `items.length === 0` directly, so it appears
                while last rows still exit. `py-1 pb-[11px]` gated on
                `items.length > 0` so an empty <ul> adds no gap. */}
            <AnimatePresence initial={false}>
              {withQueueRowKeys(items).map(({ item, rowKey }) => (
                <motion.li
                  key={rowKey}
                  className="queue-row grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border-t border-border/60 py-2 first:border-t-0"
                  style={{ overflow: "hidden" }}
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18, ease: NOTCHTAP_EASE }}
                >
                  <span className="queue-title min-w-0 text-fs-body text-foreground [overflow-wrap:anywhere]">
                    {item.title}
                  </span>
                  <MetaChip className="queue-priority-tag">
                    {PRIORITY_LABELS[item.priority]}
                  </MetaChip>
                </motion.li>
              ))}
            </AnimatePresence>
          </ul>
        </>
      )}
      {/* Refresh announces (`refresh(true)`), exactly like
          DiagnosticsSection's Refresh row; the mount-time fetch stays
          `refresh(false)`. */}
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="refresh-queue"
          name="Refresh"
          help="Re-fetch the waiting list. Not live — this is a manual pull."
        />
        <Button
          id="refresh-queue"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          onClick={() => refresh(true)}
        >
          Refresh
        </Button>
      </div>
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="skip-current"
          name="Skip current"
          help="Dismiss the visible card now and promote the next waiting item."
        />
        <Button
          id="skip-current"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          disabled={skipStatus.status.state === "pending"}
          onClick={() => void handleSkipClick()}
        >
          Skip current
        </Button>
      </div>
      <ActionStatus status={skipStatus.status} className="queue-skip-status" />
      <div className={CONTROL_ROW}>
        <ControlCopy
          htmlFor="clear-queue"
          name="Clear queue"
          help="Drops every waiting notification. The visible card is unaffected and finishes its normal turn."
        />
        <Button
          id="clear-queue"
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          disabled={clearStatus.status.state === "pending"}
          onClick={() => void handleClearClick()}
        >
          Clear queue
        </Button>
      </div>
      <ActionStatus status={clearStatus.status} className="queue-clear-status" />
    </SettingsGroup>
  );
}
