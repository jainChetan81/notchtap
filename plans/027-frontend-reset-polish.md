# Plan 027: Appearance section reflects Reset, and App.tsx gets the missing unlisten guard

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- src/settings/SettingsApp.tsx src/settings/SettingsApp.test.tsx src/App.tsx docs/TESTING_STRATEGY.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug (cosmetic) + consistency
- **Planned at**: commit `a58f115`, 2026-07-18
- **Reviewed**: 2026-07-18 at `1add02e` (review-plan pass) — every
  excerpt re-verified against the live tree; `npx vitest run`
  re-confirmed 62/62; Step 2's bug-pin check made stash-free, Step 3
  extended to mirror the `.catch` half of the pattern, working-tree
  guard generalized

## Why this matters

Two small frontend correctness gaps, both confirmed by reading:

1. The settings window's Appearance section seeds its three controls
   (scale/radius/opacity) into local `useState` at mount and never
   re-reads `config.appearance`. "Reset" and "Reset to defaults"
   replace `config` via `applyForm`, so while the Appearance section is
   the active tab, its controls and preview keep showing the pre-reset
   values while the form's actual state (what Save writes) has moved.
   Navigating away and back self-corrects (remount), which is exactly
   the hint for the minimal fix: remount the section when a reset
   happens.
2. `App.tsx`'s `appearance-changed` listener lacks the
   unmount-race guard its sibling in `useSlotState.ts` has: if the
   component unmounts before `listen()`'s promise resolves, the
   unlisten fn is assigned after cleanup ran and the subscription
   leaks. Near-zero impact today (the overlay root never unmounts) —
   this is a consistency fix so the codebase has one pattern, not two.

## Current state

- `src/settings/SettingsApp.tsx` — the settings window (single file,
  sections as components).
  - `AppearanceSection` (line 1024) seeds local state once:

    ```tsx
    function AppearanceSection({ config, patchConfig }: { ... }) {
      const initial = config.appearance;
      const [scale, setScale] = useState(initial.card_scale);
      const [radius, setRadius] = useState(initial.card_radius);
      const [opacity, setOpacity] = useState(initial.card_opacity);
    ```

    Each change calls `updateAppearance`, which invokes
    `set_appearance` (live-applies to overlay + disk) and
    `patchConfig({ appearance: next })`.
  - `applyForm` (line 1147) — the reset/load path:

    ```tsx
    function applyForm(nextConfig: Config) {
      const next = copyConfig(nextConfig);
      setConfig(next);
      setEspnLeaguesText(next.espn_leagues.join("\n"));
      setRssFeedsText(next.rss_feeds.map((feed) => feed.url).join("\n"));
      setErrors([]);
    }
    ```

    Called from the mount loader (line 1167), `resetLoaded`
    (line 1193-1195) and `resetDefaults` (line 1197-1199).
  - The section is rendered at line 1314-1316:

    ```tsx
    {activeSection === "appearance" ? (
      <AppearanceSection config={config} patchConfig={patchConfig} />
    ) : null}
    ```

- `src/App.tsx:24-35` — the unguarded listener:

  ```tsx
  let unlisten: UnlistenFn | undefined;
  listen<{ scale: number; radius: number; opacity: number }>(
    "appearance-changed",
    ({ payload }) => {
      applyAppearance(payload.scale, payload.radius, payload.opacity);
    },
  ).then((fn) => {
    unlisten = fn;
  });
  return () => {
    unlisten?.();
  };
  ```

- `src/useSlotState.ts:100-122` — the exemplar guard to mirror:

  ```tsx
  let unlisten: UnlistenFn | undefined;
  let unmounted = false;
  listen<unknown>("slot-state", ...)
    .then((fn) => {
      if (unmounted) {
        fn();
      } else {
        unlisten = fn;
      }
    })
    .catch((error) => { ... });
  return () => {
    unmounted = true;
    unlisten?.();
  };
  ```

- Test conventions: `src/settings/SettingsApp.test.tsx` uses
  `mockIPC`/`clearMocks` from `@tauri-apps/api/mocks` plus
  testing-library. Two directly relevant existing tests to model on:
  `"Reset restores the values returned by get_config"` (line ~268) and
  `"Reset to defaults applies the defaults served by get_default_config"`
  (line ~281); the Appearance tab is reached by
  `fireEvent.click(screen.getByRole("button", { name: "Appearance" }))`
  (see the test at line ~136). The segmented controls render option
  buttons labeled `Small`/`Medium`/`Large` (scale) and
  `Square`/`Soft`/`Round` (radius) — see `SettingsApp.tsx:1080-1098`.
- Frontend gates: `npx vitest run` (62 tests at planning time;
  re-confirmed 62/62 at review time, `1add02e`), `npx tsc --noEmit`,
  `npx biome ci .`, `npx vite build`. Counts live in
  `docs/TESTING_STRATEGY.md` §0 only. Known benign drift: the drift
  check WILL show `docs/TESTING_STRATEGY.md` changed since `a58f115`
  (the plan-022 deep-testing merge rewrote other sections of that
  file); §0's frontend row still reads 62 and matches the live suite,
  so that diff alone is NOT a STOP — Step 4 recounts from actual
  output regardless.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Frontend tests | `npx vitest run` | all pass (62 + new) |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint/format | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src/settings/SettingsApp.tsx`
- `src/settings/SettingsApp.test.tsx`
- `src/App.tsx`
- `docs/TESTING_STRATEGY.md` (§0 counts)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- The behavior that "Reset" does not undo already-live-applied
  appearance on the overlay/disk (`set_appearance` fires on every
  control change) — that is a deeper design question, deliberately not
  addressed here; this plan only makes the *form display* consistent.
- `src/useSlotState.ts` — already correct; it is the pattern source.
- Any pre-existing uncommitted or untracked working-tree changes —
  concurrent sessions share this checkout and the dirty set varies day
  to day (do not trust any hardcoded list of paths). Record
  `git status --short` BEFORE your first edit; whatever it shows is
  another session's work — never revert, stage, or commit it.
- Lifting scale/radius/opacity fully into `SettingsApp` state — the
  remount approach below is deliberately chosen to avoid re-render
  churn on every slider/segment interaction.

## Git workflow

- Branch: `advisor/027-frontend-reset-polish`.
- Commit style: `settings: appearance section follows reset; overlay: guard appearance unlisten race`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Remount `AppearanceSection` when a reset/load applies

In `SettingsApp`:

1. Add a generation counter next to the other state:
   `const [formGeneration, setFormGeneration] = useState(0);`
2. In `applyForm`, add `setFormGeneration((n) => n + 1);`
3. Key the section: `<AppearanceSection key={formGeneration} config={config} patchConfig={patchConfig} />`

A comment on the `key` is warranted (constraint the code can't show):

```tsx
{/* keyed on formGeneration so Reset/Reset-to-defaults remounts the
    section — its controls seed local state from config.appearance at
    mount and would otherwise show stale values (plan 027) */}
```

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 2: Test it

In `SettingsApp.test.tsx`, add one test modeled on
`"Reset to defaults applies the defaults served by get_default_config"`
(line ~281), e.g.
`"Appearance controls re-seed from config after Reset to defaults"`:

1. Serve a `get_config` fixture whose `appearance.card_scale` is `1`
   (the existing shared fixture already does) and a
   `get_default_config` fixture with `card_scale: 1` as well — so make
   the *loaded* config non-default for the test: serve `get_config`
   with `appearance: { card_scale: 1.15, ... }` or click a control
   after render instead (simpler: render with the standard fixtures,
   click the `Large` scale option, which sets local+config scale to
   1.15 via `set_appearance`/`patchConfig` — `mockIPC` must stub
   `set_appearance` to resolve).
2. Open the Appearance tab, click `Large`, assert `Large` is the
   selected segment. Verified markup (`SettingsApp.tsx:913`,
   `SegmentedControl`): each option renders as a `<button>` with
   `aria-pressed={value === option.value}` plus an `is-selected`
   class when selected — assert via `aria-pressed` (e.g.
   `getByRole("button", { name: "Large" }).getAttribute("aria-pressed")`
   → `"true"`). Scope queries to the Appearance section with
   `within(...)` the way the existing test at line ~157 does (the
   sidebar also has an "Appearance" button).
3. Click `Reset to defaults`, and assert the scale segment shows
   `Medium` (the default `card_scale: 1`) again — this fails without
   Step 1 and passes with it.

**Verify**: `npx vitest run` → all pass including the new test. Then
confirm the test actually pins the bug WITHOUT touching git state
(concurrent sessions share this tree — no `git stash`): temporarily
edit the render site's `key={formGeneration}` to `key={0}`, run
`npx vitest run -t "Appearance"` → the new test must FAIL; restore
`key={formGeneration}` and run again → passes.

### Step 3: Mirror the unlisten guard in `App.tsx`

Apply the `useSlotState.ts` pattern to the `appearance-changed`
listener — BOTH halves of it: add `let unmounted = false;`, in `.then`
call `fn()` immediately when `unmounted`, else assign; set
`unmounted = true` in cleanup; and also mirror the `.catch` (plan 009
added it to `useSlotState` — a silently dead listener here means
appearance changes never live-apply to the overlay):

```tsx
.catch((error) => {
  console.error("appearance-changed listener failed to register", error);
});
```

Keep the effect's existing seed logic
(`window.__NOTCHTAP_APPEARANCE__`) untouched. No new test — the
existing `App.test.tsx` listen-mock resolves synchronously, so the race
isn't observable there; the value is pattern consistency (and the
mirror is itself the tested pattern in `useSlotState.test.ts`).

**Verify**: `npx vitest run` → all pass; `npx biome ci .` → exit 0.

### Step 4: Reconcile counts

`docs/TESTING_STRATEGY.md` §0: frontend total +1 (settings-form file
count +1). Recount from the actual vitest output.

**Verify**: `npx vitest run 2>&1 | tail -3` matches §0.

## Test plan

- New: 1 test in `SettingsApp.test.tsx` (Step 2), pattern: the two
  existing Reset tests at lines ~268/~281.
- Existing that must stay green: all 62, especially the Appearance
  preview-cards test (~line 136) and both Reset tests.
- Verification: `npx vitest run` → all pass.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npx vitest run` exits 0; one new Appearance-reset test present
- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx biome ci .` exits 0
- [ ] `npx vite build` exits 0
- [ ] `grep -n "unmounted" src/App.tsx` → guard present in the appearance effect; `grep -n "failed to register" src/App.tsx` → the `.catch` mirror present
- [ ] `grep -n "formGeneration" src/settings/SettingsApp.tsx` → state + `key` usage present
- [ ] `docs/TESTING_STRATEGY.md` §0 matches actual counts
- [ ] `git status --short` shows, beyond the entries you recorded before your first edit (other sessions' work — untouched, unstaged), modifications ONLY to in-scope files
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `AppearanceSection`/`applyForm` no longer match the excerpts.
- The new test cannot observe the segmented control's selected state
  through the existing markup (don't add test-ids to production markup
  without reporting first — there may be an existing selected-state
  idiom you missed; search `SegmentedControl` usage in the test file).
- Fixing the display requires touching the `set_appearance` live-apply
  flow — out of scope, report instead.

## Maintenance notes

- If a future change lifts appearance state fully into `SettingsApp`
  (e.g. to make Reset also revert the live overlay), delete the
  `formGeneration` key mechanism — it exists only because the section
  holds mount-seeded local state.
- Reviewer should check: the `key` only changes on `applyForm` calls
  (load + both resets), not on every `patchConfig` — otherwise every
  appearance click would remount the section and drop focus.
- Deferred: making Reset revert the on-disk/live appearance (design
  question — Reset currently only resets the *form*; `set_appearance`
  changes were already applied live). Raise with the maintainer if it
  surprises users.
