import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { MetaChip } from "@/components/ui/meta-chip";
import { cn } from "@/lib/utils";
import { ControlCopy, SettingsGroup } from "../controls/controls";
import type { Config } from "../types";

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
  { keys: "⌃⇧A", action: "Open or focus the front Agent Session's host app", status: "active" },
  { keys: "⌃⇧,", action: "Open settings", status: "active" },
] as const;

const SHORTCUT_CELL = "border-b border-border/60 px-[13px] py-2.5 text-left align-middle";

const PREFIX_GLYPHS = "⌃⇧";

const UNICODE_WHITE_SPACE =
  /[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/;

export function isValidPrefixShortcut(raw: string): boolean {
  if (!raw.startsWith(PREFIX_GLYPHS)) {
    return false;
  }
  const rest = Array.from(raw.slice(PREFIX_GLYPHS.length));
  return rest.length >= 1 && rest.length <= 24 && !UNICODE_WHITE_SPACE.test(rest.join(""));
}

function PrefixShortcutControl({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const [raw, setRaw] = useState(value);
  const valid = isValidPrefixShortcut(raw);

  useEffect(() => {
    setRaw(value);
  }, [value]);

  return (
    <div className="textarea-control border-t border-border/60 pt-[11px] pb-3 first:border-t-0">
      <ControlCopy
        htmlFor="prefix-shortcut"
        name="Prefix keybinding"
        help="Tmux-style prefix: press this, then one more key, to select a tab, cycle agent sessions, pause, or expand/collapse — then it disarms. The seven shortcuts below are unaffected and keep working prefix-free."
      />
      <Input
        id="prefix-shortcut"
        spellCheck={false}
        aria-invalid={!valid}
        value={raw}
        placeholder="⌃⇧Space"
        onChange={(event) => {
          const next = event.currentTarget.value;
          setRaw(next);
          if (isValidPrefixShortcut(next)) {
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
        {valid
          ? "⌃⇧ + one more key, no spaces"
          : "invalid — expected ⌃⇧ followed by one key, no spaces"}
      </div>
    </div>
  );
}

export function ShortcutsSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  return (
    <div className="section-stack">
      <SettingsGroup
        title="Prefix keybinding"
        description="A tmux-style prefix for the tab-notch icon strip: press it, then press one follow-up key within 2 seconds. 1–3 select a tab, [ and ] cycle agent sessions, enter or o expands, p pauses, esc cancels."
      >
        <PrefixShortcutControl
          value={config.prefix_shortcut}
          onChange={(prefix_shortcut) => patchConfig({ prefix_shortcut })}
        />
      </SettingsGroup>

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
                  <MetaChip
                    uppercase
                    active={shortcut.status === "active"}
                    className="shortcut-status inline-block w-max"
                  >
                    {shortcut.status === "active" ? "active" : "planned · not implemented"}
                  </MetaChip>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </SettingsGroup>
    </div>
  );
}
