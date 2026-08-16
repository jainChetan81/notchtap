import { cn } from "@/lib/utils";
import { CONTROL_ROW, ControlCopy } from "./controls";

// The one segmented control, in two forms discriminated by `id`: labelled
// (ControlCopy renders the name; the fieldset points aria-labelledby back at
// it, since fieldset isn't labelable) and bare (span + sr-only legend, 180px).
type SegmentedOption<T extends string | number> = { label: string; value: T };

// Tailwind only generates utilities it can see as literals — a computed
// `grid-cols-${n}` template would silently produce no CSS.
const GRID_COLS = {
  2: "grid-cols-2",
  3: "grid-cols-3",
  4: "grid-cols-4",
} satisfies Record<number, string>;

export function Segmented<T extends string | number>({
  id,
  name,
  help,
  options,
  value,
  onChange,
  optionTones,
}: {
  /** Present for the labelled control-row form; absent for the bare (Appearance) form. */
  id?: string;
  name: string;
  /** Required whenever `id` is given — the ControlCopy help line. */
  help?: string;
  options: ReadonlyArray<SegmentedOption<T>>;
  value: T;
  onChange: (value: T) => void;
  /**
   * Per-value override for the SELECTED segment's color. Must be literal
   * Tailwind class strings — the content scanner can't see runtime-built
   * names (same constraint as GRID_COLS). Generic coloring hook, not
   * priority-specific; omit for the default neutral selected state.
   */
  optionTones?: Partial<Record<T, string>>;
}) {
  const labelled = id !== undefined;
  // SAFETY: `options.length` (2 or 3) is a known grid key — the `as keyof` narrows the numeric index after the literal check.
  const cols = GRID_COLS[options.length as keyof typeof GRID_COLS] ?? "grid-cols-3";
  const buttonClass = labelled ? "priority-toggle-button" : "segmented-control-button";
  return (
    <div className={CONTROL_ROW}>
      {labelled && help !== undefined ? (
        <ControlCopy htmlFor={id} name={name} help={help} />
      ) : (
        <div className="control-copy min-w-0">
          {/* the fieldset/legend below supplies the group's accessible name —
              a plain span avoids an orphaned <label for>. */}
          <span className="control-name block text-fs-body leading-[1.3] font-[590] text-foreground">
            {name}
          </span>
        </div>
      )}
      <fieldset
        // rounded-[7px] is intentionally off-scale (between --radius-sm/6px
        // and --radius-md/8px); snapping either way would visibly shift the
        // corner radius.
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
              // rounded-[4px] is intentionally off-scale (no --radius-* rung
              // is 4px) — a scale rung would shift the visible corner radius.
              buttonClass,
              // Focus ring and press shadow match button.tsx/switch.tsx, so
              // every pressable primitive shares one focus/depth vocabulary.
              // While pressed, --shadow-pressed can out-cascade the selected
              // pill's resting shadow — intentional: it should read as depressing.
              "rounded-[4px] border border-transparent bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-[color,background-color,border-color,box-shadow,scale] duration-[140ms] ease-notchtap focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:scale-[0.97] active:shadow-[var(--shadow-pressed)]",
              value === option.value
                ? cn(
                    "is-selected shadow-[var(--shadow-selected)]",
                    optionTones?.[option.value] ?? "bg-accent text-foreground",
                  )
                : // Hover is scoped to the unselected case so a selected tone
                  // pill keeps its own color on hover.
                  "hover:bg-accent hover:text-foreground",
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
