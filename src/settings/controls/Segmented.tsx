import { cn } from "@/lib/utils";
import { CONTROL_ROW, ControlCopy } from "./controls";

type SegmentedOption<T extends string | number> = { label: string; value: T };

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
  id?: string;
  name: string;
  help?: string;
  options: ReadonlyArray<SegmentedOption<T>>;
  value: T;
  onChange: (value: T) => void;
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
          <span className="control-name block text-fs-body leading-[1.3] font-[590] text-foreground">
            {name}
          </span>
        </div>
      )}
      <fieldset
        className={
          labelled
            ? `priority-toggle grid h-[31px] w-36 min-w-0 flex-none ${cols} gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]`
            : `segmented-control grid h-[31px] w-[180px] min-w-0 flex-none ${cols} gap-0.5 rounded-[7px] border border-input bg-input/20 p-[3px]`
        }
        id={id}
        aria-labelledby={labelled ? `${id}-label` : undefined}
      >
        <legend className="sr-only">{name}</legend>
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className={cn(
              buttonClass,
              "rounded-[4px] border border-transparent bg-transparent px-1.5 py-px font-mono text-fs-secondary font-[620] tracking-[0.03em] text-muted-foreground outline-none transition-[color,background-color,border-color,box-shadow,scale] duration-[140ms] ease-notchtap focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:scale-[0.97] active:shadow-[var(--shadow-pressed)]",
              value === option.value
                ? cn(
                    "is-selected shadow-[var(--shadow-selected)]",
                    optionTones?.[option.value] ?? "bg-accent text-foreground",
                  )
                : "hover:bg-accent hover:text-foreground",
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
