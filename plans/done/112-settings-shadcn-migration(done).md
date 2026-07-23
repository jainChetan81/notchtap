# Plan 112: Migrate settings chrome to Tailwind v4 + shadcn

> Executor instructions: this plan is reviewed now but executed only after
> Plans 109, 110, and 111 are merged, in that order. Follow every step and
> verification gate. On a STOP condition, stop and report; do not improvise.
> The reviewer maintains plans/README.md — do not edit it.
>
> Post-dependency preflight: 109/110/111 are all merged and filed to
> plans/done/; base master is `0b89b3f`. Start from clean master. Record
> PLAN112_BASE=$(git rev-parse HEAD). Inspect all 109–111 changes under
> src/settings/, src/overlay-card.css, package.json, package-lock.json,
> vite.config.ts, tsconfig*.json, and settings.html. Confirm and refresh the
> "Post-111 observed anchors" section against the live tree before editing (it
> already carries measured 0b89b3f values; re-measure and STOP on mismatch).
> STOP if 109–111 are somehow not present or their landed contracts cannot be
> classified below.

## Status

- Priority: P2
- Effort: L
- Risk: HIGH — full settings restyle plus component-generator/CSS-pipeline change
- Depends on: 109 → 110 → 111, all merged; 107/108 are already merged
- Category: UI foundation / tech-debt migration
- Reviewed against: notchtap 0b89b3f (109/110/111 all merged and filed to
  plans/done/) and shared-ui 8e395a8
- Planned at: 2026-07-22; review-plan re-baselined 2026-07-22 at `0b89b3f`

## Intended outcome and boundary

This is a settings-window foundation refactor:

- Generic settings chrome moves from the large hand-written settings.css to
  Tailwind v4 utilities and a small locally owned shadcn component set.
- Palette and motion constants come from the shared-ui token contract.
- Plan 108 feedback behavior, Plan 109 legibility/contrast/native semantics,
  Plan 110 History richness, and Plan 111 preview ownership survive.
- settings.css is deleted only after every rule has a recorded disposition.

This does not redesign or migrate the bespoke overlay. Tailwind utilities and
tw-animate-css handle component-level CSS transitions/keyframes; they do not
replace React Motion AnimatePresence state transitions. The overlay's geometry,
celebrations, and post-111 src/overlay-card.css stay bespoke and out of scope.

The library is a tool, not the product identity. Do not force a primitive swap
when native semantic markup plus utilities is clearer or preserves behavior.

## Why this matters

The settings window has accumulated roughly 1,000 lines of hand-written CSS
and repeated generic control styling. Each UI pass now requires stylesheet
archaeology and risks palette drift. The sibling ../shared-ui repository
provides a tested semantic token layer and a working shadcn playground. This
migration makes future settings work a component/composition change while
keeping notchtap's signature overlay app-owned.

## Post-111 observed anchors — confirm exact values in Step 0

Measured at `0b89b3f` (109/110/111 all merged; base is HEAD of master, NOT
the `d9a62e1` an earlier draft referenced). These are the real anchors, not
provisional placeholders — but line counts and test totals drift with every
commit, so Step 0 still re-measures them and STOPs on a mismatch rather than
trusting these:

- `src/settings/SettingsApp.tsx` — **2,260 lines**. Section registry starts
  at ~line 189 (`{ id: "general", label: "General", ... }`); the sidebar's
  first user-visible section is General.
- `src/settings/settings.css` — **1,276 lines**. This is the chrome sheet to
  classify and delete. It still owns the settings-scoped reduced-motion rule.
- `src/settings/main.tsx` — imports **`../overlay-card.css` first, then
  `./settings.css`** (fixed order, documented in-file by Plan 111). base.css
  must import before both.
- `src/overlay-card.css` — **1,839 lines**, Plan 111's shared card-shape
  sheet. OUT OF SCOPE and must be byte-identical at the end (scope-diff gate).
- `src/styles.css` — 40 lines, overlay residue. Out of scope.
- `src/settings/preview-overlay.css` — **deleted by Plan 111; does not exist.**
  Any reference to it in this plan's older prose is historical.
- Preview host: `SettingsApp.tsx:~1927` renders
  `<div className="preview-stage card-root">`; fixtures live at
  `src/settings/previewFixtures.ts` (exported `PREVIEW_SAMPLES`, imported at
  `SettingsApp.tsx:25`). Card-root ownership and fixtures are Plan 111's
  contract — out of scope.
- `src/settings/SettingsApp.test.tsx` — **≈50 tests**; full frontend baseline
  is **290** across 17 files (per `docs/TESTING_STRATEGY.md` §0). Plan 111 also
  added `src/overlayCardMirror.test.ts` and `src/entryImportOrder.test.ts` —
  both out of scope and must stay green (they guard the overlay/settings CSS
  split this migration must not break).
- `settings.html` — `theme-color` is `#050607`; entry is
  `/src/settings/main.tsx`; there is currently **no** `class="dark"` on
  `<html>` (Step 1.6 adds it).
- Plan 108's one shared ActionStatus mechanism drives footer actions, test
  notifications, Connectors, Diagnostics, History, defaults, connector health,
  and appearance hot-apply. Plan 109 landed the 9px type floor, computed AA
  contrast, 400×480 native minimum, fieldset/legend groups, ul/li lists, and a
  real table. Plan 110 landed rich History metadata/details, escaped non-link
  URL text, robust wrapping, and timestamp rules. All three are preserved
  contracts, not things to re-open.

Step 0's execution report still re-records exact counts, imports, semantic
landmarks, History structure, and section order from the live tree before any
edit — these anchors are the starting map, not a substitute for that pass.

## Shared-ui contract

Reviewed at shared-ui 8e395a8:

- the root package exports design/tokens.css only;
- the working playground uses shadcn CLI 4.13.1, style radix-nova, unified
  radix-ui, Tailwind and @tailwindcss/vite 4.3.3, tw-animate-css, and
  shadcn/tailwind.css;
- components are copied into each consumer and owned locally;
- shared-ui supplies tokens and conventions, not a runtime component bundle.

The original draft's interactive new-york prompt is stale. Use the pinned
playground generator contract and components.json, not mutable latest prompts.

### Portable token snapshot

A dependency on file:../shared-ui works only on this Mac and breaks npm ci in
CI because the sibling checkout does not exist. Use a pinned distributable
snapshot:

1. Create vendor/shared-ui with exactly two files copied verbatim from the
   reviewed shared-ui SHA (8e395a8): the upstream root `package.json` (name
   `@chetanjain/shared-ui`, `"exports": { "./design/tokens.css": ... }` — this
   is what makes the `@chetanjain/shared-ui/design/tokens.css` import resolve)
   and `design/tokens.css`. Copy nothing from `../shared-ui/playground/`.
2. Depend on @chetanjain/shared-ui as file:vendor/shared-ui.
3. The single drift-checked artifact is `vendor/shared-ui/design/tokens.css`
   (package.json is just the manifest that enables resolution — not a token
   file). Record the upstream SHA (8e395a8) and the SHA-256 of the vendored
   `design/tokens.css` in the drift-check script; the SHA-256 must equal that
   of `../shared-ui/design/tokens.css` when the sibling checkout is present.
4. Add `vendor/shared-ui/verify-snapshot.mjs` (Node, no deps; distinct from
   shared-ui's own `scripts/verify-tokens.mjs`, which serves kharcha mirroring
   and is unrelated). Behavior: if `../shared-ui/design/tokens.css` exists,
   compare its SHA-256 to the vendored copy and exit non-zero on any
   difference; if the sibling is absent, print the pinned SHA-256 + upstream
   SHA and exit 0. Do NOT wire it into `npm ci` or the CI build graph — CI has
   no sibling checkout; it is a manual/local drift guard invoked by name. Note
   in the file that `npm ci` self-containment is proven separately by the
   "npm ci with ../shared-ui absent" gate, not by this script.
5. Never edit token values in the vendored snapshot. Refresh it from upstream.

STOP if the snapshot contains playground source/dependencies or npm ci still
requires ../shared-ui. A future published or SHA-pinned remote package can
replace this snapshot in a separate plan.

## Token, type, and motion rules

Plan 109's landed computed contrast/type audit is authoritative:

| intent | utility/token |
|---|---|
| window / sidebar / card | bg-background / bg-sidebar / bg-card |
| primary text | text-foreground |
| secondary/caption text | text-muted-foreground |
| divider | border-border; subtle divider border-border/60 |
| hover/selected | hover:bg-accent / bg-accent |
| primary/focus | bg-primary / text-primary / ring-ring |
| error | text-destructive / border-destructive/40 |
| positive | text-overlay-teal |
| motion | ease-notchtap / duration-fast / duration-normal / duration-slow |

Do not use text-muted-foreground/70 for ordinary text. Opacity composition does
not guarantee contrast. Opacity variants are allowed only for proven disabled
or decorative content listed in the contrast report.

Preserve Plan 109's landed fs-body/fs-secondary/fs-caption/fs-title values.
Bridge them through settings-scoped theme utilities or explicit utilities; do
not silently inherit shadcn's larger default type scale or reopen typography.

Preserve MotionConfig reducedMotion="user" for React Motion. Port the existing
CSS reduced-motion rule to base.css so shadcn/Tailwind transitions and
keyframes are also flattened. Tailwind does not replace application state
motion.

## Commands you will need

Run all from the repo root unless noted. These are this repo's exact gates
(see CLAUDE.md / justfile) — not guesses:

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass (baseline 290, then +focused migration tests) |
| Typecheck | `npx tsc --noEmit` | exit 0, no errors |
| Lint (CI gate) | `npx biome ci .` | exit 0 (note: `biome check .` is the local variant; the gate is `ci`) |
| Build | `npx vite build` | exit 0, settings bundle emitted |
| Locked install / CI self-containment | `npm ci` | exit 0 **with `../shared-ui` renamed aside** |
| shadcn generate | `npx shadcn@4.13.1 add <name>` | component written under `src/components/ui/` |
| Snapshot drift guard | `node vendor/shared-ui/verify-snapshot.mjs` | prints pinned SHA / exits 0, or non-zero on drift |

Rust gates (`cargo test`) are not applicable — `src-tauri/**` is out of scope and
must show an empty scope diff.

## Execution model — who verifies what

This plan mixes machine-checkable gates with visual acceptance. An automated
executor cannot drive Tauri/WKWebView, so the two are split explicitly. Do not
fake, skip silently, or STOP on the operator-owned gates — complete the
executor-owned work, run the executor-owned gates, and hand the rest back.

**Executor-owned (must pass before reporting COMPLETE):**

- `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build`, and
  `npm ci` (with `../shared-ui` absent — rename it aside for the check) all
  clean. These are the exact commands; see the Commands table.
- CSS inventory + preview-equivalence via a **headless-Chromium scratchpad
  harness**, the same technique Plan 111 used to prove overlay equivalence
  (Plan 111's harness lived in the scratchpad, was never committed, and is the
  model to follow). Write it under the session scratchpad, NOT in the repo. Use
  it to: (a) load `settings.css` into `document.styleSheets` and enumerate
  every rule/keyframe/custom-property owner for the Step 0 disposition table
  (this is the "browser CSSOM, not regex" requirement — the tool is headless
  Chromium's CSSOM, driven by Node); (b) capture computed styles + bounding
  boxes for the preview subtree and every control kind before and after, and
  assert zero delta on the preview subtree; (c) confirm no Tailwind preflight
  block and no competing semantic-token values reached the built settings CSS.
- Pixel-diff of the built settings page in headless Chromium within a
  self-measured noise floor (Plan 111's bar), as a supporting signal only.

**Operator-owned (deferred visual sign-off — report as PENDING, do not block
COMPLETE on them, list them explicitly in NOTES):**

- Actual Tauri/WKWebView contact sheets of all ten sections at 480×600 and
  400×480, normal and reduced-motion. WKWebView is not Chromium; headless
  equivalence is necessary but not sufficient, exactly as it was for Plan 111.
- Keyboard traversal, visible focus, and reduced-motion feel in the real
  window. The final WKWebView contact sheet is the acceptance authority; the
  executor's headless evidence is the pre-check that makes that sign-off cheap.

Where a Step or Done-criterion below says "actual Tauri/WKWebView", read it as
operator-owned per this section: the executor produces the headless evidence
and flags the WKWebView pass as pending, rather than treating an un-runnable
GUI step as a STOP condition.

## Scope

In scope:

- post-111 SettingsApp.tsx and settings fixture modules only where chrome
  class plumbing changes
- settings.css, classified and then deleted
- new src/settings/base.css
- src/settings/main.tsx and settings.html
- new src/components/ui/** and src/lib/utils.ts
- SettingsApp.test.tsx plus focused settings migration tests
- package.json, package-lock.json, vite.config.ts, tsconfig.json,
  tsconfig.node.json, and components.json
- new vendor/shared-ui/** and its drift-check script
- plans/112-settings-ui-reference.html if target corrections are needed
- docs/TESTING_STRATEGY.md section 0 once, only after final observed counts

Out of scope:

- plans/README.md
- src/overlay-card.css, src/styles.css, src/App.tsx, src/main.tsx, overlay
  components/hooks/libs, and all src-tauri/**
- ../shared-ui edits or local token-value changes
- changing Plan 109 native semantics, contrast/type floors, or 400×480 minimum
- changing Plan 110 History content/safety contract
- changing Plan 111 preview fixtures, card-root ownership, or presentation
- replacing React Motion state transitions
- Sonner/toast UX
- adopting the Plan 075 Vite/TypeScript/Vitest major-version spike

## Git workflow

- Start from clean post-111 master.
- Create branch 112-settings-shadcn-migration.
- Record PLAN112_BASE and use it for every scope diff.
- Commit per step. Do not push or merge.

## Step 0 — post-111 baseline, inventory, and evidence

1. Record the exact post-111 landmark table and frontend test count. Run
   vitest, tsc, Biome, and Vite build; all must be clean.
2. In actual Tauri/WKWebView, capture all ten sections at:
   - primary 480×600 inner size;
   - minimum 400×480 inner size;
   - normal and reduced-motion preferences.
   Record macOS/WebKit version, device scale, font, and measured inner size.
   Store screenshots/contact sheets outside the repo.
3. Use browser CSSOM, not regex, to inventory every settings.css rule:
   selectors inside grouping rules, keyframes, custom-property owners, and
   reduced-motion rules. Assign one final disposition to each:
   - named shadcn primitive;
   - utility classes at a cited JSX location;
   - named settings-scoped base rule;
   - obsolete with removed markup; or
   - preview/overlay-owned and therefore not removable here.
4. Record computed styles and bounds for body/window/sidebar, every control
   kind, ActionStatus states, diagnostics, History, footer, card-root, card
   assembly, pseudo-elements, and preview bounds.
5. Preserve Plan 109's computed contrast report as the pre-migration floor.

Zero unclassified CSS rules is required before deletion. STOP on a red
baseline, missing dependency, unclassified rule, or unrecognized post-111
topology.

## Step 1 — portable tokens and Tailwind without preflight

1. Create and verify vendor/shared-ui as described above. Run:
   npm install --save-dev file:vendor/shared-ui
2. Install pinned build-only dependencies:
   npm install --save-dev tailwindcss@4.3.3 @tailwindcss/vite@4.3.3
   tw-animate-css@1.4.0 shadcn@4.13.1 @types/node
   This occurs before base.css is created so every imported CSS package is
   locally resolvable and lockfile-pinned during Step 1's build gate.
   `shadcn@4.13.1` is both the generator CLI and the package that resolves
   `@import "shadcn/tailwind.css"`. Do NOT add `@shadcn/react` — the sibling
   playground depends on it as a runtime component bundle, but this plan owns
   components locally (copied by `shadcn add` in Step 2), so that package is
   not a dependency here. The generated components' runtime deps
   (class-variance-authority, clsx, tailwind-merge, lucide-react, radix-ui)
   are added by `shadcn add` in Step 2, not in this step; if `shadcn add` does
   not add them, install the exact versions it prints and pin them.
3. Add tailwindcss() after react() in Vite. Configure the @ alias with
   ESM-safe fileURLToPath(new URL("./src", import.meta.url)); do not use
   "/src" or undefined ESM __dirname.
4. Add baseUrl and @/* → ./src/* to the applicable TypeScript configs.
5. Create src/settings/base.css with the no-preflight contract:

       @import "tailwindcss/theme.css" layer(theme);
       @import "@chetanjain/shared-ui/design/tokens.css";
       @import "tw-animate-css";
       @import "shadcn/tailwind.css";
       @import "tailwindcss/utilities.css" layer(utilities);

   Tailwind preflight is intentionally absent because the settings document
   hosts Plan 111's shared overlay art. Add only explicit settings-window-
   scoped base/reset rules. Port the old reduced-motion rule here. This import
   order is valid only with Step 2's mandatory removal of every generated
   root/dark semantic-token block; verify token ownership both before and
   after shadcn initialization.

   Deliberate divergence from the playground: `../shared-ui/playground/src/
   index.css` uses a single `@import "tailwindcss";` (which pulls in preflight)
   plus an `@layer base { * { @apply border-border … } body { @apply
   bg-background … } }` reset. The settings window CANNOT use that form —
   preflight's global reset would flatten `overlay-card.css`'s bespoke card
   styling that shares this document. The split theme/utilities import above
   (no preflight) is authoritative and WINS over the playground form for the
   Tailwind entry specifically. The playground contract governs `style`
   (radix-nova), token ownership, the generator config, and component shape —
   NOT the Tailwind entry import or the base-layer reset, which are this
   window's own no-preflight decision. Reproduce only the pieces of the
   playground's `@layer base`/`@theme inline` blocks that are safe without
   preflight and scoped away from the preview subtree.
6. Import base.css before temporary settings.css. Add class="dark" to
   settings.html.

Verify:

- test/type/lint/build clean;
- built settings CSS contains representative shared OKLCH values and custom
  utilities;
- the committed shared-ui drift check passes and reports the pinned upstream
  SHA plus matching token-file SHA-256 when the sibling checkout is present;
- no preflight/reset block and no competing semantic token values;
- npm ci succeeds with ../shared-ui temporarily unavailable;
- deterministic computed styles and bounds for the complete preview subtree
  are unchanged;
- actual WKWebView preview pseudo-elements, hit testing, accessibility
  exposure, and geometry remain intact.

Any preview delta or external-path dependency is a STOP.

## Step 2 — pin shadcn v4 and generate only needed primitives

1. Run npx shadcn@4.13.1 init --base radix against the existing Vite app.
   Inspect every write before accepting it. "Reproduce radix-nova" means: the
   pinned CLI accepts `style: "radix-nova"` and emits components in that style
   WITHOUT silently falling back to the default style. The reference artifact
   is the sibling playground's generated components — if
   `../shared-ui/playground/src/components/ui/*.tsx` exists, a generated
   primitive (e.g. button) must match the playground's committed one in its
   cva variant structure and class strings (allowing for the alias/path
   differences below); if the playground has no committed ui components, then
   "reproduced" is proven by components.json carrying `style: "radix-nova"`
   plus the CLI exiting 0 with radix-nova output, not a `default`/`new-york`
   fallback. STOP (per the existing STOP condition) only if the CLI cannot
   emit radix-nova at all.
2. components.json fields — inherit these verbatim from the playground's
   `../shared-ui/playground/components.json`: `style: "radix-nova"`,
   `rsc: false`, `tsx: true`, `tailwind.config: ""` (blank — no v3 config),
   `tailwind.cssVariables: true`, `tailwind.prefix: ""`, `iconLibrary:
   "lucide"`. Override these for THIS app (they legitimately differ from the
   playground and the divergence is intended — "authoritative" means the
   playground defines the *style/generator* contract, not this app's paths):
   `tailwind.css: "src/settings/base.css"` (playground uses `src/index.css`),
   and `aliases` rooted at this app's tree —
   `{ components: "@/components", ui: "@/components/ui", utils: "@/lib/utils",
   lib: "@/lib", hooks: "@/hooks" }` with the `@` → `./src` alias from Step 1.
   `tailwind.baseColor` is immaterial: shared-ui's tokens own all color and
   the generated root/dark blocks are removed in step 3 — if the CLI requires
   a value, `"neutral"` (the playground's) is fine and has no effect. Ignore
   the older shorthand "Radix base / blank Tailwind config / Lucide / @
   aliases" — this field list supersedes it.
3. Remove generated root/dark color blocks. Shared-ui is the sole owner of
   background/card/primary/border/radius/sidebar semantic variables. Keep
   required structural imports only, scoped away from the preview where
   possible.
4. Add exactly: button badge card input textarea switch label separator.
5. Do not add ToggleGroup or ScrollArea:
   - Plan 109's fieldset/button option groups keep their landed focus and
     selection behavior and receive utilities.
   - History keeps Plan 110's native list/details structure and uses native
     overflow utilities.
6. Format generated TS and inspect dependencies. Existing framework/toolchain
   version ranges must not move.

Verify shadcn info, tests, typecheck, build, token ownership, and preview
computed/bounds evidence.

STOP if init creates tailwind.config.*, overwrites existing app files, changes
framework versions, emits unresolved competing tokens, or cannot reproduce
radix-nova with the pinned CLI.

## Step 3 — cross-cutting chrome and Plan 108 feedback

Port before individual sections:

- document/window/frame, sidebar, navigation, headings, scroll container,
  loading state, footer, focus-visible states, and button variants;
- ActionStatus pending/ok/error styling in every location using
  text-muted-foreground, text-overlay-teal, and text-destructive while
  preserving Plan 108's live-announcement and passive-health policy;
- ErrorPanel keeps its motion.div lifecycle and gets Card-equivalent utilities;
  do not force a wrapper swap;
- diagnostics pre, textarea, connector-health note, test-button wrapper,
  footer statuses, and all CSS-in-JSX color literals found by Step 0;
- settings-scoped reduced-motion behavior.

Delete only CSS inventory rows accounted for here.

Verify gates, all seven Plan 108 rejection paths, passive health dedup,
defaults-disabled explanation, actual keyboard traversal, reduced motion, and
unchanged preview presentation.

## Step 4 — sections, one commit and gate each

Order: General → Football → News → Cmux → Weather → Connectors & Keys →
Shortcuts → Diagnostics → History → Appearance.

This is migration/commit order only. It does not change the user-visible
sidebar order, which remains the post-111 order shown by the reference.

Rules:

- preserve post-109 fieldset/legend, ul/li, and real
  table/thead/tbody/th semantics;
- preserve post-110 History metadata/details, escaped non-link URL text,
  wrapping, and timestamps;
- preserve post-111 preview fixtures and card-root;
- keep rotation-order-name and other still-justified test tripwire classes
  until tests are intentionally migrated;
- no raw palette values in JSX/CSS. The settings.html theme-color is the only
  documented boot-chrome literal mirror.

Control mapping:

- Native checkbox Switch → shadcn Switch + Label. This intentionally changes
  input.checked to role=switch plus aria-checked. Update focused tests and
  verify accessible name, Space toggle, disabled behavior, and state.
- NumberControl → Input type=number plus unit. Preserve label, min/max/step,
  invalid states, and value parsing.
- Priority/rotation/appearance exclusive groups → keep landed fieldset/legend
  and button behavior; utilities only, no Radix roving-focus change.
- Rotation order → native ul/li; up/down controls may use Button while
  preserving labels, disabled states, order, and rotation-order-name.
- Connectors → Badge + Input + Button; preserve masking, announcements,
  errors, and health state.
- Shortcuts → valid native table and kbd with utilities + Separator; do not
  wrap it in a component that breaks table validity.
- History → native list/details and native scroll container.
- Appearance preview DOM, custom properties, and shared stylesheet untouched.
- AnimatePresence remains; Tailwind durations/easing style CSS transitions.

After every section run tests/type/build, update the CSS disposition table,
capture 480×600, complete its 400×480 fitness row, verify keyboard/focus, and
re-check preview computed styles/bounds.

## Step 5 — delete settings.css only after proof

1. Every Step-0 CSSOM row must have a completed disposition.
2. Move only classified settings-scoped base/reduced-motion rules to base.css.
3. Delete settings.css and its import.
4. Run final formatting and all gates.

Machine gates:

- no settings.css file/reference and no unclassified rule/keyframe/property;
- vitest, tsc, Biome, Vite build, and npm ci clean;
- focused tests cover Switch semantics, preserved native
  fieldset/list/table/details semantics, ActionStatus, reduced motion, token
  ownership, no preflight, import order, and pinned shadcn config;
- npm ci works without ../shared-ui;
- no tailwind.config.*;
- diff from PLAN112_BASE is empty for src-tauri/**, src/overlay-card.css,
  src/styles.css, and overlay app/components/hooks/libs;
- React/Vite/TypeScript/Vitest ranges unchanged.

Actual Tauri/WKWebView gates:

- all ten sections at 480×600 and 400×480: reachable content, no horizontal
  overflow, intentional wrapping, visible focus, keyboard-operable sidebar and
  controls, readable statuses/errors;
- normal and reduced-motion passes;
- all rendered text/interaction states retain Plan 109's contrast floor;
- Plan 111 preview computed styles, pseudo-elements, accessibility, hit
  testing, bounds, and screenshots have zero unintended delta;
- final contact sheet matches the reference's palette, spacing grammar,
  hierarchy, density, and states.

plans/112-settings-ui-reference.html is a styling grammar, not an exhaustive
DOM arbiter. The actual all-section WKWebView contact sheet at 480×600 and
400×480 is the acceptance authority.

## Test plan

Focused test edits are allowed for intentional contracts. The original zero-
test-edit rule was invalid because shadcn Switch uses aria-checked and existing
click-only tests do not prove keyboard behavior.

Required coverage:

- baseline behavior stays green;
- Switch name/state/Space/disabled behavior;
- fieldset legends, native list, table headers/cells, History details, and
  escaped non-link URL text inherited from 109/110;
- all seven Plan 108 failure paths, passive health dedup, defaults explanation,
  and announcement policy;
- ActionStatus state styling;
- no generated semantic-token owner outside the vendor token snapshot;
- no Tailwind preflight in settings output;
- import order, pinned shadcn config, and preview root contract.

Update TESTING_STRATEGY section 0 once with observed counts after final gates.

## Done criteria

- [ ] 109, 110, and 111 merged; post-111 baseline/landmarks recorded
- [ ] portable shared-ui snapshot pinned and byte-verified; npm ci is sibling-independent
- [ ] no-preflight Tailwind v4 entry and pinned shadcn 4.13.1 radix-nova
- [ ] shared-ui is sole token owner; Plan 109 type/contrast contracts preserved
- [ ] Plan 108 ActionStatus/announcement behavior preserved and styled
- [ ] Plan 109 native semantics and 400×480 fitness preserved
- [ ] Plan 110 rich/safe History contract preserved
- [ ] Plan 111 overlay-card/preview presentation unchanged
- [ ] reduced motion covers CSS primitives and React Motion
- [ ] every legacy CSS rule classified; settings.css deleted
- [ ] tests/type/lint/build/npm ci clean; framework versions unchanged
- [ ] all-section WKWebView contact sheets and keyboard/focus/scroll tables pass

## STOP conditions

Stop and report if:

- dependencies are missing or baseline gates are red;
- post-111 contracts cannot be mapped without reopening 109–111;
- any legacy CSS rule remains unclassified;
- token snapshot differs from reviewed upstream or CI still needs ../shared-ui;
- Tailwind preflight or generated theme variables reach the settings bundle;
- shadcn cannot reproduce radix-nova, creates v3 config, overwrites source, or
  updates framework versions;
- preview/overlay computed style, geometry, pseudo-elements, accessibility,
  hit testing, or WKWebView rendering changes;
- a component swap changes behavior beyond the explicit Switch contract and
  one focused repair fails;
- a section fails 400×480 fit, contrast, focus, keyboard, or reduced motion
  after one honest fix;
- a missing token tempts a local palette value;
- implementation requires an out-of-scope edit.

## Maintenance notes

- shared-ui is upstream; vendor/shared-ui is a pinned distribution snapshot,
  never an independent token source.
- base.css import order, no-preflight policy, and settings scoping are
  load-bearing because settings and overlay preview share a document.
- shadcn components are app-owned after generation. Never overwrite them
  blindly when refreshing.
- Tailwind improves consistency and iteration speed. It does not make
  contrast, semantics, responsive fit, or animation correctness automatic.
- Overlay Tailwind adoption, token publication, and toolchain major upgrades
  are separate decisions.

## shared-ui upstream sync (2026-07-22 evening, appended from the shared-ui session)

shared-ui advanced `8e395a8` → `ca4faf8` before this plan executed. When cutting
the vendored snapshot, pin `ca4faf8` (or later) instead of `8e395a8`. Diff since
`8e395a8` is additive/doc-only — every oklch color value is byte-identical:

- **Font tokens (new, and directly useful here):** tokens.css now declares
  `--font-sans` (default: `-apple-system, BlinkMacSystemFont, "SF Pro Text",
  "Helvetica Neue", sans-serif` — the exact stack settings.css:43 already uses),
  `--font-mono` (`ui-monospace, "SFMono-Regular", Menlo, monospace` — matches the
  legacy mono stack), and `--font-heading`, mirrored in `:root`/`.dark`, bridged
  to `font-sans`/`font-mono`/`font-heading` utilities. Step 0's deletion
  inventory can therefore map the legacy font-family declarations to shared
  tokens instead of carrying them; zero visual change for this app since the
  shared default IS the system stack. (Apps wanting another face override the
  runtime var after the import — transcribe does this for Geist; notchtap does
  nothing.)
- **tokens.css header comment corrected** to the tailwind-first/tokens-second
  import order this plan's base.css already uses; kharcha mirror comment marked
  deferred. Comment-only.
- **Reference install additions:** playground now includes `label` and `table`
  (label is in this plan's Step 2.4 install set — its themed rendering is now
  visually verified upstream).
- **Tooltip generation trap (relevant only if the shadcn `sidebar` is adopted
  for the settings nav later):** newer-generation `sidebar.tsx` assumes
  `Tooltip` self-wraps in `TooltipProvider`; an older-generation `tooltip.tsx`
  alongside it crashes the whole tree at mount. shared-ui fixed its copy
  (playground commit `72bb45e`, Tooltip wraps its own provider). If installing
  `sidebar` + `tooltip` here, verify the pair matches.
- **Extra drift gate available:** shared-ui ships `scripts/verify-tokens.mjs`
  (parse sanity, `:root`/`.dark` mirror, all `@theme` var() refs resolve). It can
  run against the vendored snapshot + base.css as a complement to
  `verify-snapshot.mjs`'s byte check:
  `node ../shared-ui/scripts/verify-tokens.mjs vendor/shared-ui/design/tokens.css src/settings/base.css`.
