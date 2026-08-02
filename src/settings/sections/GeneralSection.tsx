import { ChevronDown, ChevronUp } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import {
  ControlCopy,
  NumberControl,
  SettingsGroup,
  TestButtonRow,
  ToggleControl,
} from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import type { Config, SourceKind } from "../types";
import { PRIORITY_SEGMENT_OPTIONS, PRIORITY_TONES, SOURCE_LABELS } from "../types";

// plan 146a (docs/ARCHITECTURE.md §21, CONTEXT.md's Silenced/Silent Period
// entries): validates a `"HH:MM-HH:MM"` (24h) silence window string with
// the EXACT rules `src-tauri/src/silence.rs`'s `Window::parse` enforces —
// split on the first `-`, split each half on the first `:`, both halves
// digits-only in range (hours 0-23, minutes 0-59), start != end (a
// zero-length/24h window has no unambiguous meaning in this format).
// Midnight-crossing (start > end) is deliberately NOT rejected here —
// `Window::in_window` handles it, same as the rust side.
// `isValidSilenceWindow` returns `true`/`false` only; this control never
// needs the parsed minutes themselves, just whether the current text is
// save-able — `parseHhMm` yields minutes solely for that comparison.
function parseHhMm(part: string): number | null {
  const colonIdx = part.indexOf(":");
  if (colonIdx === -1) {
    return null;
  }
  const hStr = part.slice(0, colonIdx);
  const mStr = part.slice(colonIdx + 1);
  if (!/^\d+$/.test(hStr) || !/^\d+$/.test(mStr)) {
    return null;
  }
  const h = Number(hStr);
  const m = Number(mStr);
  if (h > 23 || m > 59) {
    return null;
  }
  return h * 60 + m;
}

export function isValidSilenceWindow(raw: string): boolean {
  const dashIdx = raw.indexOf("-");
  if (dashIdx === -1) {
    return false;
  }
  const start = parseHhMm(raw.slice(0, dashIdx));
  const end = parseHhMm(raw.slice(dashIdx + 1));
  return start !== null && end !== null && start !== end;
}

// plan 146a: the one new text field this plan adds, following
// `NumberControl`'s own established idiom exactly (controls.tsx) — a
// local raw-string mirror of the committed value, re-synced via `useEffect`
// only when the EXTERNAL value changes (Reset, a fresh `get_config`), so
// mid-edit keystrokes are never fought. Unlike `NumberControl`, an invalid
// in-progress value is never silently discarded: `raw` always reflects
// exactly what's typed, `patchConfig` only fires once the text parses per
// `isValidSilenceWindow` above, and an inline error replaces the caption
// while invalid — the server-side `ErrorPanel` (SettingsApp.tsx) remains
// the final backstop (e.g. if this validator and the rust one ever drift),
// but this gives immediate feedback without a save round-trip.
function SilenceWindowControl({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const [raw, setRaw] = useState(value);
  const valid = isValidSilenceWindow(raw);

  useEffect(() => {
    setRaw(value);
  }, [value]);

  return (
    <div className="textarea-control border-t border-border/60 pt-[11px] pb-3 first:border-t-0">
      <ControlCopy
        htmlFor="silence-window"
        name="Silent period window"
        help="24h local time, HH:MM-HH:MM. May cross midnight (e.g. 23:00-07:30)."
      />
      <Input
        id="silence-window"
        spellCheck={false}
        aria-invalid={!valid}
        value={raw}
        placeholder="00:00-10:00"
        onChange={(event) => {
          const next = event.currentTarget.value;
          setRaw(next);
          if (isValidSilenceWindow(next)) {
            onChange(next);
          }
        }}
        className={cn(
          "mt-2 h-[31px] w-32 rounded-sm border-input bg-input/20 font-mono text-fs-body font-[650] text-foreground",
          !valid && "border-destructive/60 text-destructive",
        )}
      />
      <div
        className={cn(
          "field-caption mt-[5px] text-fs-caption font-bold tracking-[0.08em] uppercase",
          valid ? "text-muted-foreground" : "text-destructive",
        )}
      >
        {valid ? "start-end, 24h" : "invalid — expected HH:MM-HH:MM, start ≠ end"}
      </div>
    </div>
  );
}

function RotationOrderList({
  order,
  onChange,
}: {
  order: SourceKind[];
  onChange: (order: SourceKind[]) => void;
}) {
  function move(index: number, delta: number) {
    const target = index + delta;
    // M12 fix: the boundary rows used to key this off native `disabled`
    // on the button instead — the instant a boundary move fired, the
    // button re-rendered disabled and focus dropped to <body>,
    // stranding a keyboard user. The buttons stay focusable at every
    // row now (see below); guarding here instead means an
    // already-at-the-boundary button is a harmless no-op rather than an
    // out-of-bounds swap.
    if (target < 0 || target >= order.length) {
      return;
    }
    const next = [...order];
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
            {/* M12 fix: `aria-disabled` (not native `disabled`) at the
                boundary rows — a natively-disabled button is dropped
                from the tab order the instant it re-renders disabled,
                which is exactly what stranded keyboard focus on `body`
                after a boundary move. Staying focusable (with `move`
                above now a no-op past the boundary) keeps the keyboard
                user's focus on this same button in its new position. */}
            <Button
              type="button"
              variant="outline"
              size="icon-xs"
              className={cn("text-muted-foreground", index === 0 && "opacity-50")}
              aria-label={`Move ${SOURCE_LABELS[source]} earlier`}
              aria-disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              <ChevronUp className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon-xs"
              className={cn("text-muted-foreground", index === order.length - 1 && "opacity-50")}
              aria-label={`Move ${SOURCE_LABELS[source]} later`}
              aria-disabled={index === order.length - 1}
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

export function GeneralSection({
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
          help="Records notification content (including agent-originated payloads) to ~/.config/notchtap/history.jsonl. Applies after Save & Relaunch."
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
        // plan 146b (docs/ARCHITECTURE.md §21): reverses the old "priority
        // never interrupts the visible item" contract — a strictly-higher
        // arrival now cuts the visible card's turn short (Priority
        // Preemption); it re-queues at the head of its own tier with its
        // remaining time intact. Equal priority still never preempts.
        description="Waiting items promote high → medium → low. A strictly-higher-priority arrival interrupts the visible item immediately; equal priority never preempts."
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
        <Segmented
          id="manual-default-priority"
          name="Manual push priority"
          help="Fallback for a CLI push that doesn't set its own priority."
          options={PRIORITY_SEGMENT_OPTIONS}
          optionTones={PRIORITY_TONES}
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

      {/* plan 146a (docs/ARCHITECTURE.md §21, CONTEXT.md's Silenced/Silent
          Period entries): the persisted daily schedule only — the tray's
          own Timed Mutes/Skip are session-only live controls, not edited
          here (see the group description below). A High Event still
          promotes compact (Breakthrough) while Silenced regardless of
          this schedule; that's queue behavior, not a setting. */}
      <SettingsGroup
        title="Silenced"
        description="Medium/Low events buffer during this daily window; a High event still promotes (Breakthrough). Tray mutes and Skip are live controls for right now — this schedule is only the persisted default."
      >
        <ToggleControl
          id="silence-enabled"
          name="Silent period"
          help="Buffer Medium/Low notifications during the window below, every day."
          label="Enable silent period"
          checked={config.silence.enabled}
          onChange={(enabled) => patchConfig({ silence: { ...config.silence, enabled } })}
        />
        <SilenceWindowControl
          value={config.silence.window}
          onChange={(window) => patchConfig({ silence: { ...config.silence, window } })}
        />
      </SettingsGroup>
    </div>
  );
}
