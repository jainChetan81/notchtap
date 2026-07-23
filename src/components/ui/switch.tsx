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
        // 2026-07-23 (operator switch restyle): resized to a clean,
        // iOS-like proportion — track h-22px/w-36px (default),
        // h-16px/w-28px (sm) — so the thumb below (18px/12px, with a
        // consistent 2px inset baked into ITS OWN translate math, not
        // this element) never overflows. `transition-colors
        // duration-150` replaces the old bare `transition-all` so the
        // track's own on/off color swap is explicitly pinned to the
        // spec'd 150ms, matching the thumb's own transition below.
        // Dropped the old `dark:data-unchecked:bg-input/80` half-opacity
        // dimming — that was on top of `--input` (already a fairly dark
        // token), the root cause of the "barely-visible track"
        // complaint; plain `bg-input` alone reads at full, readable
        // contrast against the window background in every state.
        // `p-0` (2026-07-23 finding): this window's Tailwind entry is a
        // deliberate NO-preflight build (settings/base.css's own header
        // comment), so nothing ever zeroes a plain `<button>`'s native UA
        // padding — Radix's Switch root IS a real `<button>`. Measured in
        // headless Chrome: `1px 6px` (Chrome's default button padding),
        // silently eating into the thumb's available travel and making
        // the OLD `calc(100% - 2px)` checked-only inset look plausible
        // purely by accident. Every OTHER shadcn button-family component
        // happens to set its own explicit padding (overriding the UA
        // default without ever naming it), which is why this never
        // surfaced before a component with intentionally NO padding.
        "peer group/switch relative inline-flex shrink-0 items-center rounded-full border border-transparent p-0 transition-colors duration-150 ease-out outline-none after:absolute after:-inset-x-1 after:-inset-y-1.5 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-[size=default]:h-[22px] data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 data-checked:bg-primary data-unchecked:bg-input data-disabled:cursor-not-allowed data-disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          // 2026-07-23 (operator switch restyle): `translate-x-px` (1px)
          // is now the BASE (unchecked) position, not `translate-x-0` —
          // the old unchecked thumb sat flush against the track's inner
          // edge with zero inset while the checked thumb's inset came
          // from a `calc(100% - 2px)` translate, an asymmetry that read
          // as the thumb "overflowing" the track on one side. Both
          // states now carry the SAME 2px inset from the track's OUTER
          // (visible) edge — verified in headless Chrome, not just
          // arithmetic: the track's own `border` (1px, transparent at
          // rest, colored on `focus-visible`) is real box model, not
          // decoration, so the thumb's un-translated static position
          // already starts 1px in from the border-box edge; every
          // translate value below is the REMAINING 1px/13px/15px needed
          // on top of that, landing the thumb exactly 2px from the
          // track's outer edge in every state (confirmed via
          // `getBoundingClientRect` on both the track and thumb, not just
          // computed from track/thumb size alone — the vertical axis
          // gets this for free from `items-center`, since a symmetric
          // flex-centered slack cancels the border term algebraically;
          // the horizontal axis has no such symmetry, hence the
          // border-aware pixel math here):
          //   default: static(1px border) + translate(1px) = 2px unchecked;
          //            static(1px) + translate(15px) = 16px checked, and
          //            36 (track) - 16 - 18 (thumb) = 2px on the right.
          //   sm:      static(1px) + translate(1px) = 2px unchecked;
          //            static(1px) + translate(13px) = 14px checked, and
          //            28 (track) - 14 - 12 (thumb) = 2px on the right.
          // Thumb height is sized so track-height minus 18px/12px leaves
          // exactly 2px top+bottom too (22 - 18 = 4; 16 - 12 = 4) — same
          // 2px inset, all four sides, both states, both sizes.
          // `bg-background` is the non-dark fallback (kept from the
          // original, in case this ever renders outside the always-`.dark`
          // settings window) — the `dark:` pair below is what actually
          // paints in this app today.
          "pointer-events-none block translate-x-px rounded-full bg-background ring-0 transition-transform duration-150 ease-out",
          "group-data-[size=default]/switch:size-[18px] group-data-[size=default]/switch:data-checked:translate-x-[15px]",
          "group-data-[size=sm]/switch:size-3 group-data-[size=sm]/switch:data-checked:translate-x-[13px]",
          // checked keeps the existing AA-safe primary-on-primary pair;
          // unchecked moves off the old bright-white `bg-foreground` to
          // a muted light gray, so an off switch doesn't read as a
          // blown-out white blob against its (now-visible) dark track.
          "dark:data-checked:bg-primary-foreground dark:data-unchecked:bg-muted-foreground",
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
