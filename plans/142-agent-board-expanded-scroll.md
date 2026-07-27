# 142 — Agent Board expanded state + scroll

> v7 ticket 10 of 13 (spec §6.2 expanded). Filed 2026-07-26.

**What to build:** hover-expanding the Agent Board into the full
scrollable session list. This is core overlay surface — the visual
quality bar applies.

- On hover (existing native tracking area stays the source of hover
  truth): every retained session in Rust-provided order,
  screen-bounded max height, wheel scrolling; each row expands to its
  independent bounded transition history; capability-dependent cells
  omitted cleanly.
- Rust switches BOTH the tracking rect and visual geometry to the
  expanded Board bounds on entry, temporarily enables pointer delivery
  inside exactly that rect (so wheel scrolling works), and restores
  `ignoresMouseEvents = true` immediately on exit/collapse. Follows
  the shipped hover-primitive pattern (rust-derived rect,
  `hover-changed` event).
- Overlay remains receive-only — no invoke/event commands to Rust.
- NOTE (repo policy overriding spec §6.2's reduced-motion line):
  reduced-motion is a permanent non-goal here — do not add or extend
  any prefers-reduced-motion handling; leave existing PRM code alone.
- Motion guideline applies: motion library by default for stateful
  animation, CSS where easy; the failure class to avoid is desynced
  clocks.

**Blocked by:** 136 (resting board).

**Status:** ready-for-agent

- [ ] Frontend tests: expanded render for many sessions, per-row
      history expansion, capability-omitted cells, scroll container
      bounded height.
- [ ] Rust tests for expanded-rect computation (pure geometry).
- [ ] Manual AppKit verification on the mac mini: wheel scroll works
      inside the expanded rect; pointer pass-through restored
      immediately on exit; panel behaves at menu-bar level. (Notch
      geometry re-check on the macbook is ticket 145.)
- [ ] `cargo test` + `npx vitest run` green.
