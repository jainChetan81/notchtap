# Plan 108: Settings truthfulness — resets hot-apply, actions report their outcome

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: `git diff --stat 870cdeb..HEAD -- src/settings/SettingsApp.tsx src/settings/settings.css`
> — on content mismatch with the excerpts below, STOP.

## Status

- **Priority**: P2 (a control that silently disagrees with the live
  overlay, and five actions whose failures are invisible)
- **Effort**: M
- **Risk**: LOW-MED (SettingsApp.tsx is 1,939 lines; changes are
  additive state + one routing fix, no invoke-command surface change)
- **Depends on**: none in code. Land before 109 (both edit
  SettingsApp.tsx heavily; 109 rebases on this).
- **Category**: bug + UX correctness
- **Planned at**: commit `870cdeb`, 2026-07-22

## Why this matters

From the verified 2026-07-22 external review:

- **A.** Appearance "Reset" (re-load saved) and "Reset to defaults"
  update the form and the preview but never call `set_appearance` — so
  the LIVE overlay keeps the pre-reset appearance until Save &
  Relaunch. Every other appearance mutation hot-applies. Two controls
  that look identical to the user follow different contracts; the
  window lies about the state of the world.
- **B.** Seven operation paths fail silently or console-only:
  send-test, appearance hot-apply, history read, history clear,
  diagnostics read, connector-health read, defaults fetch. History
  read and clear are independently failing operations, not one grouped
  “history path.” Three are empty-bodied
  `.catch(() => { /* comment */ })` handlers (multi-line, comment
  only — NOT literal one-liners; this matters for why no grep can
  gate them, see Done criteria). The
  user clicks "Send test", nothing appears, and there is no way to
  know whether the engine rejected it, the queue is full, or the
  invoke failed. Meanwhile `SecretField` (same file, `:660`) already
  implements the correct pattern — `saving`/`error` state — so the
  repo has an in-file precedent to copy.

## Current state (verified 2026-07-22 at `870cdeb`)

All in `src/settings/SettingsApp.tsx` (1,939 lines):

- `updateAppearance` (`:1579`) — the GOOD path: calls
  `invoke("set_appearance", {scale, radius, opacity})` (`:1589`) AND
  `patchConfig`; its `.catch` is console-only (`:1594`, part of B).
- `applyForm` (`:1692-1698`) — does `setConfig` /
  `setEspnLeaguesText` / `setRssFeedsText` / `setErrors` /
  `setFormGeneration` only. No invoke.
- `resetLoaded` (`:1762`) and `resetDefaults` (`:1766`) — both call
  only `applyForm`. The preview follows because `formGeneration`
  remounts AppearanceSection (plan 027); the live overlay does not.
- Silent handlers:
  - `TestButton.send` (`:1396`): try/catch →
    `console.error("send_test_notification failed:", reason)` (`:1401`).
  - history `refresh` (`:1310`): `.catch(() => {})` (comment at
    `:1312-1314`).
  - `handleClearClick` (`:1330`): console.error (`:1333`) then
    `refresh()`.
  - diagnostics read (`:1238`): `.catch(() => {})` (`:1240-1242`).
  - connector-health read (`:1742`): `.catch(() => {})` (`:1746-1748`).
  - **(added at plan review, 2026-07-22)** `get_default_config` fetch
    (`:1723-1730`): `.catch(() => {})` whose own comment admits the
    consequence — "leave defaults null — Reset to defaults stays
    disabled". On failure the Reset-to-defaults button is disabled
    FOREVER with zero explanation; the user can't distinguish "not
    loaded yet" from "failed". This is the seventh silent operation, and it
    directly feeds Step A's reset flow.
- `SecretField` (`:660`) has real `error`/`saving` state — the
  pattern to imitate.

## Commands you will need

`npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` /
`npx vite build` from the worktree root. Rust untouched (editing
`src-tauri/` is a STOP — no new commands, no build.rs change).

## Scope

**In scope**: `src/settings/SettingsApp.tsx`, its test file(s),
`src/settings/settings.css` (styles for the new inline status UI),
`docs/TESTING_STRATEGY.md` §0 (counts, last).

**Out of scope**: everything under `src-tauri/` (the invoke-command
surface is frozen — `build.rs` + `capabilities/settings.json`
byte-untouched is a done criterion); the overlay entry point;
`preview-overlay.css` (no overlay-mirrored styles here — the new
status styles are settings-only and live in settings.css).

## Steps

### Step A: one appearance mutation path
Route the resets through the hot-apply. **Component-boundary note
(cold-read fix, 2026-07-22)**: `updateAppearance` lives inside
`AppearanceSection` (`:1567`) while `resetLoaded`/`resetDefaults`
live inside the `SettingsApp` component (`:1679`) — 100+ lines and a
component boundary apart, so the shared function cannot be a closure
in either:
1. Define `applyAppearanceLive(scale, radius, opacity)` at MODULE
   scope (it wraps only the `invoke("set_appearance", …)` call —
   `invoke` is already a module import, so no component state is
   needed); `updateAppearance` and both resets call it.
2. `resetLoaded`: after `applyForm(...)`, call `applyAppearanceLive`
   with the appearance values from **`lastLoadedConfig`** (`:1682`,
   used at `:1763` — there is no variable named `savedConfig`; this
   plan previously said so erroneously). `resetDefaults`: same with
   the values from **`defaults`** (`:1766`).
3. Failure of the invoke reports through Step B's status mechanism
   (the reset still applies to the form either way — form state and
   live-apply are separate concerns; say so in a comment).
**Verify**: tests — mock `invoke`; assert `set_appearance` is called
with the saved values on Reset and default values on
Reset-to-defaults; reject each reset's `set_appearance` invoke and
assert the shared visible live-apply error renders while the form still
resets; assert existing hot-apply tests unchanged.

### Step B: visible outcome for every operation
Introduce ONE small reusable mechanism, not five ad-hoc ones:
1. A `useActionStatus()` hook (or equivalent tiny state triple):
   `{state: "idle" | "pending" | "ok" | "error", message?}` with an
   auto-clear timer for `ok` (~2.5s; keep `error` sticky until next
   attempt). Its runner API carries attempt origin explicitly, e.g.
   `run(action, { announce: true | false })`; mixed handlers must not
   rely on one static component prop to remember whether the current
   result came from mount or a button click.
2. An inline `<ActionStatus>` presentational element with an explicit
   `announce` option. When true it owns one `aria-live="polite"` region
   for that action row; when false it renders ordinary discoverable
   status with no live-region attribute. Never create competing live
   regions for one operation.
   **(Corrected at review round 3)** — announcement behavior is chosen
   per ATTEMPT. `aria-live` is for USER-INITIATED attempts only.
   Connector health is always passive; defaults is mount-only passive;
   history and diagnostics each perform a passive mount attempt, while
   history Clear and diagnostics Refresh are interactive. (History has
   no manual Refresh control; do not invent one.) The connector-health read is
   a passive `setInterval(fetchHealth, 5000)` poll
   (verified at `:1751`) — announcing every failed poll would chant
   "Health unavailable" at an AT user every five seconds. Diagnostics
   is mixed: its mount-only `useEffect` attempt is passive and its
   Refresh-button attempt is interactive. History's mount load is
   passive; clear is interactive. Defaults is passive and discoverable
   beside the disabled footer control. Passive attempts render useful
   inline state but carry NO `aria-live`; interactive attempts may
   announce. Connector-health additionally gets the passive-poll rule:
   TRANSITION-ONLY inline status — render
   the state change once when ok→failed or failed→ok flips,
   deduplicate identical consecutive failures, NO `aria-live` on
   the element (it's ambient state, discoverable on navigation, not
   an announcement).
3. Wire it into the seven operations:
   - Send test → pending disables the button; success shows "Queued";
     failure shows the reason string.
   - `updateAppearance` / `applyAppearanceLive` failure → "Live
     preview couldn't update — will apply on Save & Relaunch".
     Slider/segmented-control hot-apply is high-frequency: do not
     announce pending/ok on every adjustment. Render/announce one
     deduplicated error only; the next successful apply clears it.
   - History `refresh` failure → inline "Couldn't load history"
     where the list would render (passive mount attempt, no live
     announcement); `clear` failure → inline error near the button,
     and success → "History cleared" (interactive, polite live).
   - Diagnostics mount-read failure → inline "Couldn't read log
     lines" without live announcement; the same failure from the
     Refresh button is interactive and politely announced.
   - Connector-health read failure → inline "Health unavailable"
     (transition-only, deduped, no aria-live — see the passive-poll
     rule in 2).
   - `get_default_config` failure → inline note by the footer's
     Reset-to-defaults button: "Defaults unavailable — reset
     disabled" (button stays disabled, which is correct; the LABEL
     for why is what's missing today). A retry affordance is optional
     — if trivial (re-run the effect), add it; if not, skip and say
     so.
4. Keep the console.error lines (they cost nothing and help dev), but
   they are no longer the ONLY signal.
**Verify**: independently mock rejection for EACH of the seven
operations — send; appearance apply; history load; history clear;
diagnostics load/refresh; connector health; defaults. History load and
clear MUST be distinct tests with messages in their distinct UI
locations. Diagnostics must test passive mount and interactive Refresh
announcement behavior separately. Assert the inline message renders
and the button (where one exists) disables while pending. Assert
`aria-live` only for an interactive attempt; defaults, connector
health, history mount, and diagnostics mount have none. Assert slider
hot-apply does not announce pending/ok chatter. Assert send-test success
shows then clears; clear success shows "History cleared." For defaults,
rejection renders the explanation next to the disabled button.

### Step C: gates + §0
`npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`,
`npx vite build` → clean. Update §0 counts with attribution.

## Done criteria

- [ ] Both resets invoke `set_appearance` (tests pin values)
- [ ] All seven operations render context-appropriate visible status;
      history load/clear have independent tests and locations; passive
      attempts do not announce; high-frequency appearance changes
      announce only deduplicated errors.
      **(Corrected at review round 2; grep DELETED at cold-read)**:
      the per-path rejection tests are the ONLY gate. The previously
      suggested `grep -n "catch(() => {})"` is vacuous — cold-read
      verified it returns 0 hits TODAY against the unmodified broken
      code (the three silent handlers are multi-line
      `.catch(() => { /* comment */ })` bodies the pattern can never
      match, and the two try/catch paths are invisible to any .catch
      grep). A gate that passes before the fix proves nothing; do
      not reintroduce it.
- [ ] Passive-poll status (connector-health) is transition-only and
      carries NO aria-live; a test drives two consecutive failed polls
      and asserts one state-transition/render callback, then failed→ok
      and one recovery transition (DOM element count alone does not
      prove deduplication)
- [ ] Exactly one status mechanism (hook/component, with explicit
      announce/passive behavior) reused by all seven operations
- [ ] `git diff master -- src-tauri/` → empty
- [ ] All gates clean; §0 matches observed counts; only in-scope files

## STOP conditions

- Any path turns out to need a NEW rust command to report properly
  (it should not — all seven already return promises whose rejection
  carries the failure).
- The `formGeneration` remount (plan 027) interferes with status
  state lifetime in a way that needs restructuring beyond lifting the
  status above the remount boundary.
- Content mismatch against the excerpts above.

## Maintenance notes

- 109 (typography/semantics) will restyle these status elements; keep
  their class names semantic (`.action-status`, `.is-error`…) so 109
  only touches CSS.
- If another silent operation appears later, the hook is the contract:
  no new invoke path ships without an ActionStatus.
