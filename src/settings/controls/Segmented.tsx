import { cn } from "@/lib/utils";
import { CONTROL_ROW, ControlCopy } from "./controls";

// plan 119: the ONE segmented control. Replaces the three near-identical
// implementations SettingsApp.tsx grew (PriorityToggle, UnitsToggle,
// SegmentedControl — the 2026-07-23 review's triplication finding).
//
// Two rendered forms, discriminated by whether `id` is present — exactly
// the split the three originals had:
//
// - labelled (id + help given; the old PriorityToggle/UnitsToggle form):
//   ControlCopy renders the visible name, and the fieldset carries
//   id + aria-labelledby back to it (plan 109 semantics — fieldset isn't
//   a labelable element, so <label for> alone can't associate).
// - bare (no id; the old SegmentedControl form, Appearance's
//   Scale/Radius/Opacity rows): a plain span renders the name (no
//   orphaned <label for>), the sr-only legend alone names the group, and
//   the fieldset is the wider 180px variant.
//
// The class-name tokens (`priority-toggle`/`segmented-control`, and the
// matching *-button tokens) are kept per-form so rendered markup at every
// existing call site is unchanged.
type SegmentedOption<T extends string | number> = { label: string; value: T };

// Tailwind only generates utilities it can see as literals — a computed
// `grid-cols-${n}` template would silently produce no CSS. The one
// authorized visual fix in plan 119 lives here: column count derives from
// the option count (the old UnitsToggle hardcoded grid-cols-3 around 2
// options, leaving a dead third column).
const GRID_COLS: Record<number, string> = {
  2: "grid-cols-2",
  3: "grid-cols-3",
  4: "grid-cols-4",
};

export function Segmented<T extends string | number>({
  id,
  name,
  help,
  options,
  value,
  onChange,
}: {
  /** Present for the labelled control-row form; absent for the bare (Appearance) form. */
  id?: string;
  name: string;
  /** Required whenever `id` is given — the ControlCopy help line. */
  help?: string;
  options: ReadonlyArray<SegmentedOption<T>>;
  value: T;
  onChange: (value: T) => void;
}) {
  const labelled = id !== undefined;
  const cols = GRID_COLS[options.length] ?? "grid-cols-3";
  const buttonClass = labelled ? "priority-toggle-button" : "segmented-control-button";
  return (
    <div className={CONTROL_ROW}>
      {labelled && help !== undefined ? (
        <ControlCopy htmlFor={id} name={name} help={help} />
      ) : (
        <div className="control-copy min-w-0">
          {/* not a form-control label (the fieldset/legend below supplies
              the group's accessible name) — a plain span avoids an
              orphaned <label for="…">. */}
          <span className="control-name block text-fs-body leading-[1.3] font-[590] text-foreground">
            {name}
          </span>
        </div>
      )}
      <fieldset
        // plan 115: rounded-[7px] is intentionally off-scale (sits
        // between --radius-sm/6px and --radius-md/8px, no scale rung
        // matches) — left as a literal arbitrary value; snapping either
        // way would visibly shift this control's corner radius.
        className={
          labelled
            ? `priority-toggle grid h-[31px] w-36 min-w-0 flex-none ${cols} gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]`
            : `segmented-control grid h-[31px] w-[180px] min-w-0 flex-none ${cols} gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]`
        }
        id={id}
        aria-labelledby={labelled ? `${id}-label` : undefined}
      >
        {/* accessible-name only — in the labelled form ControlCopy already
            renders the visible label for this group via the htmlFor above. */}
        <legend className="sr-only">{name}</legend>
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className={cn(
              // plan 115: rounded-[4px] is intentionally off-scale (no
              // --radius-* rung is 4px; --radius-sm is 6px) — left as a
              // literal arbitrary value rather than shifting the
              // visible corner radius.
              buttonClass,
              "rounded-[4px] border-0 bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-colors duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:shadow-[0_0_0_2px_var(--ring)]",
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
