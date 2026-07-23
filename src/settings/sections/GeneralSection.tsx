import { ChevronDown, ChevronUp } from "lucide-react";
import { Button } from "@/components/ui/button";
import { NumberControl, SettingsGroup, TestButtonRow, ToggleControl } from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import type { Config, SourceKind } from "../types";
import { PRIORITY_SEGMENT_OPTIONS, SOURCE_LABELS } from "../types";

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
        <Segmented
          id="manual-default-priority"
          name="Manual push priority"
          help="Fallback for a CLI push that doesn't set its own priority."
          options={PRIORITY_SEGMENT_OPTIONS}
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
