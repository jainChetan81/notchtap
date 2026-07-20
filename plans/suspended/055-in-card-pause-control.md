# Plan 055: SPIKE — should pause get a visible in-card control?

> **Executor instructions**: This is a decision spike, not a build plan.
> The deliverable is a recorded decision (in this file, or a design doc
> under `docs/design/` if the operator wants the fuller spike-doc
> treatment) — do not add an invoke command or touch
> `capabilities/default.json` without that decision being made first.
>
> **Drift check (run first)**: `grep -n "receive-only" DESIGN.html` and
> confirm Law #1 still reads "the overlay is receive-only, forever." If
> that law has already been revised, re-read the current architecture
> before treating this spike's premise as current.

## Status

- **Priority**: P3
- **Effort**: S (decision) / S-M (build, if approved)
- **Risk**: MED — the build side touches a locked architectural
  invariant, not because the feature itself is hard.
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `f58ced2`, 2026-07-19

## Why this matters

Pause already exists — confirmed via direct code read: a global hotkey
(⌃⇧P, `PAUSE_TOGGLE_SHORTCUT`, `src-tauri/src/lib.rs:63-64`), a tray menu
item (`build_tray()`, `lib.rs:646-660`, toggled by `toggle_pause()`,
`lib.rs:609-628`), and a boot-time "Start paused" checkbox in Settings
(`config.rs:14-16,316`; `src/settings/SettingsApp.tsx:628-633`). What
doesn't exist is a pause affordance **inside the overlay card itself** —
an operator watching the overlay with no tray/menu-bar visible (e.g.
fullscreen, or just not thinking to check the tray) has no visual cue
that pause is even possible, let alone a way to trigger it without
memorizing a hotkey.

## Current state — why this isn't a trivial add

`DESIGN.html`'s own documented Law #1: *"The overlay is receive-only,
forever. It listens for `slot-state`, `status-state`, and
`appearance-changed`; it never invokes commands. The settings window is
the only invoke surface."* This is restated as an architectural hard
rule in `CLAUDE.md`'s "ipc & security" section: the overlay's
`capabilities/default.json` "must never change," and `build.rs`'s
`AppManifest::commands` allowlist deliberately scopes every
`#[tauri::command]` away from the main/overlay window by default.

A clickable pause button in the card means one of:

1. **Add a new invoke command reachable from the overlay window** — this
   directly breaks the receive-only law. Every other v5 command
   (`get_config`, `save_config_and_relaunch`, etc.) is scoped to the
   *settings* window specifically for this reason; punching one hole for
   pause means the guarantee "the overlay can never trigger anything" no
   longer holds, and the next feature request that wants an overlay
   button has an easier time arguing precedent.
2. **Keep it hotkey/tray-only, improve discoverability instead** — e.g. a
   status-rail chip (reusing the existing `src-chip` idle-rail pattern,
   `IdleView.tsx:32-38`) that displays "Paused" as a passive indicator
   (already partially true — `status.paused` already reaches the
   frontend, see `useStatusState.ts:106-113`) without being clickable,
   plus surfacing the ⌃⇧P hotkey more visibly somewhere the operator will
   see it before they need it.

## Decision needed (operator)

Pick one:

- **(A) Reopen the receive-only law.** Add a scoped, narrow exception —
  e.g. a single `toggle_pause` invoke command, explicitly justified and
  documented in `ARCHITECTURE.md` next to the existing exception
  language for the settings window, with its own
  `capabilities/overlay-pause.json` (not touching `default.json`) if the
  operator wants the blast radius contained to just that one command.
- **(B) Passive-only.** A non-interactive "Paused" indicator chip when
  `status.paused` is true (the data already flows), hotkey/tray stay the
  only way to actually toggle it.
- **(C) Do nothing** — hotkey + tray is judged sufficient, close this
  plan as rejected.

## Recommendation

(B) is the lowest-risk option that still solves "I can't tell pause
exists" — most of the plumbing (`status.paused`) already exists and
`IdleView.tsx` already has the chip pattern to extend. (A) is only worth
it if the operator anticipates wanting *more* overlay-triggered actions
later (in which case, decide the general policy once, not per-feature).

## Maintenance notes

- If (A) is chosen, this needs its own `docs/design/` spike doc before
  building — reopening a documented law is exactly the kind of decision
  `ARCHITECTURE.md` says shouldn't be relitigated casually.
- If (B) is chosen, this is a small, low-risk build — no plan file
  rewrite needed, just implement directly following the `IdleView.tsx`
  chip pattern.
