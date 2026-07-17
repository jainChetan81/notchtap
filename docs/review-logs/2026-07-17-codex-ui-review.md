# independent UI review — v3.6 single-slot overlay — 2026-07-17

reviewed: `src/styles.css`, `src/App.tsx`, `src/useSlotState.ts`,
`src/useClock.ts`, `IMPLEMENTATION_PLAN.md` §3.6, and
`V3_6_TECHNICAL_SPEC.md`.

## verdict

The implementation has the right architecture and a coherent first visual
language: one persistent top-edge surface, a quiet clock while idle, one
Rust-authoritative visible item, and a compact-to-expanded treatment that does
not reintroduce a frontend lifecycle. The basic proportions are sensible and
the true-black, square-top/rounded-bottom silhouette is appropriate for a
notch-integrated surface.

The main weakness is motion/state communication. The stylesheet contains size
transitions, but no actual enter or exit animation. New content can therefore
snap into the existing slot, particularly when two successive items share the
same priority/expanded classes. The priority system is also too dependent on a
very small color-only mark—especially for `Low`, whose 25%-white accent is close
to invisible on black. I would keep the architecture and edge-flush geometry,
but strengthen the state transition and add a non-color priority cue.

## correctness and implementation review

### What is solid

- `App.tsx` preserves hook order by calling both hooks unconditionally, then
  selects idle/showing markup inside one persistent `.slot`. This is the right
  shape for a surface that morphs instead of mounting a stack.
- The idle clock is genuinely client-only. `useClock.ts` reads `Date`, updates
  every 30 seconds, and has no queue/event plumbing, matching the §3.6 decision.
- `useSlotState.ts` is much better than the minimal spec sketch around async
  cleanup: its `unmounted` flag closes the late-resolving listener instead of
  leaking it after unmount.
- The frontend contains no rotation timers. Direct state replacement correctly
  leaves promotion, rotation, pause, and expand authority in Rust.
- Box sizing is explicit; the 3px priority border is always present (transparent
  when idle), so switching priority does not shift notification text.
- The compact text budget fits its cap: two body lines plus one title line and
  20px vertical padding remain below the 84px `max-height`. The five-line
  expanded body similarly fits below 168px at the current type sizes.
- The top corners remain square while only the bottom corners round, which
  supports the deliberate edge-flush/no-gap decision instead of making the slot
  look like an ordinary floating toast.

### Bugs and behavior risks

1. **The promised enter/exit treatment is not present.** `styles.css` only
   transitions `max-height`, `width`, and `padding`; there are no enter/exit
   keyframes or phase classes. A replacement from one compact `Medium` item to
   another leaves every animated property unchanged, so the title/body swap in
   one frame. A `High` replacement can do the same if both items are expanded.
   This is visible spec drift from `V3_6_TECHNICAL_SPEC.md` §5.3's fixed-duration
   enter/exit language.

2. **There is a transient-event startup race.** `listen("slot-state", ...)` is
   asynchronous and the hook begins as `empty`. If Rust emits the current
   showing state after page load but before the listener registration resolves,
   the payload is lost. Because the backend's `slot_state_if_changed` guard then
   regards that state as already emitted, heartbeat ticks need not resend it;
   the UI can show the idle clock until the next real slot change. The cleanup
   race is handled, but initial synchronization is not. A receive-only-safe fix
   would be the same kind of snapshot-plus-event double shield already used for
   presentation mode, or a backend replay after frontend readiness.

3. **Content appears before its container has made room for it.** On
   idle→showing, the new title/body mount immediately while the slot is still
   animating from 220×32 with zero padding toward its notification geometry.
   `overflow: hidden` prevents spill, but the reveal is incidental clipping,
   not a composed entrance. On showing→idle the reverse is harsher: content is
   removed immediately and only the empty shell animates closed.

4. **`max-height` does not produce a consistent 250ms expansion.** The compact
   content is usually shorter than its 84px cap and five expanded body lines are
   usually shorter than 168px. The visible height therefore starts or stops
   partway through the numeric max-height interpolation, while width continues
   for the full duration. Simultaneously changing width also reflows body text
   during the reveal. The motion can feel clipped or lopsided even though the
   CSS is technically valid.

### Smaller rough edges

- `useClock` remains active while a notification is showing, causing an
  otherwise unnecessary App render every 30 seconds. A small `IdleSlot` child
  could scope that timer to idle periods without conditional-hook problems.
- The clock may be almost 30 seconds behind a minute boundary because its
  interval is not aligned to the wall clock. That is acceptable for the stated
  30-second refresh decision, but a timeout aligned to the next minute followed
  by 60-second ticks would be both more exact and cheaper.
- The formatter follows the user's locale while the idle width is fixed at
  220px. Some long weekday/month/12-hour combinations can clip because the
  clock is `nowrap` with neither responsive width nor ellipsis.
- `eventType` is correctly retained in the wire type but unused in this
  renderer. That is harmless, though it is worth keeping deliberately rather
  than letting it become accidental dead-contract drift.
- There is no reduced-motion rule. The current motion is modest, but a
  `prefers-reduced-motion` override should collapse it to an instant state
  change.
- The changing notification is not exposed as an ARIA live region. If the
  overlay is meant to remain visible-only, document that limitation; otherwise
  `role="status"`/`aria-live` deserves consideration, with care not to make
  every recurring low-priority update noisy for VoiceOver.

## visual and interaction critique

### Priority encoding

`Medium` blue and `High` red are immediately legible as different categories,
and automatic expansion gives `High` a useful second signal. `Low` is the weak
link: `rgba(245, 245, 247, 0.25)` over black is intentionally quiet but is so
quiet that it reads more like an inactive divider than a tier. More broadly, a
3px left strip is a learned code with no label, icon, texture, or shape backup.
It is easy to miss in peripheral vision and does not survive color-vision
differences well.

I would pair the color with one compact semantic cue: one/two/three signal
marks, a circle/diamond/burst glyph, or a tiny `LOW`/`MED`/`HIGH` utility label.
That cue can live at the opposite edge so it does not turn the card into a
traffic-light dashboard. If the strip remains, `Low` needs more contrast or a
different pattern (for example, a neutral dashed rail rather than dimmer color).

### Compact versus expanded

The width change from 360→420 and body clamp from two→five lines is functional,
but the two states are visually too similar. Compact already shows a title plus
two lines, so expanded often feels like “the same card, 60px wider” rather than
a meaningful detail mode. A clearer hierarchy would make compact a one-line
glance surface (title plus a single summary line), then let expanded introduce
detail, metadata, or a more generous title wrap.

The current title is always one line. That is a sensible compact rule, but in
expanded mode it can truncate the most important part of a high-priority alert
while the less important body receives five lines. Allowing a two-line expanded
title, or reserving more width for it, would better match urgency.

There is no explicit gap between the title and body. Their line boxes touch,
which keeps the card dense but weakens hierarchy. A restrained 2–3px gap would
improve scanning without compromising notch compactness.

### Typography, contrast, spacing

The SF system stack is exactly the right default for a macOS overlay. The
13px/600 title over a 12px/75%-white body creates a clear hierarchy, and body
contrast is adequate on true black. The sizes are nevertheless at the lower
edge for content meant to be read at a glance near the top of a high-density
display. I would test 13px body / 14px title on the physical macbook before
settling; the extra pixel may buy more real-world legibility than another body
line.

Horizontal padding at 16px is comfortable. Vertical padding at 10px is slightly
generous relative to the tiny type but works because the surface needs a calm,
system-like feel. The 14px bottom radius is polished and the shadow gives the
HUD fallback separation; on notch hardware, confirm that the shadow does not
create a gray seam against the physical cutout.

### Idle state

The idle clock is calm and useful, and the 220px silhouette makes the surface
feel alive without looking like an empty notification. It is also the most
generic possible idle treatment. A small day-progress tick, focus-state glyph,
or secondary timezone could make idle feel intentional while staying entirely
client-side. The prototypes created alongside this review explore several such
treatments without proposing any queue changes.

## recommended order of changes

1. Close the initial `slot-state` synchronization race; it is the only issue
   here that can make the frontend show the wrong state indefinitely.
2. Design an explicit content-change transition that also fires when geometry
   and priority are unchanged. Keep its timing fixed and independent of
   rotation, preserving Rust authority.
3. Add a non-color priority cue and increase `Low` legibility.
4. Make compact and expanded information hierarchy more distinct; test title
   wrapping and type size on the physical macbook.
5. Add reduced-motion and decide/document the VoiceOver behavior.

No queue, priority-order, rotation, or idle-clock architecture change is
recommended. The rough edges are confined to state delivery and presentation.
