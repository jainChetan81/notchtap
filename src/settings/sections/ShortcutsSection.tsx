import { cn } from "@/lib/utils";
import { SettingsGroup } from "../controls/controls";

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

export function ShortcutsSection() {
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
