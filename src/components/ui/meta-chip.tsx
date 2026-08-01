import type * as React from "react";

import { cn } from "@/lib/utils";

// M16 consolidation: this repo had grown FOUR separate implementations
// of the same "small mono metadata chip" role — History's
// `history-meta-chip` / Queue's `queue-priority-tag`
// (rounded-full border-border px-[7px] py-0.5 font-[650]), About's
// tech-stack chip (rounded-full px-[9px] py-[3px], no weight), a shadcn
// `Badge` override in Connectors (rounded-[4px] px-[5px] py-[3px]), and
// Shortcuts' `shortcut-status` (rounded-[3px] px-1 py-0.5). One shape —
// radius/padding/weight/tracking — replaces all four call sites; the two
// genuinely semantic differences (a status chip reading as emphasized
// once "set"/"active", and free-form content wanting mixed case rather
// than an uppercase status word) stay as the `active`/`uppercase` props
// below rather than forking the shape again.
export function MetaChip({
  active = false,
  uppercase = false,
  dotColor,
  className,
  children,
  ...props
}: React.ComponentProps<"span"> & {
  /** Emphasized tone for a status chip that's "set"/"active" — e.g. a
   *  secret that's saved, or a shortcut that's actually wired up. */
  active?: boolean;
  /** Status-word chips (Connectors' secret status, Shortcuts' active/
   *  planned column) read as uppercase; free-form content chips
   *  (a history entry's source, a tech-stack name) don't. */
  uppercase?: boolean;
  /** Plan 147: leading colour swatch — a source/runtime/category
   *  identity dot pulled from `src/lib/sourceColors.ts`. Omitted by
   *  default (no swatch, no layout change). */
  dotColor?: string;
}) {
  return (
    <span
      data-slot="meta-chip"
      className={cn(
        "meta-chip min-w-0 rounded-full border border-border px-[7px] py-0.5 font-mono text-fs-caption font-[650] leading-[1.5] text-muted-foreground transition-colors duration-[150ms] ease-notchtap",
        uppercase && "tracking-[0.06em] uppercase",
        active && "border-ring/40 bg-input/40 text-foreground",
        className,
      )}
      {...props}
    >
      {dotColor ? (
        <span
          data-slot="meta-chip-dot"
          className="meta-chip-dot mr-[5px] inline-block size-[6px] rounded-full align-middle"
          style={{ background: dotColor }}
        />
      ) : null}
      {children}
    </span>
  );
}
