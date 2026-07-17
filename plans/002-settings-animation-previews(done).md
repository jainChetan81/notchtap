# Plan 002: Animation previews in the settings window (Appearance section)

> **Status**: DONE — implemented 2026-07-17 alongside Plans 004 and 005.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src/settings/SettingsApp.tsx src/settings/SettingsApp.test.tsx src/settings/settings.css src/components/StatusRailCard.tsx src-tauri/capabilities/settings.json`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts below against the live code before proceeding; on
> a mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction (feature — polish on an already-shipped settings surface)
- **Status**: DONE
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

`docs/V5_TECHNICAL_SPEC.md` (§0, "not in v5" list) explicitly names
"animation previews in the panel" as future work, and says the settings
window is deliberately **sized so they can land later**
(`docs/ARCHITECTURE.md:555`: "the tray can't hold key entry or (future)
animation previews"). The settings window's sidebar (`SettingsApp.tsx:105`)
already has an "Appearance" nav item, rendered `disabled` with a "soon"
badge — this is the one section of the shipped UI that is a stated,
labeled placeholder for exactly this feature.

This plan is scoped deliberately narrow: **a static, prop-driven demo
render** of the existing overlay components inside the settings window, with
no new IPC surface, no new invoke command, and no touching the
receive-only/two-trust-levels boundary the rest of the settings window
infrastructure took real care to build (`docs/ARCHITECTURE.md` §17,
`docs/V5_TECHNICAL_SPEC.md` §2). The settings window already has
`core:event:allow-listen`/`allow-unlisten` reserved for "(future previews)"
(`capabilities/settings.json`) — this plan does **not** use that
reservation, because a static preview needs no live event stream at all.
If a future iteration wants a *live* preview (driven by a real event pushed
from Rust), that reservation is what enables it — but that is explicitly
out of scope here (see Scope).

## Current state

- `src/settings/SettingsApp.tsx` — the `navigation` array (line 93-106) has
  the disabled `"appearance"` entry; `SectionId` (line 59) does not include
  `"appearance"` (it's typed separately as `SectionId | "appearance"` on the
  nav array only, meaning the render switch at the bottom never handles it).
- `src/components/StatusRailCard.tsx` — the actual overlay card component
  this plan will reuse for previews, unmodified.
- `src/useSlotState.ts` — exports the `SlotState` type `StatusRailCard`
  needs as a prop; this plan constructs static instances of this type by
  hand (the same way `src/App.test.tsx` and
  `src/components/StatusRailCard.test.tsx` already do for tests — see
  "Current state" excerpts below).
- `src/settings/settings.css` — where new preview-specific styling belongs;
  `src/styles.css` is the overlay's own stylesheet and must not be imported
  into or duplicated for the settings window (see Scope).

Exact current nav array (`SettingsApp.tsx:93-106`):

```ts
const navigation: ReadonlyArray<{
  id: SectionId | "appearance";
  label: string;
  icon: LucideIcon;
  disabled?: boolean;
}> = [
  { id: "general", label: "General", icon: SlidersHorizontal },
  { id: "football", label: "Football", icon: Trophy },
  { id: "news", label: "News", icon: Newspaper },
  { id: "cmux", label: "Cmux", icon: Terminal },
  { id: "connectors", label: "Connectors & Keys", icon: KeyRound },
  { id: "shortcuts", label: "Shortcuts", icon: Command },
  { id: "appearance", label: "Appearance", icon: Palette, disabled: true },
];
```

Exact current `SectionId` type (`SettingsApp.tsx:59`):

```ts
type SectionId = "general" | "football" | "news" | "cmux" | "connectors" | "shortcuts";
```

Exact current `sectionCopy` map (`SettingsApp.tsx:108-139`) — one entry per
`SectionId`, keyed by the same six strings; this plan adds a seventh.

The render switch that maps `activeSection` to a section component
(`SettingsApp.tsx:910-915`):

```tsx
{activeSection === "general" ? <GeneralSection config={config} patchConfig={patchConfig} /> : null}
{activeSection === "football" ? <FootballSection config={config} leaguesText={espnLeaguesText} patchConfig={patchConfig} setLeaguesText={setEspnLeaguesText} /> : null}
{activeSection === "news" ? <NewsSection config={config} feedsText={rssFeedsText} patchConfig={patchConfig} setFeedsText={setRssFeedsText} /> : null}
{activeSection === "cmux" ? <CmuxSection config={config} patchConfig={patchConfig} /> : null}
{activeSection === "connectors" ? <ConnectorsSection config={config} secretStatus={secretStatus} patchConfig={patchConfig} refreshSecretStatus={refreshSecretStatus} /> : null}
{activeSection === "shortcuts" ? <ShortcutsSection /> : null}
```

The nav button's click handler (`SettingsApp.tsx:876-878`) only calls
`setActiveSection(item.id as SectionId)` when `!item.disabled` — this is why
"Appearance" currently does nothing when clicked. Once you add
`"appearance"` to `SectionId` and remove `disabled: true`, this cast becomes
sound (today it's an unsound cast for the appearance item specifically,
masked by the disabled guard — that's what you're fixing).

`StatusRailCard`'s full prop contract (`src/components/StatusRailCard.tsx:22`,
already read in full during recon — the component takes exactly one prop,
`slot: SlotState`, and is otherwise self-contained apart from importing
`motion/react`, `lottie-react` (via `GoalCelebration`), and
`../lib/presentation`).

`SlotState`'s exact shape (`src/useSlotState.ts:25-40`):

```ts
export type SlotState =
  | { state: "empty" }
  | {
      state: "showing";
      id: string;
      title: string;
      body: string;
      eventType: EventType;   // "generic" | "score_update" | "match_state" | "news_item"
      priority: Priority;     // "low" | "medium" | "high"
      signal: EventSignal;    // "generic" | "goal" | "red_card" | "yellow_card" | "kickoff" | "halftime" | "fulltime"
      expanded: boolean;
      source: string | null;
      category: string | null;
      publishedAtMs: number | null;
      link: string | null;
    };
```

Existing test fixtures to model your sample `SlotState` objects on
(`src/components/StatusRailCard.test.tsx:30-88` — four ready-made examples:
`GOAL`, `RED_CARD`, `CMUX_NEEDS_INPUT`, `NEWS`). Reuse this exact shape and
these exact field combinations for your preview samples rather than
inventing new ones — they're already proven to exercise every visual branch
(goal celebration, red-card pulse, generic cmux card, and the full news-card
layout with category pill + age pill).

The stub pattern every existing frontend test uses for `lottie-react`
(needed because your preview renders `StatusRailCard`, which conditionally
mounts `<GoalCelebration>`, which imports `lottie-react` — jsdom has no
`HTMLCanvasElement.getContext`):

```ts
// StatusRailCard.test.tsx:9
vi.mock("lottie-react", () => ({ default: () => null }));
```

## Commands you will need

| Purpose        | Command                          | Expected on success |
|----------------|-----------------------------------|---------------------|
| TS typecheck   | `npx tsc --noEmit` (repo root)    | exit 0 |
| Frontend tests | `npx vitest run` (repo root)      | exit 0, all pass |
| Frontend build | `npx vite build` (repo root)      | exit 0 |

These are exactly the frontend commands CI runs (`.github/workflows/ci.yml`,
`web` job). This plan touches no Rust code, so `cargo test`/`cargo build`
are not required gates for this plan — but running `cargo test` once at the
end to confirm zero collateral change is good practice (expected: identical
pass count to before this plan).

## Scope

**In scope** (the only files you should modify):
- `src/settings/SettingsApp.tsx` — add `"appearance"` to `SectionId`,
  remove `disabled: true` from the nav entry, add an `appearance` entry to
  `sectionCopy`, add a new `AppearanceSection` component, wire it into the
  render switch.
- `src/settings/SettingsApp.test.tsx` — add test coverage for the new
  section (see Test plan).
- `src/settings/settings.css` — add styling scoped to the new preview
  section only (new class names, no edits to existing rules).

**Out of scope** (do NOT touch, even though they look related):
- `src/components/StatusRailCard.tsx`, `TierCode.tsx`, `Stamp.tsx`,
  `Track.tsx`, `Manifest.tsx`, `IdleView.tsx`, `GoalCelebration.tsx` — reuse
  these components exactly as they are. If you find yourself wanting to add
  a prop or a preview-mode flag to any of them, STOP — that's a sign this
  plan's "reuse via props" approach doesn't fit and needs a design
  conversation, not an unplanned component change.
- `src/styles.css` — the overlay's own stylesheet, loaded only by
  `src/App.tsx`/`src/main.tsx`. Do not import it into the settings bundle
  and do not copy its rules wholesale into `settings.css`. `StatusRailCard`
  and its children rely on class names defined in `styles.css`
  (`.rail-card`, `.compact`, `.tier-code`, etc.) — when rendered inside the
  settings window, those class names will be present in the DOM but
  **unstyled** unless you either (a) import `styles.css` scoped somehow, or
  (b) accept unstyled previews. Resolving this is Step 2 below — read it
  before assuming either direction.
- `src-tauri/capabilities/settings.json`, `src-tauri/build.rs`,
  `src-tauri/src/settings.rs` — no new invoke command, no new IPC
  permission. This preview is 100% static, client-side data.
- `src-tauri/src/lib.rs` — no Rust change of any kind.
- Any change to `Config` or `DEFAULTS` — previews don't read or write
  config.
- `docs/V5_TECHNICAL_SPEC.md` §8's parked appearance-customization item
  (fonts/background/colors/gradients per tab) — that is a *different*,
  larger, explicitly-parked feature ("appearance customization... is parked
  as a future settings section"). This plan is previews-only: showing what
  the shipped animations look like, not letting the user *change* them.
  Do not scope-creep into a color/theme editor.

## Git workflow

- Branch: `advisor/002-settings-animation-previews`
- Commit per logical step (CSS import approach as one commit, section
  component + wiring as a second, tests as a third) — match the repo's
  terse, colon-prefixed style, e.g. `settings: appearance section — static
  overlay animation previews`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Decide and confirm the CSS-scoping approach before writing any component

This is the one real design decision in this plan — read it fully before
touching code.

`StatusRailCard` and its children are unstyled without `src/styles.css`'s
class rules (`.rail-card`, `.rail-card.high`, `.compact`, `.tier-code`,
`.track`, `.manifest`, `.idle-view`, `.masthead`, `.pill`, etc. — over 300
lines of rules). The settings window currently has its own completely
separate stylesheet (`settings.css`, dark-themed but with a different
design language — sidebar nav, `--accent: #0a84ff`, etc.) and the two have
never been loaded together.

**Do this**: add a **scoped import** of `../styles.css` inside
`src/settings/SettingsApp.tsx` (`import "../styles.css";` alongside the
existing CSS import in `src/settings/main.tsx`, or directly in
`SettingsApp.tsx` — vite supports importing CSS from a component file, and
since `settings.html`'s bundle never currently imports `styles.css`, this
is a net-new import, not a duplicate). This is safe because:

- CSS class names in `styles.css` (`.rail-card`, `.compact`, etc.) do not
  collide with any class name in `settings.css` (confirmed via
  `grep -n "^\.rail-card\|^\.stamp\|^\.tier-code\|^\.compact\|^\.track\|^\.manifest\|^\.pill\|^\.masthead\|^\.idle-view" src/settings/settings.css`
  — zero matches; the two stylesheets' class namespaces don't overlap).
- `styles.css`'s top-level `html, body, #root` rules (lines 1-14) *do*
  target the same elements `settings.css` styles (lines 31-43) — both set
  `margin: 0`, but `styles.css` additionally sets `background: transparent`
  and `overflow: hidden` on `body`, which would conflict with
  `settings.css`'s `--window`-colored `background: var(--window)` on body.
  **You must confirm which import order vite applies and verify the
  settings window's own background isn't clobbered** — do this by running
  `npx vite build` and visually inspecting the built CSS order in
  `dist/assets/*.css`, or more reliably: wrap your `styles.css` import in a
  scoping strategy that avoids the collision entirely. The safest concrete
  approach: **do not import the raw `styles.css` file.** Instead, copy only
  the specific classes your preview actually renders (`.rail-card` and its
  priority/expanded/pulse/news-shade/cat-* variants, `.compact`,
  `.compact-hint`, `.tier-code` and children, `.stamp`, `.track` and
  `.track span`, `.manifest` and `.manifest-inner`, `.idle-view` and
  children, `.masthead`, `.pill` and its category/age variants, `.goal-
  celebration`) into a new scoped stylesheet
  `src/settings/preview-overlay.css`, imported only by the new
  `AppearanceSection` component, with every rule prefixed under a wrapper
  class `.appearance-preview` so nothing leaks into the rest of the
  settings window (e.g. `.appearance-preview .rail-card { ... }` instead of
  bare `.rail-card { ... }`). This is more work than a blind import but is
  the only approach that can't silently break the settings window's own
  background/overflow/margin rules — and it matches this repo's existing
  convention of every component owning exactly the CSS it needs (contrast
  with how cleanly `settings.css` and `styles.css` currently don't overlap
  at all).

Read the full `.rail-card` rule tree in `src/styles.css` (already fetched
during recon — it runs from line 16 to line ~360, including `.rail-card`,
`.rail-card.expanded`, `.rail-card.idle`, `.rail-card.low/.medium/.high`,
pulse rules, `.compact`, `.tier-code`, `.stamp`, `.compact-hint`, `.track`,
`.manifest`, `.idle-view`, `.masthead`, `.pill`) before copying — copy the
literal rule bodies verbatim (renaming only the top-level selector prefix),
do not paraphrase or "clean up" values, since visual parity with the real
overlay is the entire point of a preview.

**Verify**: after writing `preview-overlay.css` (Step 3), `npx vite build`
succeeds and the built CSS contains both `.appearance-preview .rail-card`
rules and the existing `settings.css` rules with no reported syntax errors.

### Step 2: Add `"appearance"` to `SectionId` and wire up the nav item

In `src/settings/SettingsApp.tsx`:

1. Change the `SectionId` type (line 59) to:
   ```ts
   type SectionId = "general" | "football" | "news" | "cmux" | "connectors" | "shortcuts" | "appearance";
   ```
2. In the `navigation` array, remove `disabled: true` from the appearance
   entry and simplify its type — since every entry is now a real
   `SectionId`, the array's type annotation (line 93-98) can drop the
   `| "appearance"` union member:
   ```ts
   const navigation: ReadonlyArray<{
     id: SectionId;
     label: string;
     icon: LucideIcon;
   }> = [
     { id: "general", label: "General", icon: SlidersHorizontal },
     { id: "football", label: "Football", icon: Trophy },
     { id: "news", label: "News", icon: Newspaper },
     { id: "cmux", label: "Cmux", icon: Terminal },
     { id: "connectors", label: "Connectors & Keys", icon: KeyRound },
     { id: "shortcuts", label: "Shortcuts", icon: Command },
     { id: "appearance", label: "Appearance", icon: Palette },
   ];
   ```
   Since no entry has `disabled` anymore, you may also simplify the nav
   button JSX (lines 866-884) to drop the `disabled`/`soon-badge` branches —
   but this is optional cleanup, not required for done criteria. If you do
   simplify it, re-verify the "General"/etc. buttons still render and click
   correctly (existing test coverage in `SettingsApp.test.tsx` already
   checks this).
3. Add an `appearance` entry to `sectionCopy` (line 108-139):
   ```ts
   appearance: {
     index: "07",
     title: "Appearance",
     description: "Preview the overlay's built-in animations. Nothing here is configurable yet.",
   },
   ```
4. Wire the render switch (line 910-915) with one more line:
   ```tsx
   {activeSection === "appearance" ? <AppearanceSection /> : null}
   ```
   (component defined in Step 3 — this line will not compile until then;
   write Steps 2 and 3 together if your editor flags the missing symbol.)

**Verify**: `npx tsc --noEmit` → exit 0 once Step 3's component exists.

### Step 3: Write the `AppearanceSection` component

Add this component to `SettingsApp.tsx` (alongside the other
`*Section` components, e.g. after `ShortcutsSection`, before `export
function SettingsApp()`):

```tsx
import { StatusRailCard } from "../components/StatusRailCard";
import type { SlotState } from "../useSlotState";
import "./preview-overlay.css";

type ShowingSlotState = Extract<SlotState, { state: "showing" }>;

// Reuses the exact fixture shapes from StatusRailCard.test.tsx — proven to
// exercise every visual branch (goal celebration, red-card pulse, a
// generic/cmux card, and the full news-card layout with pills).
const PREVIEW_SAMPLES: ReadonlyArray<{ label: string; slot: ShowingSlotState }> = [
  {
    label: "Goal (High priority, football)",
    slot: {
      state: "showing",
      id: "preview-goal",
      title: "GOAL",
      body: "Arsenal 2-0",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
    },
  },
  {
    label: "Red card (High priority, football)",
    slot: {
      state: "showing",
      id: "preview-red-card",
      title: "Red Card",
      body: "Chelsea down to 10",
      eventType: "match_state",
      priority: "high",
      signal: "red_card",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
    },
  },
  {
    label: "Generic alert (High priority, cmux)",
    slot: {
      state: "showing",
      id: "preview-cmux",
      title: "Claude Code needs input",
      body: "Workspace command is waiting",
      eventType: "generic",
      priority: "high",
      signal: "generic",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
    },
  },
  {
    label: "News headline (Low priority)",
    slot: {
      state: "showing",
      id: "preview-news",
      title: "Parliament passes the landmark digital rights bill",
      body: "The measure passed after a late-night vote.",
      eventType: "news_item",
      priority: "low",
      signal: "generic",
      expanded: true,
      source: "NDTV",
      category: "politics",
      publishedAtMs: null,
      link: "https://example.com/digital-rights",
    },
  },
];

function AppearanceSection() {
  return (
    <SettingsGroup
      title="Overlay animations"
      description="These are the built-in card styles the overlay renders. Preview-only — nothing on this page is saved or configurable yet."
    >
      <div className="appearance-preview">
        {PREVIEW_SAMPLES.map(({ label, slot }) => (
          <div className="preview-row" key={slot.id}>
            <div className="preview-label">{label}</div>
            <div className="preview-stage">
              <StatusRailCard slot={slot} />
            </div>
          </div>
        ))}
      </div>
    </SettingsGroup>
  );
}
```

Note `publishedAtMs: null` on the news sample (unlike the test fixture's
real timestamp) — this avoids the age-pill text ("5m ago" in the test)
going stale/wrong relative to whenever someone actually opens this section;
a null timestamp renders no age pill at all (confirmed behavior:
`StatusRailCard.test.tsx`'s "omits category and age pills when news
metadata is null" test proves `publishedAtMs: null` alone, independent of
`category`, suppresses that one pill — check `ageLabel` in
`src/lib/presentation.ts` if you want to confirm the null-handling
yourself, but do not modify that file).

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 4: Write `src/settings/preview-overlay.css`

Create this new file. Copy the rule bodies for exactly these selectors from
`src/styles.css`, each prefixed with `.appearance-preview` (per Step 1's
decision) — do not alter values:

- `.rail-card` (base rule)
- `.rail-card.expanded`
- `.rail-card.idle` (not needed for this plan's four always-`showing`
  samples, but include it for completeness / future-proofing at near-zero
  cost)
- `.rail-card.low`, `.rail-card.medium`, `.rail-card.high`
- `.rail-card.pulse-goal`, `.rail-card.pulse-goal::after`,
  `.rail-card.pulse-red` (and any `::after`/keyframe-linked rule under
  those selectors in `styles.css`)
- `.compact`
- `.compact-hint`
- `.tier-code`, `.tier-code svg`, `.tier-code .code`, `.tier-code
  .tier-label`
- `.stamp`
- `.track`, `.track span`, `.track span.lit`
- `.manifest`, `.manifest-inner`
- `.idle-view`, `.idle-view .time`, `.idle-view .timeline`, `.idle-view
  .timeline::before`
- `.masthead`, `.masthead::after`, `.masthead .dot`
- any `.pill`/`.pill.category`/`.pill.age` rules
- `.goal-celebration` (the wrapper div `GoalCelebration.tsx` renders)
- any `@keyframes` referenced by the above (e.g. `goal-overshoot`,
  `red-alert`) — `@keyframes` rules are global regardless of prefix, so
  copy them as-is with their original names; do not rename them (renaming
  would desync them from the `animationend` handler in `StatusRailCard.tsx`
  which checks for `"goal-overshoot"`/`"red-alert"` by literal string).

Then add this plan's own layout rules for the wrapper/label/stage
(these are new, not copied from anywhere):

```css
.appearance-preview {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.appearance-preview .preview-row {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.appearance-preview .preview-label {
  font-size: 12px;
  color: var(--text-secondary);
}

.appearance-preview .preview-stage {
  display: flex;
  justify-content: center;
  padding: 24px;
  border-radius: 8px;
  background: var(--surface);
  border: 1px solid var(--divider);
}
```

(`var(--text-secondary)`, `var(--surface)`, `var(--divider)` are existing
tokens defined in `settings.css`'s `:root` block — reuse them rather than
hardcoding new colors, matching this file's existing convention of every
other settings component using the same token set.)

**Verify**: `npx vite build` → exit 0, no CSS parse errors reported.

### Step 5: Manual visual check (not machine-verifiable, do once)

Run `npm run dev` is not applicable here (that's Tauri dev mode requiring
the Rust side); instead run `npx vite build && npx vite preview` and open
`http://localhost:4173/settings.html` in a browser, click "Appearance" in
the sidebar, and confirm:
- all four preview cards render with visible styling (not bare unstyled
  HTML) — priority color accents, tier-code icons, track segments all
  visible
- the goal preview shows the lottie celebration animation area (it will be
  visually blank/static in a plain browser since `lottie-react` needs no
  jsdom stub outside tests — if it errors in the browser console, that's a
  real bug to report, not expected)
- clicking between "Appearance" and other sidebar sections doesn't throw
  console errors

This step has no pass/fail command — record what you observed in your
final report to the operator rather than treating it as a blocking gate.

## Test plan

Add one new `describe` block to `src/settings/SettingsApp.test.tsx`,
modeled directly on the existing "renders sidebar navigation and switches
among available sections" test (`SettingsApp.test.tsx:52-83`) and reusing
its `mockLoads()` helper:

```ts
it("Appearance section is enabled and renders all four preview cards", async () => {
  mockLoads();
  render(<SettingsApp />);

  await screen.findByRole("heading", { level: 1, name: "General" });
  const appearanceButton = screen.getByRole("button", { name: "Appearance" }) as HTMLButtonElement;
  expect(appearanceButton.disabled).toBe(false);

  fireEvent.click(appearanceButton);
  expect(await screen.findByRole("heading", { level: 1, name: "Appearance" })).toBeTruthy();
  expect(screen.getByText("Goal (High priority, football)")).toBeTruthy();
  expect(screen.getByText("Red card (High priority, football)")).toBeTruthy();
  expect(screen.getByText("Generic alert (High priority, cmux)")).toBeTruthy();
  expect(screen.getByText("News headline (Low priority)")).toBeTruthy();
  expect(screen.getByText("GOAL")).toBeTruthy();
  expect(screen.getByText("Parliament passes the landmark digital rights bill")).toBeTruthy();
});
```

You must also add the `lottie-react` stub at the top of the test file
(mirroring `StatusRailCard.test.tsx:9` exactly), since `AppearanceSection`
now transitively renders `<GoalCelebration>` for the goal sample:

```ts
vi.mock("lottie-react", () => ({ default: () => null }));
```

Add this `vi.mock` call near the top of `SettingsApp.test.tsx`, alongside
the existing imports (before the `describe` block) — check whether this
file already imports `vi` from `"vitest"` (it does, line 1) so no new
import is needed for the mock call itself.

Also update the existing test that currently asserts `"Appearance soon"` is
the disabled button's accessible name
(`SettingsApp.test.tsx:64`):

```ts
const appearance = screen.getByRole("button", { name: "Appearance soon" }) as HTMLButtonElement;
expect(appearance.disabled).toBe(true);
```

This must change — once the "soon" badge is removed (Step 2, if you took
the optional cleanup) the accessible name becomes just `"Appearance"` and
`disabled` becomes `false`. If you did **not** remove the soon-badge JSX in
Step 2 (kept it optional/skipped), this assertion will still fail because
`disabled: true` is gone from the data even if the JSX branch remains,
since the button's `disabled` attribute is driven by `item.disabled` which
is now `undefined`/falsy for every entry. Update this assertion regardless:

```ts
const appearance = screen.getByRole("button", { name: "Appearance" }) as HTMLButtonElement;
expect(appearance.disabled).toBe(false);
```

**Verification**: `npx vitest run` (repo root) → exit 0, all pass including
the new test and the updated assertion, count increases by exactly 1 new
test.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx vitest run` exits 0; the new "Appearance section is enabled..."
      test exists and passes; no regressions
- [ ] `npx vite build` exits 0
- [ ] `grep -n "disabled: true" src/settings/SettingsApp.tsx` no longer
      matches the appearance nav entry (confirm by also checking
      `grep -n '"appearance"' src/settings/SettingsApp.tsx` shows it in
      `SectionId`, `sectionCopy`, and the render switch)
- [ ] `cargo test` (from `src-tauri/`) exits 0 with an unchanged pass count
      (proves this plan made zero Rust changes)
- [ ] No files outside the in-scope list are modified (`git status --short`)
- [ ] `plans/README.md` status row for `002` updated to `DONE`

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the cited `SettingsApp.tsx` line ranges doesn't match the
  excerpts in "Current state" — re-read the live file; if the section
  component pattern has been restructured (e.g. sections moved to separate
  files, or `SectionId`/`navigation` reshaped), STOP and report rather than
  forcing this plan's exact diff onto a changed structure.
- Copying the `.rail-card` (and related) CSS rules into
  `preview-overlay.css` under the `.appearance-preview` prefix does not
  produce visually correct previews (Step 5's manual check fails) — do not
  start "fixing" the copied CSS by guessing at new values; report exactly
  what looks wrong and let a human decide whether the scoping approach
  itself needs to change.
- You discover `StatusRailCard` (or any child it renders) reads from a
  browser API, context, or hook that behaves differently or errors outside
  the real overlay window (beyond the already-known `lottie-react`/jsdom
  canvas issue this plan's test stub already handles) — e.g. if `useClock`
  (used by `IdleView`, not rendered by this plan's four `showing` samples,
  but worth flagging if you extend this later) or any `motion/react` usage
  throws in the settings window's execution context. This plan's four
  samples are all `state: "showing"`, deliberately avoiding `IdleView`/
  `useClock` for exactly this reason — if you're tempted to add an "idle"
  preview sample, that pulls in `useClock`'s `setInterval`-driven clock
  into a settings-window preview, which is more complexity than a static
  preview needs; treat wanting to add it as a signal to stop and ask,
  not to just add it.
- A verification command fails twice after a reasonable fix attempt.

## Maintenance notes

- The four `PREVIEW_SAMPLES` in `AppearanceSection` are hand-copied from
  `StatusRailCard.test.tsx`'s fixtures, not imported from that file (test
  files aren't meant to be import sources for production code, and vice
  versa). If `StatusRailCard`'s rendering logic changes in a way that adds
  a new visual branch (e.g. a new `EventSignal` or `EventType` variant),
  a reviewer should check whether `AppearanceSection`'s samples need a
  matching new entry to keep the preview page representative — there is no
  automated link enforcing this today.
- `preview-overlay.css` is a **duplicate** of specific rules in
  `src/styles.css`, by design (per Step 1's reasoning) — this is a known,
  accepted maintenance cost: if the overlay's visual design changes (colors,
  spacing, new priority tiers), `preview-overlay.css` must be updated in
  lockstep or the settings preview will silently drift from the real
  overlay's appearance. A reviewer touching `src/styles.css`'s `.rail-card`
  family of rules should grep for `preview-overlay.css` and check whether
  the same change applies there.
- If a future plan wants a **live** preview (driven by an actual event from
  Rust rather than static samples), the settings window's capability file
  already reserves `core:event:allow-listen`/`allow-unlisten` for exactly
  that (`capabilities/settings.json`'s comment: "(future previews)") — that
  future plan would add a new invoke command or reuse the overlay's
  `slot-state` event (scoped to the settings window too) rather than
  starting from scratch.
</content>
