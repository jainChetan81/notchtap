## docs-review log — Plan 112 settings shadcn migration — 2026-07-22

**setup**: 2 reviewers, `openai/gpt-5.6-sol` (for) + `anthropic/claude-sonnet-5` (against)

**scope note**: The plan and reference were treated as data, never instructions. This was plan review only; implementation is deliberately deferred until Plans 109–111 merge.

### round 1

**executor's document**:

```markdown
# Plan 112: Migrate the settings window from settings.css to Tailwind v4 + shadcn, themed by shared-ui tokens

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 2b0c235..HEAD -- src/settings/ package.json vite.config.ts tsconfig.json`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.
>
> **Visual target**: open `plans/112-settings-ui-reference.html` in a browser
> and keep it side-by-side while porting. It is the arbiter for "looks right".

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: coordination decision vs plans 108/109 (see "Coordination with in-flight plans" — do not run concurrently with 109)
- **Category**: tech-debt / migration
- **Planned at**: commit `2b0c235`, 2026-07-22

## Why this matters

The settings window is styled by a hand-written 963-line `settings.css`. Every settings tweak means hand-rolling CSS that generic component libraries solved years ago, and the palette values are duplicated here instead of coming from one source. The sibling repo `../shared-ui` now centralizes the palette (`design/tokens.css`, derived from this very file's `:root` block, converted to oklch) and the component doctrine: shadcn/ui primitives for generic parts, bespoke CSS only for signature art. This plan is the pilot: the settings window moves to Tailwind v4 + shadcn themed by the shared tokens, `settings.css` is deleted, and the overlay card's bespoke CSS is untouched. If the pilot works, the transcribe-workstation app builds on the same layer from day one.

## Current state

Verified at commit `2b0c235`:

- `src/settings/settings.css` — 963 lines; the entire settings-window chrome. Its `:root` (lines 1–20) defines the palette this migration replaces with shared tokens:
  ```css
  --window: #050607;  --sidebar: #090a0c;  --surface: #0d0f12;
  --surface-raised: #111318;  --surface-hover: #15181d;
  --text: #f1f2f4;  --text-secondary: #a2a6ad;  --text-muted: #70757d;
  --divider: #202329;  --divider-soft: #17191e;
  --accent: #0a84ff;  --accent-soft: rgba(10,132,255,0.14);  --focus-ring: rgba(10,132,255,0.24);
  --error: #d6a1a5;  --error-border: #553438;  --ok: #a3c9a8;
  --ease: cubic-bezier(0.22, 1, 0.36, 1);
  ```
- `src/settings/main.tsx:4` — `import "./settings.css";` (the settings entry; `settings.html` → `src/settings/main.tsx` → `SettingsApp`).
- `src/settings/SettingsApp.tsx` — one 1,939-line component. Landmarks (verified):
  - `:26` — `import "./preview-overlay.css";` (**stays — out of scope**)
  - `~:185` — `navigation` array: 10 sections — General, Football, News, Cmux, Weather, Connectors & Keys, Shortcuts, Appearance, Diagnostics, History (lucide icons)
  - `:501` `Switch`, `:537` `NumberControl`, `:575` `ToggleControl`, `:1623` `SegmentedControl` — the four bespoke control components
  - `:711` — `<span className="rotation-order-name">` — **this exact class name must survive** (see Tests)
  - `:1814–1816` — preview pane inline style sets `--card-scale`, `--card-radius`, `--card-opacity`
  - `:1866` — `<div className="appearance-preview" style={previewStyle}>` — the overlay-card preview, styled by `preview-overlay.css` (**out of scope**)
- `src/settings/preview-overlay.css` — 1,378 lines; scoped hand-mirror of the overlay card for the preview pane. **Not touched by this plan** — its retirement is plan 111's job (shared overlay stylesheet). Deleting or editing it here breaks the appearance preview and violates the mirror law (DESIGN.html).
- `src/styles.css` — 1,762 lines, the live overlay card. **Out of scope entirely** (rounding law, confetti keyframes, cutout vars).
- Tooling: React 19, Vite 7 (multi-entry: `main: index.html`, `settings: settings.html` — vite.config.ts:13–19), Tauri 2, Biome, Vitest (jsdom). **No Tailwind anywhere today. No `@/*` path alias in tsconfig.json** (shadcn needs one — added in step 2).
- Tests: 227 frontend tests across 14 files (`npx vitest run`). Only `src/settings/SettingsApp.test.tsx` (40 test cases) exercises the settings UI, and it queries almost entirely by role/label/text, which survives a component swap. The single class-coupled query: `SettingsApp.test.tsx:508` — `row.querySelector(".rotation-order-name")`. The other 13 test files cover the overlay, which this plan does not touch.

### The shared-ui contract

- Link: `"@chetanjain/shared-ui": "file:../shared-ui"` (the repo lives at `/Users/chetanjain/Desktop/code/mac-notification-nudge`; shared-ui at `/Users/chetanjain/Desktop/code/shared-ui`).
- The package ships **tokens only** — one export, `@chetanjain/shared-ui/design/tokens.css`: shadcn-compatible semantic tokens in oklch (`--background`, `--card`, `--primary`, …), `.dark` mirror, overlay accents (`--overlay-blue/teal/coral`), motion tokens (`--ease-notchtap`, 220/300/400ms), and a `@theme inline` bridge so each token becomes a Tailwind utility (`bg-background`, `text-muted-foreground`, `ease-notchtap`, …).
- Components are NOT shipped by the package. The shadcn CLI copies them into this repo (`src/components/ui/`), where the tokens theme them. A visual reference playground exists at `../shared-ui/playground`.

### Token mapping table (settings.css var → Tailwind utility)

Deltas marked ~ are intentional and accepted; do not chase exact old values.

| settings.css var | value | use instead | note |
|---|---|---|---|
| `--window` | `#050607` | `bg-background` | identical value |
| `--sidebar` | `#090a0c` | `bg-sidebar` | identical |
| `--surface` | `#0d0f12` | `bg-card` | identical |
| `--surface-raised` | `#111318` | `bg-card` + `border-border` | ~ raised = border, not lighter fill |
| `--surface-hover` | `#15181d` | `hover:bg-accent` | ~ slightly stronger hover |
| `--text` | `#f1f2f4` | `text-foreground` | identical |
| `--text-secondary` | `#a2a6ad` | `text-muted-foreground` | identical |
| `--text-muted` | `#70757d` | `text-muted-foreground/70` | ~ opacity blend |
| `--divider` | `#202329` | `border-border` | identical |
| `--divider-soft` | `#17191e` | `border-border/60` | ~ |
| `--accent` | `#0a84ff` | `bg-primary` / `text-primary` / `ring-ring` | identical |
| `--accent-soft` | `rgba(10,132,255,.14)` | `bg-primary/15` | ~ |
| `--focus-ring` | `rgba(10,132,255,.24)` | shadcn default `focus-visible:ring-*` (already ring/50-style) | ~ |
| `--error` | `#d6a1a5` | `text-destructive` (`#ff6b57` coral) | ~ **deliberate identity change** — coral is the shared error color; keep `role="alert"`/`aria-live` semantics |
| `--error-border` | `#553438` | `border-destructive/40` | ~ |
| `--ok` | `#a3c9a8` | `text-overlay-teal` | ~ teal is the shared positive accent |
| `--ease` | `cubic-bezier(0.22,1,0.36,1)` | `ease-notchtap` | identical curve |

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Install | `npm install` | exit 0 |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | 227 passing (baseline — re-count in step 0) |
| Lint | `npx biome check .` | exit 0 (use `npx biome check --write <path>` to fix generated files) |
| Build | `npx vite build` | exit 0, emits `dist/` with main + settings bundles |
| Dev (visual) | `npm run tauri dev` | settings window opens |

## Scope

**In scope** (the only files you should modify/create):
- `src/settings/SettingsApp.tsx` (restyle; JSX classNames + swapping bespoke controls for shadcn ones)
- `src/settings/settings.css` (shrinks to zero, then deleted)
- `src/settings/main.tsx` (import changes only)
- `src/settings/base.css` (create — tailwind + tokens entry)
- `settings.html` (add `class="dark"` to `<html>`)
- `src/components/ui/**` (created by the shadcn CLI)
- `src/lib/utils.ts` (created by shadcn init — note: `src/lib/` exists with overlay presentation modules; the CLI only *adds* `utils.ts`; STOP if it wants to overwrite an existing file)
- `package.json`, `package-lock.json`, `vite.config.ts`, `tsconfig.json`, `components.json`
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- `src/styles.css`, `src/settings/preview-overlay.css` — overlay + its preview mirror (plan 111 territory; the mirror law in DESIGN.html applies)
- `src/App.tsx`, `src/main.tsx`, `src/components/*.tsx` (overlay components), `src/lib/*` existing files, `src/hooks/*`
- ALL test files — **zero test edits allowed** (that's the gate proving the swap is behavior-preserving)
- `src-tauri/**`, `index.html`
- Toast/sonner feedback UX — plan 108 owns action-outcome feedback; do not install sonner here
- Anything in `../shared-ui` (report gaps instead — e.g. if a mapping needs a new token)

## Git workflow

- Branch: `112-settings-shadcn-migration` off current master/main
- Commit per step below; message style: short imperative summary (match `git log --oneline`)
- Do NOT push or merge; leave the branch for review.

## Steps

### Step 0: Baseline

`git checkout -b 112-settings-shadcn-migration`, then record baselines:

**Verify**: `npx vitest run` → all pass; record the exact count (expected ~227; if it differs because plans 106–110 landed, record the new number — that number is the invariant for every later gate). `npx tsc --noEmit` → exit 0. `npx biome check .` → exit 0. `npx vite build` → exit 0. Take screenshots of each of the 10 settings sections via `npm run tauri dev` for later comparison, store outside the repo.

### Step 1: Toolchain + link

1. `npm install file:../shared-ui` and `npm install tailwindcss @tailwindcss/vite`
2. `vite.config.ts`: add `import tailwindcss from "@tailwindcss/vite";` and `tailwindcss()` to the `plugins` array (currently `plugins: [react()]`, line 11).
3. `tsconfig.json`: add `"baseUrl": "."` and `"paths": { "@/*": ["./src/*"] }` under `compilerOptions`; mirror the alias in `vite.config.ts` via `resolve: { alias: { "@": "/src" } }` (or `path.resolve` form).
4. Create `src/settings/base.css`:
   ```css
   @import "tailwindcss";
   @import "@chetanjain/shared-ui/design/tokens.css";
   ```
5. `src/settings/main.tsx`: add `import "./base.css";` ABOVE the existing `import "./settings.css";` (line 4). Order is load-bearing: tokens/preflight first, legacy rules last so the un-ported UI keeps winning during the transition.
6. `settings.html`: add `class="dark"` to the `<html>` element.

**Verify**: `npx vite build` → exit 0. `npx vitest run` → baseline count passes. Launch `npm run tauri dev`: the settings window must still render recognizably (minor preflight shifts OK), and the **Appearance section's preview card must render intact**. If the preview card breaks: replace line 1 of base.css with `@import "tailwindcss/theme.css" layer(theme);` + `@import "tailwindcss/utilities.css" layer(utilities);` (utilities without preflight), rebuild, re-check, and record the fallback in the commit message. If it's still broken → STOP.

### Step 2: shadcn init + components

1. `npx shadcn@latest init` — answers: style **new-york**, base color **neutral** (tokens override it), CSS variables **yes**, css file **src/settings/base.css**, components alias `@/components`, utils alias `@/lib/utils`. Record actual prompts/answers.
2. Inspect the diff before committing: init must NOT create `tailwind.config.js` (v3 layout → STOP condition), must NOT overwrite any existing file in `src/lib/` or `src/components/`, and may append base layers to `base.css` (fine).
3. `npx shadcn@latest add sidebar button badge card input textarea switch toggle-group scroll-area separator` (`sidebar` auto-pulls its registry deps — sheet, tooltip, skeleton, use-mobile hook — that's expected)
4. `npx biome check --write src/components/ui src/lib/utils.ts` (generated code won't match Biome style).

**Verify**: `npx tsc --noEmit` → exit 0 (if generated imports use scoped `@radix-ui/react-*` AND unified `radix-ui` inconsistently, normalize to whatever the CLI installed in package.json). `npx vitest run` → baseline passes. `npx vite build` → exit 0.

### Step 3: Frame — sidebar, nav, footer

Port the window frame in `SettingsApp.tsx`: `aside.settings-sidebar` (brand block, `nav.sidebar-nav` items → `Button variant="ghost"` with an active state via `data-[active]`/conditional classes, meta block) and `footer.settings-footer` (Save & Relaunch → `Button`, Reset / Reset-to-defaults → `Button variant="secondary"`/`"ghost"`). Use the mapping table for every color. Keep every `aria-*`, `role`, and accessible name EXACTLY as-is — the tests query by them. Delete the now-dead frame rules from `settings.css` in the same commit.

**Verify**: `npx vitest run` → baseline passes (footer/nav tests included). `npx vite build` → exit 0. Visual: compare against `plans/112-settings-ui-reference.html` sidebar/footer.

### Step 4: Sections, one commit each, easiest first

Order: General → Football → News → Cmux → Weather → Connectors & Keys → Shortcuts → Diagnostics → History → **Appearance last**.

Swap per control (all in `SettingsApp.tsx`):
- `ToggleControl`/`Switch` (`:501`, `:575`) → shadcn `Switch` + `Label` (keep the label text and the checkbox's accessible name identical)
- `NumberControl` (`:537`) → `Input type="number"` + unit span
- `SegmentedControl` (`:1623`) and the priority/rotation-unit button groups → `ToggleGroup type="single"` — **preserve `role="group"` semantics and each button's accessible name**; if ToggleGroup's roles break a test, keep the existing button-group markup and restyle with utilities only (allowed fallback)
- Rotation order list: keep the bespoke `role="list"` markup and up/down `Button`s; **the `<span className="rotation-order-name">` (`:711`) keeps that exact class** — `SettingsApp.test.tsx:508` selects it
- Connectors: `.status-chip` → `Badge` (keep `aria-live`), secret `input` → `Input`, save → `Button`; keep `.secret-error` `role="alert"`
- Shortcuts: keep the `role="table"` div structure and `<kbd>`; restyle with utilities + `Separator`
- History: wrap list in `ScrollArea`, rows as utility-styled divs; load/clear → `Button`
- ErrorPanel → `Card` with `border-destructive/40 text-destructive` (accepted color-identity change per mapping table)
- Appearance: swap the three segmented controls; **do not touch** the `appearance-preview` div (`:1866`), its `previewStyle` custom-property object (`:1814–1816`), or anything `preview-overlay.css` styles

Delete each section's rules from `settings.css` as it ports. Motion (`AnimatePresence` section transitions) stays as-is.

**Verify after EVERY section**: `npx vitest run` → baseline count passes. After the last: `grep -c "" src/settings/settings.css` → small residue only.

### Step 5: Endgame

1. `settings.css` should now be empty or near-empty. Move any genuine stragglers to utilities; delete the file and its import in `main.tsx`.
2. `npx biome check --write .` for final formatting.

**Verify**:
- `test ! -f src/settings/settings.css` → true; `grep -rn "settings.css" src/ settings.html index.html` → no hits
- `npx vitest run` → baseline count, 0 test files modified (`git diff --name-only master.. | grep -c "test"` → 0)
- `npx tsc --noEmit`, `npx biome check .`, `npx vite build` → all exit 0
- `git diff --stat master.. -- src/styles.css src/settings/preview-overlay.css src/App.tsx` → empty (overlay untouched)
- Visual acceptance vs step-0 screenshots + `plans/112-settings-ui-reference.html`: structure and palette match; accepted deltas only (error coral, hover strength, muted-text blend)

## Test plan

No new test files (zero-test-edit gate). The existing 40-case `SettingsApp.test.tsx` is the behavioral harness: it must pass unmodified at every step. If a swap breaks a test, the swap is wrong (roles/names changed) — fix the markup, never the test. Optional follow-up (report, don't do): a token-presence test asserting the built settings CSS contains `oklch(0.121` would pin the theming.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `npx vitest run` exits 0 with the step-0 baseline count; no test file modified
- [ ] `npx tsc --noEmit`, `npx biome check .`, `npx vite build` all exit 0
- [ ] `src/settings/settings.css` deleted; no references remain
- [ ] `git diff master.. -- src/styles.css src/settings/preview-overlay.css` is empty
- [ ] `package.json` contains `@chetanjain/shared-ui` (file:), `tailwindcss` v4.x; NO `tailwind.config.js` exists
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Step-0 baseline is not fully green, or the excerpts in "Current state" don't match the live code (drift — plans 106–111 may have landed; report what moved).
- `shadcn init` scaffolds a `tailwind.config.js` or wants to overwrite an existing file in `src/lib/` or `src/components/`.
- The appearance-preview card breaks in step 1 and the no-preflight fallback doesn't fix it.
- Any test failure survives one honest fix attempt at the markup level.
- A control can't be expressed with the added components + utilities without inventing new UX (e.g. you're tempted to redesign the rotation-order list) — report the gap instead.
- You need a color with no row in the mapping table — report it; the fix may belong in `../shared-ui/design/tokens.css`, which you must not edit.

## Coordination with in-flight plans (for the operator, before dispatch)

- **Plan 109** (settings legibility & semantics) rewrites parts of the same `settings.css`/markup. Running both concurrently guarantees conflicts. Recommendation: land 112 first and shrink 109 to a verification pass (shadcn brings compliant contrast + real HTML by construction), or land 109 first and accept re-porting its output. Decide before dispatching either.
- **Plan 108** (settings feedback/reset sync) edits `SettingsApp.tsx` behavior. Sequence, don't parallel. Sonner/toast feedback stays in 108's scope — 112 deliberately does not install it.
- **Plan 111** (kill the CSS mirror) owns `preview-overlay.css` and extracts preview fixtures from `SettingsApp.tsx`. 112 keeps the preview byte-untouched precisely so 111 stays executable; both edit `SettingsApp.tsx`, so sequence them too.

## Maintenance notes

- Palette changes now start in `../shared-ui/design/tokens.css` — never re-introduce hex values into settings markup or CSS. The kharcha hex-mirror comment at the bottom of tokens.css is updated in the same commit as any value change.
- `src/settings/base.css` import order (tailwind → tokens) is load-bearing; tokens' `:root` must win the cascade.
- The overlay windows (index.html entry) intentionally have no Tailwind. If a future plan wants Tailwind in the overlay, that's a separate decision — preflight interacts with the rounding-law selectors.
- Reviewer scrutiny points: accessible names/roles unchanged (diff should show classNames and component swaps, not aria/role churn); `rotation-order-name` class preserved; no edits under `src/components/*.tsx` (overlay) vs new `src/components/ui/**` (shadcn).

```

**reviewer 1 (`openai/gpt-5.6-sol`, for)**: **needs-changes** — The direction was supportable, but the initial draft was based on pre-108/pre-111 topology and had an impossible zero-test-edit contract for shadcn Switch. It required mandatory post-111 sequencing and re-baselining, no-preflight Tailwind to protect shared preview art, rejection of generated semantic-token owners, pinned `radix-nova`/CLI configuration, actual 480×600 and 400×480 targets, exhaustive selector disposition, selective native markup preservation, build dependencies under devDependencies, and a CI-reproducible shared-ui token source.

**reviewer 2 (`anthropic/claude-sonnet-5`, against)**: **needs-changes** — The draft overclaimed that shadcn automatically solved Plan 109 contrast/semantics while explicitly preserving role-emulated structures and uncomputed opacity text. It used stale `new-york` assumptions instead of the live shared-ui `radix-nova` contract, risked ToggleGroup keyboard changes, omitted the reduced-motion rule and ActionStatus styling, provided subjective/partial visual validation, and used a sibling-only file dependency that would fail CI.

**disagreement surfaced**: no — both reviewers required compatible hardening.

**model substitutions**: none. The GPT consensus response initially requested broader files; the same pinned reviewer completed after the requested repository evidence was supplied in continuation.

**action taken**: Rewrote the plan around mandatory 109→110→111→112 sequencing; post-111 landmarks; portable vendored tokens; pinned shadcn/Tailwind; no preflight; native semantic preservation; focused test migrations; ActionStatus/reduced-motion coverage; CSSOM disposition; and actual WKWebView evidence. Updated the reference to the real 480×600 and 400×480 targets and made it a styling grammar rather than the sole arbiter.

### round 2

**executor's document**:

```markdown
# Plan 112: Migrate settings chrome to Tailwind v4 + shadcn

> Executor instructions: this plan is reviewed now but executed only after
> Plans 109, 110, and 111 are merged, in that order. Follow every step and
> verification gate. On a STOP condition, stop and report; do not improvise.
> The reviewer maintains plans/README.md — do not edit it.
>
> Post-dependency preflight: start from clean master after Plan 111. Record
> PLAN112_BASE=$(git rev-parse HEAD). Inspect all 109–111 changes under
> src/settings/, src/overlay-card.css, package.json, package-lock.json,
> vite.config.ts, tsconfig*.json, and settings.html. Replace this plan's
> provisional landmark table with observed post-111 paths and line numbers
> before editing. STOP if 109–111 are not merged or their landed contracts
> cannot be classified below.

## Status

- Priority: P2
- Effort: L
- Risk: HIGH — full settings restyle plus component-generator/CSS-pipeline change
- Depends on: 109 → 110 → 111, all merged; 107/108 are already merged
- Category: UI foundation / tech-debt migration
- Reviewed against: notchtap d9a62e1 and shared-ui 8e395a8; re-baseline after 111
- Planned at: 2026-07-22

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

## Provisional current state — replace after Plan 111

At d9a62e1, after merged Plans 107/108 but before 109–111:

- SettingsApp.tsx is 2,203 lines.
- settings.css is 1,003 lines and owns the global reduced-motion rule.
- SettingsApp.test.tsx has 40 tests; full frontend baseline is 256.
- Plan 108 added one shared ActionStatus mechanism used by footer actions,
  test notifications, Connectors, Diagnostics, History, defaults, connector
  health, and appearance hot-apply.
- Tests have class coupling beyond rotation-order-name, including history-title,
  and current Switch tests inspect HTMLInputElement.checked.
- preview-overlay.css is 1,393 lines, but Plan 111 will delete it, create
  src/overlay-card.css, add card-root preview scopes, and move fixtures.
- Plan 109 will establish a 9px type floor, computed AA contrast, a 400×480
  native minimum, fieldset/legend groups, ul/li lists, and a real table.
- Plan 110 will add rich History metadata/details, non-navigable escaped URL
  text, robust wrapping, and timestamp rules.

After 111, rewrite this section in the execution report with exact counts,
imports, tests, semantic landmarks, History structure, preview fixture path,
card-root host, and shared stylesheet import order. Do not execute from these
pre-111 line numbers.

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

1. Create vendor/shared-ui containing only the upstream package.json and
   design/tokens.css from the reviewed shared-ui SHA.
2. Depend on @chetanjain/shared-ui as file:vendor/shared-ui.
3. Record the upstream SHA and SHA-256 of both token files; they must match
   when the sibling checkout is available.
4. Add a small check script that fails on byte drift when ../shared-ui exists
   and otherwise reports the pinned SHA. CI must remain self-contained.
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
   npm install file:vendor/shared-ui
2. Install pinned build-only dependencies:
   npm install --save-dev tailwindcss@4.3.3 @tailwindcss/vite@4.3.3 @types/node
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
   scoped base/reset rules. Port the old reduced-motion rule here.
6. Import base.css before temporary settings.css. Add class="dark" to
   settings.html.

Verify:

- test/type/lint/build clean;
- built settings CSS contains representative shared OKLCH values and custom
  utilities;
- no preflight/reset block and no competing semantic token values;
- npm ci succeeds with ../shared-ui temporarily unavailable;
- deterministic computed styles and bounds for the complete preview subtree
  are unchanged;
- actual WKWebView preview pseudo-elements, hit testing, accessibility
  exposure, and geometry remain intact.

Any preview delta or external-path dependency is a STOP.

## Step 2 — pin shadcn v4 and generate only needed primitives

1. Run npx shadcn@4.13.1 init --base radix against the existing Vite app.
   Inspect every write before accepting it.
2. components.json must match the shared playground contract:
   style radix-nova, Radix base, CSS variables, blank Tailwind config,
   src/settings/base.css, Lucide, and @ aliases. This file is authoritative.
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

```

**companion reference**:

```html
<!doctype html>
<html lang="en" class="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>112 — settings window UI reference (shadcn + shared-ui tokens)</title>
<style>
  /* ------------------------------------------------------------------
     Visual reference for plan 112. Open in a browser, keep side-by-side
     while porting. Palette values are the hex mirrors of
     ../shared-ui/design/tokens.css (oklch source of truth) — do NOT copy
     these hexes into app code; use the Tailwind utilities named in the
     annotations (bg-background, text-muted-foreground, ...).
     ------------------------------------------------------------------ */
  :root {
    --background: #050607;      /* bg-background */
    --foreground: #f1f2f4;      /* text-foreground */
    --card: #0d0f12;            /* bg-card */
    --muted-fg: #a2a6ad;        /* text-muted-foreground */
    --primary: #0a84ff;         /* bg-primary / text-primary / ring */
    --border: #202329;          /* border-border */
    --accent: #202329;          /* hover:bg-accent */
    --sidebar: #090a0c;         /* bg-sidebar */
    --destructive: #ff6b57;     /* text-destructive (NEW error identity) */
    --overlay-teal: #55d6bd;    /* text-overlay-teal (ok/positive) */
    --radius: 0.625rem;         /* shadcn --radius */
    --ease: cubic-bezier(.22, 1, .36, 1);  /* ease-notchtap */
  }
  * { box-sizing: border-box; margin: 0; }
  body {
    background: #101114;
    color: var(--foreground);
    font: 11px/1.5 -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
    padding: 32px 24px 80px;
  }
  h1 { font-size: 18px; margin-bottom: 4px; }
  .subtitle { color: var(--muted-fg); font-size: 13px; margin-bottom: 24px; }
  .note {
    max-width: 980px; margin: 0 auto 20px; padding: 10px 14px;
    border: 1px solid var(--border); border-left: 3px solid var(--primary);
    border-radius: 6px; background: var(--card); font-size: 13px; color: var(--muted-fg);
  }
  .note b { color: var(--foreground); }

  /* ---------- window mock ---------- */
  .window {
    width: min(480px, 100%); height: 600px; margin: 0 auto; display: flex;
    background: var(--background); border: 1px solid var(--border);
    border-radius: 12px; overflow: hidden;
    box-shadow: 0 24px 64px rgba(0,0,0,.5);
  }
  /* sidebar — keep existing <aside>/<nav> DOM, restyle with utilities */
  .sidebar {
    width: 140px; flex: none; background: var(--sidebar);
    border-right: 1px solid var(--border);
    display: flex; flex-direction: column; padding: 16px 10px;
  }
  .brand { padding: 4px 10px 14px; font-weight: 600; letter-spacing: .02em; }
  .brand small { display: block; font-weight: 400; color: var(--muted-fg); font-size: 11px; }
  .nav { display: flex; flex-direction: column; gap: 2px; }
  /* nav item = Button variant="ghost", active gets bg-accent + text-foreground */
  .nav button {
    all: unset; display: flex; align-items: center; gap: 10px;
    padding: 7px 10px; border-radius: 8px; color: var(--muted-fg);
    font-size: 11px; cursor: pointer; transition: background 220ms var(--ease), color 220ms var(--ease);
  }
  .nav button:hover { background: var(--accent); color: var(--foreground); }
  .nav button[data-active] { background: var(--accent); color: var(--foreground); }
  .nav button[data-active] .dot { background: var(--primary); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: #3a3f47; flex: none; }
  .sidebar .meta { margin-top: auto; padding: 10px; font-size: 11px; color: var(--muted-fg); }

  .content { flex: 1; display: flex; flex-direction: column; }
  .scroll { flex: 1; min-width: 0; padding: 20px; overflow: auto; }
  .section-title { font-size: 19px; font-weight: 600; margin-bottom: 2px; }
  .section-sub { color: var(--muted-fg); font-size: 10px; margin-bottom: 16px; }

  /* Card (shadcn Card, bg-card border-border rounded-[--radius]) */
  .card {
    background: var(--card); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 4px 16px; margin-bottom: 16px;
  }
  .row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 16px; padding: 13px 0; border-bottom: 1px solid var(--border);
  }
  .row:last-child { border-bottom: 0; }
  .row .label { font-size: 11px; }
  .row .caption { display: block; color: var(--muted-fg); font-size: 9px; margin-top: 2px; }

  /* Switch (shadcn Switch: track bg-input, checked bg-primary) */
  .switch { position: relative; width: 36px; height: 21px; flex: none;
    border-radius: 999px; background: var(--border); transition: background 220ms var(--ease); }
  .switch.on { background: var(--primary); }
  .switch::after { content: ""; position: absolute; top: 2.5px; left: 2.5px;
    width: 16px; height: 16px; border-radius: 50%; background: #fff;
    transition: transform 220ms var(--ease); }
  .switch.on::after { transform: translateX(15px); }

  /* Input type=number (shadcn Input, w-20) + unit */
  .num { display: flex; align-items: center; gap: 8px; }
  .num input {
    width: 72px; background: transparent; border: 1px solid var(--border);
    border-radius: 8px; color: var(--foreground); padding: 6px 10px; font: inherit; font-size: 11px;
  }
  .num input:focus { outline: 2px solid color-mix(in srgb, var(--primary) 40%, transparent); outline-offset: 1px; border-color: var(--primary); }
  .num .unit { color: var(--muted-fg); font-size: 12px; }

  /* Segmented option group — post-109 fieldset/button semantics stay;
     Tailwind utilities provide this visual treatment without Radix roving focus. */
  .toggle-group { display: inline-flex; border: 1px solid var(--border); border-radius: 8px; padding: 2px; gap: 2px; }
  .toggle-group button {
    all: unset; padding: 4px 8px; border-radius: 6px; font-size: 10px;
    color: var(--muted-fg); cursor: pointer;
  }
  .toggle-group button[data-on] { background: var(--accent); color: var(--foreground); }
  .toggle-group button:hover:not([data-on]) { color: var(--foreground); }

  /* Badge (status-chip → Badge; keep aria-live) */
  .badge {
    display: inline-flex; align-items: center; gap: 6px; padding: 2px 10px;
    border-radius: 999px; border: 1px solid var(--border);
    font-size: 11.5px; color: var(--muted-fg);
  }
  .badge.ok { color: var(--overlay-teal); border-color: color-mix(in srgb, var(--overlay-teal) 35%, transparent); }
  .badge .dot { width: 5px; height: 5px; }
  .badge.ok .dot { background: var(--overlay-teal); }

  /* Buttons (shadcn Button variants) */
  .btn { all: unset; font-size: 13px; padding: 7px 14px; border-radius: 8px;
    cursor: pointer; transition: filter 220ms var(--ease), background 220ms var(--ease); }
  .btn.primary { background: var(--primary); color: #fff; font-weight: 500; }
  .btn.primary:hover { filter: brightness(1.1); }
  .btn.secondary { background: transparent; border: 1px solid var(--border); color: var(--foreground); }
  .btn.secondary:hover { background: var(--accent); }
  .btn.ghost { color: var(--muted-fg); }
  .btn.ghost:hover { color: var(--foreground); background: var(--accent); }

  /* secret row (Connectors & Keys) */
  .secret { display: flex; gap: 8px; align-items: center; flex: 1; max-width: 420px; }
  .secret input { flex: 1; background: transparent; border: 1px solid var(--border);
    border-radius: 8px; color: var(--foreground); padding: 6px 10px; font: 12.5px ui-monospace, "SF Mono", monospace; }

  /* ErrorPanel → Card border-destructive/40 text-destructive
     NOTE: deliberate identity change — error moves from old rose #d6a1a5 to
     shared coral #ff6b57. Keep role="alert". */
  .error-panel {
    border: 1px solid color-mix(in srgb, var(--destructive) 40%, transparent);
    background: color-mix(in srgb, var(--destructive) 7%, var(--card));
    color: var(--destructive); border-radius: var(--radius);
    padding: 10px 14px; font-size: 13px; margin-bottom: 16px;
  }

  /* footer */
  .footer {
    display: flex; align-items: center; gap: 8px; padding: 12px 20px;
    border-top: 1px solid var(--border); background: var(--sidebar);
  }
  .footer .spacer { flex: 1; }

  /* ---------- annotations ---------- */
  .anno { max-width: 980px; margin: 28px auto 0; }
  .anno h2 { font-size: 15px; margin: 24px 0 10px; }
  table.map { width: 100%; border-collapse: collapse; font-size: 13px; background: var(--card); border-radius: var(--radius); overflow: hidden; }
  .map th, .map td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); vertical-align: top; }
  .map th { color: var(--muted-fg); font-weight: 500; font-size: 12px; }
  .map tr:last-child td { border-bottom: 0; }
  .map code { font: 12px ui-monospace, "SF Mono", monospace; color: #8ec2ff; }
  .keep { color: var(--overlay-teal); font-size: 11.5px; border: 1px solid color-mix(in srgb, var(--overlay-teal) 35%, transparent); border-radius: 999px; padding: 1px 8px; white-space: nowrap; }
  .swap { color: #8ec2ff; font-size: 11.5px; border: 1px solid color-mix(in srgb, #8ec2ff 35%, transparent); border-radius: 999px; padding: 1px 8px; white-space: nowrap; }
  .sw { display: inline-block; width: 12px; height: 12px; border-radius: 3px; vertical-align: -1px; margin-right: 6px; border: 1px solid rgba(255,255,255,.12); }

  /* Minimum supported Tauri inner size from Plan 109. The mock is a
     styling grammar; the executor's real all-section WKWebView contact sheet
     is the acceptance authority at both 480×600 and 400×480. */
  @media (max-width: 430px) {
    body { padding-inline: 8px; }
    .window { width: min(400px, 100%); height: 480px; }
    .sidebar { width: 124px; padding-inline: 6px; }
    .nav button { gap: 7px; padding-inline: 7px; }
    .scroll { padding: 14px; }
    .row { align-items: flex-start; flex-wrap: wrap; gap: 8px; }
    .footer { padding-inline: 14px; flex-wrap: wrap; }
  }
</style>
</head>
<body>

<h1>Plan 112 — settings window target look</h1>
<p class="subtitle">Pinned shadcn radix-nova primitives themed by <code>shared-ui/design/tokens.css</code>. This 480×600 mock defines the visual grammar; the real ten-section WKWebView contact sheet is the acceptance authority.</p>

<div class="note"><b>How to read this:</b> the window below demonstrates frame, spacing, hierarchy, controls, statuses, and connector rows at the actual 480×600 target. It is not an exhaustive mock of all ten sections. Plan 109's native semantics and type/contrast floor remain authoritative. The <b>Appearance preview card is absent on purpose</b>: after Plan 111 it renders through <code>overlay-card.css</code> and must remain visually unchanged.</div>

<div class="window" aria-label="settings window mock">
  <!-- target: shadcn Sidebar collapsible="icon" (expanded state mocked here;
       the collapsed icon-rail state is a free extra, not separately mocked) -->
  <aside class="sidebar">
    <div class="brand">notchtap <small>settings</small></div>
    <nav class="nav" aria-label="sections">
      <button data-active><span class="dot"></span>General</button>
      <button><span class="dot"></span>Football</button>
      <button><span class="dot"></span>News</button>
      <button><span class="dot"></span>Cmux</button>
      <button><span class="dot"></span>Weather</button>
      <button><span class="dot"></span>Connectors &amp; Keys</button>
      <button><span class="dot"></span>Shortcuts</button>
      <button><span class="dot"></span>Appearance</button>
      <button><span class="dot"></span>Diagnostics</button>
      <button><span class="dot"></span>History</button>
    </nav>
    <div class="meta">v0.1.0 · config synced</div>
  </aside>

  <div class="content">
    <div class="scroll">
      <div class="section-title">General</div>
      <div class="section-sub">Rotation, timing, and source defaults.</div>

      <div class="error-panel" role="alert">Couldn’t reach the feed — retrying in 30s. <em>(error identity: coral <code style="color:inherit">text-destructive</code>, was rose #d6a1a5)</em></div>

      <div class="card">
        <div class="row">
          <div><span class="label">Enable rotation</span><span class="caption">Cycle sources on the overlay rail.</span></div>
          <div class="switch on" role="presentation"></div>
        </div>
        <div class="row">
          <div><span class="label">Rotation interval</span><span class="caption">Seconds each source stays visible.</span></div>
          <div class="num"><input type="number" value="12"><span class="unit">sec</span></div>
        </div>
        <div class="row">
          <div><span class="label">Priority</span><span class="caption">Native fieldset/button behavior, restyled with utilities.</span></div>
          <div class="toggle-group" role="group" aria-label="priority">
            <button>Low</button><button data-on>Medium</button><button>High</button>
          </div>
        </div>
        <div class="row">
          <div><span class="label">Quiet hours</span><span class="caption">Pause non-critical cards overnight.</span></div>
          <div class="switch" role="presentation"></div>
        </div>
      </div>

      <div class="section-title" style="margin-top:28px">Connectors &amp; Keys</div>
      <div class="section-sub">Secret rows: Badge (keep aria-live) + Input + Button.</div>
      <div class="card">
        <div class="row">
          <span class="badge ok" aria-live="polite"><span class="dot"></span>connected</span>
          <div class="secret"><input value="••••••••••••••••" readonly><button class="btn secondary">Save</button></div>
        </div>
        <div class="row">
          <span class="badge" aria-live="polite"><span class="dot"></span>not set</span>
          <div class="secret"><input placeholder="API key"><button class="btn secondary">Save</button></div>
        </div>
      </div>
    </div>

    <footer class="footer">
      <button class="btn ghost">Reset to defaults</button>
      <div class="spacer"></div>
      <button class="btn secondary">Reset</button>
      <button class="btn primary">Save &amp; Relaunch</button>
    </footer>
  </div>
</div>

<div class="anno">
  <h2>Control mapping (current → target)</h2>
  <table class="map">
    <tr><th>Current (SettingsApp.tsx)</th><th>Target</th><th></th></tr>
    <tr><td>Bespoke <code>Switch</code> (:501) via <code>ToggleControl</code> (:575)</td><td>shadcn <code>Switch</code> + <code>Label</code> — identical accessible name</td><td><span class="swap">swap</span></td></tr>
    <tr><td><code>NumberControl</code> (:537)</td><td><code>Input type="number"</code> + unit span</td><td><span class="swap">swap</span></td></tr>
    <tr><td>Post-109 <code>fieldset</code>/<code>legend</code> priority and appearance groups</td><td>Keep native semantics and button behavior; restyle with utilities (no Radix roving-focus change)</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td>Rotation-order list, up/down buttons, <code>.rotation-order-name</code> (:711)</td><td>Bespoke markup stays; buttons → <code>Button</code>; <b>class name preserved</b> (test :508)</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td><code>.status-chip</code>, secret input, save button</td><td><code>Badge</code> (keep <code>aria-live</code>) + <code>Input</code> + <code>Button</code></td><td><span class="swap">swap</span></td></tr>
    <tr><td>Post-109 native shortcut <code>&lt;table&gt;</code> + <code>&lt;kbd&gt;</code></td><td>Keep valid table structure; utilities + <code>Separator</code></td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td>Post-110 History list + metadata/details</td><td>Keep native list/details and use native overflow utilities</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td><code>ErrorPanel</code></td><td><code>Card</code> with <code>border-destructive/40 text-destructive</code> (coral — accepted change)</td><td><span class="swap">swap</span></td></tr>
    <tr><td>Post-111 Appearance preview (<code>.card-root</code> + shared fixtures)</td><td><b>Untouched</b> — <code>overlay-card.css</code> and custom-property plumbing stay byte-for-byte out of scope</td><td><span class="keep">out of scope</span></td></tr>
    <tr><td>Sidebar: <code>aside.settings-sidebar</code> + <code>nav.sidebar-nav</code></td><td>shadcn <code>Sidebar collapsible="icon"</code> — <code>SidebarProvider/Header/Menu/MenuButton/Footer/Inset</code> + <code>SidebarTrigger</code>; same accessible names; icon-rail collapse comes free. Fallback if tests break: restyle existing markup with <code>Button variant="ghost"</code></td><td><span class="swap">swap</span></td></tr>
    <tr><td>Footer buttons</td><td><code>Button</code> variants (default / secondary / ghost)</td><td><span class="swap">swap</span></td></tr>
    <tr><td>Toasts</td><td><b>None</b> — sonner deliberately excluded; plan 108 owns feedback UX</td><td><span class="keep">defer</span></td></tr>
  </table>

  <h2>Palette (utilities, not hexes — hex shown for eyeballing)</h2>
  <table class="map">
    <tr><th>Utility</th><th>Value</th><th>Replaces</th></tr>
    <tr><td><code>bg-background</code></td><td><span class="sw" style="background:#050607"></span>#050607</td><td><code>--window</code> (identical)</td></tr>
    <tr><td><code>bg-sidebar</code></td><td><span class="sw" style="background:#090a0c"></span>#090a0c</td><td><code>--sidebar</code> (identical)</td></tr>
    <tr><td><code>bg-card</code></td><td><span class="sw" style="background:#0d0f12"></span>#0d0f12</td><td><code>--surface</code> (identical); raised = + <code>border-border</code></td></tr>
    <tr><td><code>text-foreground</code> / <code>text-muted-foreground</code></td><td><span class="sw" style="background:#f1f2f4"></span>#f1f2f4 / <span class="sw" style="background:#a2a6ad"></span>#a2a6ad</td><td><code>--text</code> / <code>--text-secondary</code>; <code>--text-muted</code> → <code>/70</code></td></tr>
    <tr><td><code>border-border</code> / <code>hover:bg-accent</code></td><td><span class="sw" style="background:#202329"></span>#202329</td><td><code>--divider</code>; <code>--surface-hover</code> (slightly stronger, accepted)</td></tr>
    <tr><td><code>bg-primary</code> / <code>ring-ring</code></td><td><span class="sw" style="background:#0a84ff"></span>#0a84ff</td><td><code>--accent</code>, <code>--accent-soft</code> → <code>bg-primary/15</code></td></tr>
    <tr><td><code>text-destructive</code></td><td><span class="sw" style="background:#ff6b57"></span>#ff6b57</td><td><code>--error</code> #d6a1a5 — <b>deliberate identity change</b></td></tr>
    <tr><td><code>text-overlay-teal</code></td><td><span class="sw" style="background:#55d6bd"></span>#55d6bd</td><td><code>--ok</code> #a3c9a8 — accepted change</td></tr>
    <tr><td><code>ease-notchtap</code>, <code>duration-fast/normal/slow</code></td><td>cubic-bezier(.22,1,.36,1) · 220/300/400ms</td><td><code>--ease</code> (identical curve)</td></tr>
  </table>
</div>

</body>
</html>

```

**reviewer 1 (`openai/gpt-5.6-sol`, for)**: **needs-changes** — One execution blocker remained: Step 1 imported `tw-animate-css` and `shadcn/tailwind.css` before installing their packages, so its required build could fail before Step 2. Install exact-pinned `tw-animate-css` and `shadcn` before authoring/importing base.css.

**reviewer 2 (`anthropic/claude-sonnet-5`, against)**: **needs-changes** — The plan's exhaustive primitive list excluded shadcn Sidebar, while the reference still instructed an invasive Sidebar/collapse swap. Preserve existing aside/nav landmarks or explicitly add and test the Sidebar family. Also clarify migration order versus visible nav order and cross-reference import order with generated-token cleanup.

**disagreement surfaced**: no — both findings were independent and compatible.

**model substitutions**: none. The GPT reviewer again required inline document delivery after the file attachment was not surfaced by the provider; it completed through the same pinned model and continuation.

**action taken**: Installed and pinned every CSS import before Step 1's build; removed the Sidebar primitive recommendation; preserved ErrorPanel's motion lifecycle; clarified nav order; and made token cleanup/verification explicit before and after shadcn init.

### round 3

**executor's document**:

```markdown
# Plan 112: Migrate settings chrome to Tailwind v4 + shadcn

> Executor instructions: this plan is reviewed now but executed only after
> Plans 109, 110, and 111 are merged, in that order. Follow every step and
> verification gate. On a STOP condition, stop and report; do not improvise.
> The reviewer maintains plans/README.md — do not edit it.
>
> Post-dependency preflight: start from clean master after Plan 111. Record
> PLAN112_BASE=$(git rev-parse HEAD). Inspect all 109–111 changes under
> src/settings/, src/overlay-card.css, package.json, package-lock.json,
> vite.config.ts, tsconfig*.json, and settings.html. Replace this plan's
> provisional landmark table with observed post-111 paths and line numbers
> before editing. STOP if 109–111 are not merged or their landed contracts
> cannot be classified below.

## Status

- Priority: P2
- Effort: L
- Risk: HIGH — full settings restyle plus component-generator/CSS-pipeline change
- Depends on: 109 → 110 → 111, all merged; 107/108 are already merged
- Category: UI foundation / tech-debt migration
- Reviewed against: notchtap d9a62e1 and shared-ui 8e395a8; re-baseline after 111
- Planned at: 2026-07-22

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

## Provisional current state — replace after Plan 111

At d9a62e1, after merged Plans 107/108 but before 109–111:

- SettingsApp.tsx is 2,203 lines.
- settings.css is 1,003 lines and owns the global reduced-motion rule.
- SettingsApp.test.tsx has 40 tests; full frontend baseline is 256.
- Plan 108 added one shared ActionStatus mechanism used by footer actions,
  test notifications, Connectors, Diagnostics, History, defaults, connector
  health, and appearance hot-apply.
- Tests have class coupling beyond rotation-order-name, including history-title,
  and current Switch tests inspect HTMLInputElement.checked.
- preview-overlay.css is 1,393 lines, but Plan 111 will delete it, create
  src/overlay-card.css, add card-root preview scopes, and move fixtures.
- Plan 109 will establish a 9px type floor, computed AA contrast, a 400×480
  native minimum, fieldset/legend groups, ul/li lists, and a real table.
- Plan 110 will add rich History metadata/details, non-navigable escaped URL
  text, robust wrapping, and timestamp rules.

After 111, rewrite this section in the execution report with exact counts,
imports, tests, semantic landmarks, History structure, preview fixture path,
card-root host, and shared stylesheet import order. Do not execute from these
pre-111 line numbers.

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

1. Create vendor/shared-ui containing only the upstream package.json and
   design/tokens.css from the reviewed shared-ui SHA.
2. Depend on @chetanjain/shared-ui as file:vendor/shared-ui.
3. Record the upstream SHA and SHA-256 of both token files; they must match
   when the sibling checkout is available.
4. Add a small check script that fails on byte drift when ../shared-ui exists
   and otherwise reports the pinned SHA. CI must remain self-contained.
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
   Inspect every write before accepting it.
2. components.json must match the shared playground contract:
   style radix-nova, Radix base, CSS variables, blank Tailwind config,
   src/settings/base.css, Lucide, and @ aliases. This file is authoritative.
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

```

**companion reference**:

```html
<!doctype html>
<html lang="en" class="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>112 — settings window UI reference (shadcn + shared-ui tokens)</title>
<style>
  /* ------------------------------------------------------------------
     Visual reference for plan 112. Open in a browser, keep side-by-side
     while porting. Palette values are the hex mirrors of
     ../shared-ui/design/tokens.css (oklch source of truth) — do NOT copy
     these hexes into app code; use the Tailwind utilities named in the
     annotations (bg-background, text-muted-foreground, ...).
     ------------------------------------------------------------------ */
  :root {
    --background: #050607;      /* bg-background */
    --foreground: #f1f2f4;      /* text-foreground */
    --card: #0d0f12;            /* bg-card */
    --muted-fg: #a2a6ad;        /* text-muted-foreground */
    --primary: #0a84ff;         /* bg-primary / text-primary / ring */
    --border: #202329;          /* border-border */
    --accent: #202329;          /* hover:bg-accent */
    --sidebar: #090a0c;         /* bg-sidebar */
    --destructive: #ff6b57;     /* text-destructive (NEW error identity) */
    --overlay-teal: #55d6bd;    /* text-overlay-teal (ok/positive) */
    --radius: 0.625rem;         /* shadcn --radius */
    --ease: cubic-bezier(.22, 1, .36, 1);  /* ease-notchtap */
  }
  * { box-sizing: border-box; margin: 0; }
  body {
    background: #101114;
    color: var(--foreground);
    font: 11px/1.5 -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
    padding: 32px 24px 80px;
  }
  h1 { font-size: 18px; margin-bottom: 4px; }
  .subtitle { color: var(--muted-fg); font-size: 13px; margin-bottom: 24px; }
  .note {
    max-width: 980px; margin: 0 auto 20px; padding: 10px 14px;
    border: 1px solid var(--border); border-left: 3px solid var(--primary);
    border-radius: 6px; background: var(--card); font-size: 13px; color: var(--muted-fg);
  }
  .note b { color: var(--foreground); }

  /* ---------- window mock ---------- */
  .window {
    width: min(480px, 100%); height: 600px; margin: 0 auto; display: flex;
    background: var(--background); border: 1px solid var(--border);
    border-radius: 12px; overflow: hidden;
    box-shadow: 0 24px 64px rgba(0,0,0,.5);
  }
  /* sidebar — keep existing <aside>/<nav> DOM, restyle with utilities */
  .sidebar {
    width: 140px; flex: none; background: var(--sidebar);
    border-right: 1px solid var(--border);
    display: flex; flex-direction: column; padding: 16px 10px;
  }
  .brand { padding: 4px 10px 14px; font-weight: 600; letter-spacing: .02em; }
  .brand small { display: block; font-weight: 400; color: var(--muted-fg); font-size: 11px; }
  .nav { display: flex; flex-direction: column; gap: 2px; }
  /* nav item = Button variant="ghost", active gets bg-accent + text-foreground */
  .nav button {
    all: unset; display: flex; align-items: center; gap: 10px;
    padding: 7px 10px; border-radius: 8px; color: var(--muted-fg);
    font-size: 11px; cursor: pointer; transition: background 220ms var(--ease), color 220ms var(--ease);
  }
  .nav button:hover { background: var(--accent); color: var(--foreground); }
  .nav button[data-active] { background: var(--accent); color: var(--foreground); }
  .nav button[data-active] .dot { background: var(--primary); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: #3a3f47; flex: none; }
  .sidebar .meta { margin-top: auto; padding: 10px; font-size: 11px; color: var(--muted-fg); }

  .content { flex: 1; display: flex; flex-direction: column; }
  .scroll { flex: 1; min-width: 0; padding: 20px; overflow: auto; }
  .section-title { font-size: 19px; font-weight: 600; margin-bottom: 2px; }
  .section-sub { color: var(--muted-fg); font-size: 10px; margin-bottom: 16px; }

  /* Card (shadcn Card, bg-card border-border rounded-[--radius]) */
  .card {
    background: var(--card); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 4px 16px; margin-bottom: 16px;
  }
  .row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 16px; padding: 13px 0; border-bottom: 1px solid var(--border);
  }
  .row:last-child { border-bottom: 0; }
  .row .label { font-size: 11px; }
  .row .caption { display: block; color: var(--muted-fg); font-size: 9px; margin-top: 2px; }

  /* Switch (shadcn Switch: track bg-input, checked bg-primary) */
  .switch { position: relative; width: 36px; height: 21px; flex: none;
    border-radius: 999px; background: var(--border); transition: background 220ms var(--ease); }
  .switch.on { background: var(--primary); }
  .switch::after { content: ""; position: absolute; top: 2.5px; left: 2.5px;
    width: 16px; height: 16px; border-radius: 50%; background: #fff;
    transition: transform 220ms var(--ease); }
  .switch.on::after { transform: translateX(15px); }

  /* Input type=number (shadcn Input, w-20) + unit */
  .num { display: flex; align-items: center; gap: 8px; }
  .num input {
    width: 72px; background: transparent; border: 1px solid var(--border);
    border-radius: 8px; color: var(--foreground); padding: 6px 10px; font: inherit; font-size: 11px;
  }
  .num input:focus { outline: 2px solid color-mix(in srgb, var(--primary) 40%, transparent); outline-offset: 1px; border-color: var(--primary); }
  .num .unit { color: var(--muted-fg); font-size: 12px; }

  /* Segmented option group — post-109 fieldset/button semantics stay;
     Tailwind utilities provide this visual treatment without Radix roving focus. */
  .toggle-group { display: inline-flex; border: 1px solid var(--border); border-radius: 8px; padding: 2px; gap: 2px; }
  .toggle-group button {
    all: unset; padding: 4px 8px; border-radius: 6px; font-size: 10px;
    color: var(--muted-fg); cursor: pointer;
  }
  .toggle-group button[data-on] { background: var(--accent); color: var(--foreground); }
  .toggle-group button:hover:not([data-on]) { color: var(--foreground); }

  /* Badge (status-chip → Badge; keep aria-live) */
  .badge {
    display: inline-flex; align-items: center; gap: 6px; padding: 2px 10px;
    border-radius: 999px; border: 1px solid var(--border);
    font-size: 11.5px; color: var(--muted-fg);
  }
  .badge.ok { color: var(--overlay-teal); border-color: color-mix(in srgb, var(--overlay-teal) 35%, transparent); }
  .badge .dot { width: 5px; height: 5px; }
  .badge.ok .dot { background: var(--overlay-teal); }

  /* Buttons (shadcn Button variants) */
  .btn { all: unset; font-size: 13px; padding: 7px 14px; border-radius: 8px;
    cursor: pointer; transition: filter 220ms var(--ease), background 220ms var(--ease); }
  .btn.primary { background: var(--primary); color: #fff; font-weight: 500; }
  .btn.primary:hover { filter: brightness(1.1); }
  .btn.secondary { background: transparent; border: 1px solid var(--border); color: var(--foreground); }
  .btn.secondary:hover { background: var(--accent); }
  .btn.ghost { color: var(--muted-fg); }
  .btn.ghost:hover { color: var(--foreground); background: var(--accent); }

  /* secret row (Connectors & Keys) */
  .secret { display: flex; gap: 8px; align-items: center; flex: 1; max-width: 420px; }
  .secret input { flex: 1; background: transparent; border: 1px solid var(--border);
    border-radius: 8px; color: var(--foreground); padding: 6px 10px; font: 12.5px ui-monospace, "SF Mono", monospace; }

  /* ErrorPanel → Card border-destructive/40 text-destructive
     NOTE: deliberate identity change — error moves from old rose #d6a1a5 to
     shared coral #ff6b57. Keep role="alert". */
  .error-panel {
    border: 1px solid color-mix(in srgb, var(--destructive) 40%, transparent);
    background: color-mix(in srgb, var(--destructive) 7%, var(--card));
    color: var(--destructive); border-radius: var(--radius);
    padding: 10px 14px; font-size: 13px; margin-bottom: 16px;
  }

  /* footer */
  .footer {
    display: flex; align-items: center; gap: 8px; padding: 12px 20px;
    border-top: 1px solid var(--border); background: var(--sidebar);
  }
  .footer .spacer { flex: 1; }

  /* ---------- annotations ---------- */
  .anno { max-width: 980px; margin: 28px auto 0; }
  .anno h2 { font-size: 15px; margin: 24px 0 10px; }
  table.map { width: 100%; border-collapse: collapse; font-size: 13px; background: var(--card); border-radius: var(--radius); overflow: hidden; }
  .map th, .map td { text-align: left; padding: 8px 12px; border-bottom: 1px solid var(--border); vertical-align: top; }
  .map th { color: var(--muted-fg); font-weight: 500; font-size: 12px; }
  .map tr:last-child td { border-bottom: 0; }
  .map code { font: 12px ui-monospace, "SF Mono", monospace; color: #8ec2ff; }
  .keep { color: var(--overlay-teal); font-size: 11.5px; border: 1px solid color-mix(in srgb, var(--overlay-teal) 35%, transparent); border-radius: 999px; padding: 1px 8px; white-space: nowrap; }
  .swap { color: #8ec2ff; font-size: 11.5px; border: 1px solid color-mix(in srgb, #8ec2ff 35%, transparent); border-radius: 999px; padding: 1px 8px; white-space: nowrap; }
  .sw { display: inline-block; width: 12px; height: 12px; border-radius: 3px; vertical-align: -1px; margin-right: 6px; border: 1px solid rgba(255,255,255,.12); }

  /* Minimum supported Tauri inner size from Plan 109. The mock is a
     styling grammar; the executor's real all-section WKWebView contact sheet
     is the acceptance authority at both 480×600 and 400×480. */
  @media (max-width: 430px) {
    body { padding-inline: 8px; }
    .window { width: min(400px, 100%); height: 480px; }
    .sidebar { width: 124px; padding-inline: 6px; }
    .nav button { gap: 7px; padding-inline: 7px; }
    .scroll { padding: 14px; }
    .row { align-items: flex-start; flex-wrap: wrap; gap: 8px; }
    .footer { padding-inline: 14px; flex-wrap: wrap; }
  }
</style>
</head>
<body>

<h1>Plan 112 — settings window target look</h1>
<p class="subtitle">Pinned shadcn radix-nova primitives themed by <code>shared-ui/design/tokens.css</code>. This 480×600 mock defines the visual grammar; the real ten-section WKWebView contact sheet is the acceptance authority.</p>

<div class="note"><b>How to read this:</b> the window below demonstrates frame, spacing, hierarchy, controls, statuses, and connector rows at the actual 480×600 target. It is not an exhaustive mock of all ten sections. Plan 109's native semantics and type/contrast floor remain authoritative. The <b>Appearance preview card is absent on purpose</b>: after Plan 111 it renders through <code>overlay-card.css</code> and must remain visually unchanged.</div>

<div class="window" aria-label="settings window mock">
  <!-- Existing aside/nav landmarks stay. The migration restyles their buttons
       with utilities; shadcn Sidebar and collapse behavior are out of scope. -->
  <aside class="sidebar">
    <div class="brand">notchtap <small>settings</small></div>
    <nav class="nav" aria-label="sections">
      <button data-active><span class="dot"></span>General</button>
      <button><span class="dot"></span>Football</button>
      <button><span class="dot"></span>News</button>
      <button><span class="dot"></span>Cmux</button>
      <button><span class="dot"></span>Weather</button>
      <button><span class="dot"></span>Connectors &amp; Keys</button>
      <button><span class="dot"></span>Shortcuts</button>
      <button><span class="dot"></span>Appearance</button>
      <button><span class="dot"></span>Diagnostics</button>
      <button><span class="dot"></span>History</button>
    </nav>
    <div class="meta">v0.1.0 · config synced</div>
  </aside>

  <div class="content">
    <div class="scroll">
      <div class="section-title">General</div>
      <div class="section-sub">Rotation, timing, and source defaults.</div>

      <div class="error-panel" role="alert">Couldn’t reach the feed — retrying in 30s. <em>(error identity: coral <code style="color:inherit">text-destructive</code>, was rose #d6a1a5)</em></div>

      <div class="card">
        <div class="row">
          <div><span class="label">Enable rotation</span><span class="caption">Cycle sources on the overlay rail.</span></div>
          <div class="switch on" role="presentation"></div>
        </div>
        <div class="row">
          <div><span class="label">Rotation interval</span><span class="caption">Seconds each source stays visible.</span></div>
          <div class="num"><input type="number" value="12"><span class="unit">sec</span></div>
        </div>
        <div class="row">
          <div><span class="label">Priority</span><span class="caption">Native fieldset/button behavior, restyled with utilities.</span></div>
          <div class="toggle-group" role="group" aria-label="priority">
            <button>Low</button><button data-on>Medium</button><button>High</button>
          </div>
        </div>
        <div class="row">
          <div><span class="label">Quiet hours</span><span class="caption">Pause non-critical cards overnight.</span></div>
          <div class="switch" role="presentation"></div>
        </div>
      </div>

      <div class="section-title" style="margin-top:28px">Connectors &amp; Keys</div>
      <div class="section-sub">Secret rows: Badge (keep aria-live) + Input + Button.</div>
      <div class="card">
        <div class="row">
          <span class="badge ok" aria-live="polite"><span class="dot"></span>connected</span>
          <div class="secret"><input value="••••••••••••••••" readonly><button class="btn secondary">Save</button></div>
        </div>
        <div class="row">
          <span class="badge" aria-live="polite"><span class="dot"></span>not set</span>
          <div class="secret"><input placeholder="API key"><button class="btn secondary">Save</button></div>
        </div>
      </div>
    </div>

    <footer class="footer">
      <button class="btn ghost">Reset to defaults</button>
      <div class="spacer"></div>
      <button class="btn secondary">Reset</button>
      <button class="btn primary">Save &amp; Relaunch</button>
    </footer>
  </div>
</div>

<div class="anno">
  <h2>Control mapping (current → target)</h2>
  <table class="map">
    <tr><th>Current (SettingsApp.tsx)</th><th>Target</th><th></th></tr>
    <tr><td>Bespoke <code>Switch</code> (:501) via <code>ToggleControl</code> (:575)</td><td>shadcn <code>Switch</code> + <code>Label</code> — identical accessible name</td><td><span class="swap">swap</span></td></tr>
    <tr><td><code>NumberControl</code> (:537)</td><td><code>Input type="number"</code> + unit span</td><td><span class="swap">swap</span></td></tr>
    <tr><td>Post-109 <code>fieldset</code>/<code>legend</code> priority and appearance groups</td><td>Keep native semantics and button behavior; restyle with utilities (no Radix roving-focus change)</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td>Rotation-order list, up/down buttons, <code>.rotation-order-name</code> (:711)</td><td>Bespoke markup stays; buttons → <code>Button</code>; <b>class name preserved</b> (test :508)</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td><code>.status-chip</code>, secret input, save button</td><td><code>Badge</code> (keep <code>aria-live</code>) + <code>Input</code> + <code>Button</code></td><td><span class="swap">swap</span></td></tr>
    <tr><td>Post-109 native shortcut <code>&lt;table&gt;</code> + <code>&lt;kbd&gt;</code></td><td>Keep valid table structure; utilities + <code>Separator</code></td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td>Post-110 History list + metadata/details</td><td>Keep native list/details and use native overflow utilities</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td><code>ErrorPanel</code> motion lifecycle</td><td>Keep <code>motion.div</code>; apply Card-equivalent <code>border-destructive/40 text-destructive</code> utilities</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td>Post-111 Appearance preview (<code>.card-root</code> + shared fixtures)</td><td><b>Untouched</b> — <code>overlay-card.css</code> and custom-property plumbing stay byte-for-byte out of scope</td><td><span class="keep">out of scope</span></td></tr>
    <tr><td>Sidebar: <code>aside.settings-sidebar</code> + <code>nav.sidebar-nav</code></td><td>Keep landmarks and order; restyle with utilities / <code>Button variant="ghost"</code>. Sidebar collapse is a separate UX decision.</td><td><span class="keep">keep DOM</span></td></tr>
    <tr><td>Footer buttons</td><td><code>Button</code> variants (default / secondary / ghost)</td><td><span class="swap">swap</span></td></tr>
    <tr><td>Toasts</td><td><b>None</b> — sonner deliberately excluded; plan 108 owns feedback UX</td><td><span class="keep">defer</span></td></tr>
  </table>

  <h2>Palette (utilities, not hexes — hex shown for eyeballing)</h2>
  <table class="map">
    <tr><th>Utility</th><th>Value</th><th>Replaces</th></tr>
    <tr><td><code>bg-background</code></td><td><span class="sw" style="background:#050607"></span>#050607</td><td><code>--window</code> (identical)</td></tr>
    <tr><td><code>bg-sidebar</code></td><td><span class="sw" style="background:#090a0c"></span>#090a0c</td><td><code>--sidebar</code> (identical)</td></tr>
    <tr><td><code>bg-card</code></td><td><span class="sw" style="background:#0d0f12"></span>#0d0f12</td><td><code>--surface</code> (identical); raised = + <code>border-border</code></td></tr>
    <tr><td><code>text-foreground</code> / <code>text-muted-foreground</code></td><td><span class="sw" style="background:#f1f2f4"></span>#f1f2f4 / <span class="sw" style="background:#a2a6ad"></span>#a2a6ad</td><td><code>--text</code> / <code>--text-secondary</code>; <code>--text-muted</code> → <code>/70</code></td></tr>
    <tr><td><code>border-border</code> / <code>hover:bg-accent</code></td><td><span class="sw" style="background:#202329"></span>#202329</td><td><code>--divider</code>; <code>--surface-hover</code> (slightly stronger, accepted)</td></tr>
    <tr><td><code>bg-primary</code> / <code>ring-ring</code></td><td><span class="sw" style="background:#0a84ff"></span>#0a84ff</td><td><code>--accent</code>, <code>--accent-soft</code> → <code>bg-primary/15</code></td></tr>
    <tr><td><code>text-destructive</code></td><td><span class="sw" style="background:#ff6b57"></span>#ff6b57</td><td><code>--error</code> #d6a1a5 — <b>deliberate identity change</b></td></tr>
    <tr><td><code>text-overlay-teal</code></td><td><span class="sw" style="background:#55d6bd"></span>#55d6bd</td><td><code>--ok</code> #a3c9a8 — accepted change</td></tr>
    <tr><td><code>ease-notchtap</code>, <code>duration-fast/normal/slow</code></td><td>cubic-bezier(.22,1,.36,1) · 220/300/400ms</td><td><code>--ease</code> (identical curve)</td></tr>
  </table>
</div>

</body>
</html>

```

**reviewer 1 (`openai/gpt-5.6-sol`, for)**: **approve** — Dependency ordering is deterministic; the post-111 preflight and STOP conditions control drift; and CSSOM disposition, no-preflight verification, semantic/behavioral tests, minimum-size WKWebView checks, and zero unintended preview delta adequately bound the migration.

**reviewer 2 (`anthropic/claude-sonnet-5`, against)**: **approve** — Sidebar, ErrorPanel, visible ordering, import/token ownership, and package-order contradictions are resolved. The plan is internally consistent, has concrete machine-checkable gates, and unambiguously preserves Plans 108–111. Two optional nits—running the token drift check explicitly and classifying the token snapshot as build-only—were folded into the final text without reopening review.

**disagreement surfaced**: no — both reviewers approved.

**model substitutions**: none.

**action taken**: Final editorial polish made the vendored token package a dev dependency and added its SHA/drift check to Step 1 verification. No implementation was started.

**reviewed by**: `openai/gpt-5.6-sol` and `anthropic/claude-sonnet-5`

