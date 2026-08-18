import { Switch as SwitchPrimitive } from "radix-ui";
import type * as React from "react";

import { cn } from "@/lib/utils";

function Switch({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: "sm" | "default";
}) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      data-size={size}
      className={cn(
        // Track is sized so the thumb (18px/12px with a consistent 2px
        // inset in its own translate math) never overflows. `duration-150`
        // pins the track's on/off color swap to the spec'd 150ms, matching
        // the thumb. No `dark:data-unchecked:bg-input/80` dim — `--input`
        // alone reads at full contrast against the window. This window's
        // Tailwind is a NO-preflight build (settings/base.css header), so
        // nothing zeroes a plain `<button>`'s native 1px 6px UA padding —
        // Radix's Switch root IS a real button, hence the explicit `p-0`,
        // or the padding silently eats the thumb's travel.
        // Press: `active:scale-[0.97]` plus an inset shadow (same value as
        // button.tsx, borrowed from `--shadow-selected` flipped to inset) so
        // the track reads as depressing. Scale stays 0.97, not button's
        // 0.96 — the track is the smaller, secondary surface. `box-shadow`
        // is transitioned so press/release both animate.
        // Tailwind v4 emits `scale-*` as a standalone `scale` property, not
        // `transform` — the transition list must name `scale` for the press
        // to animate.
        "peer group/switch relative inline-flex shrink-0 items-center rounded-full border border-transparent p-0 transition-[color,background-color,border-color,box-shadow,scale] duration-150 ease-out outline-none after:absolute after:-inset-x-1 after:-inset-y-1.5 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-[size=default]:h-[22px] data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:bg-primary data-unchecked:bg-input data-disabled:cursor-not-allowed data-disabled:opacity-50 active:scale-[0.97] active:shadow-[var(--shadow-pressed)]",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          // `translate-x-px` is the BASE (unchecked) position, not `translate-x-0`,
          // so both states carry the same 2px inset from the track's OUTER
          // edge. The track's `border` (1px, transparent at rest) is real
          // box model, so the thumb's static position already starts 1px in
          // from the border-box edge; each translate is the REMAINING px on
          // top of that: unchecked static(1)+translate(1)=2px; checked
          // default static(1)+translate(15)=16px (36-16-18=2px right);
          // sm static(1)+translate(13)=14px (28-14-12=2px right). Thumb
          // height leaves the same 2px top+bottom (22-18=4, 16-12=4).
          // `bg-background` is the non-dark fallback (this window is always
          // `.dark`); the `dark:` pair is what paints today.
          "pointer-events-none block translate-x-px rounded-full bg-background ring-0 transition-transform duration-150 ease-out",
          "group-data-[size=default]/switch:size-[18px] group-data-[size=default]/switch:data-checked:translate-x-[15px]",
          "group-data-[size=sm]/switch:size-3 group-data-[size=sm]/switch:data-checked:translate-x-[13px]",
          // checked keeps the AA-safe dark-thumb-on-blue-track pair, pinned
          // to `bg-background` (not `--primary-foreground`) so the switch
          // doesn't ride on that token's meaning. unchecked uses a muted
          // light gray, not bright white, so an off switch doesn't read as
          // a blown-out white blob.
          "dark:data-checked:bg-background dark:data-unchecked:bg-muted-foreground",
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
