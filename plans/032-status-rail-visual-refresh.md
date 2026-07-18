# Plan 032: Status Rail visual refresh — chip removal, rounded default, body prominence, markdown body, celebration A+B

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d926977..HEAD -- src/ src-tauri/src/config.rs`
> On any change to the files in Scope, re-verify the excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW-MED (visual judgement calls — the operator eyeballs the result)
- **Depends on**: none
- **Category**: ui
- **Planned at**: commit `d926977`, 2026-07-18, from the `prototype/status-rail.html`
  rev-3 review session. First of the four-plan status-rail redesign block
  (032–035); the only fully frontend-side one.

## Decisions locked (operator, 2026-07-18)

1. **Tier chip (L1/M2/H3 + label) is deleted.** Priority reads from a 3px
   accent edge (`.compact::before`) + the existing Stamp + Track colour.
   "Color is enough — that chip is worthless."
2. **Rounded corners become the default**: `config.rs` `default_card_radius()`
   8.0 → 16.0 (one line; `get_default_config` inherits automatically since
   it returns `Config::default()`). The Square/Soft/Round presets in
   Settings are unchanged.
3. **Body copy gets more prominent**: `.body` colour `rgba(255,255,255,0.55)`
   → `0.74`, size 11px → 12px, line-height 1.35. The expanded manifest's
   Message cell gets a `.detail-value.message` treatment (0.84 alpha, 12px).
4. **Markdown in bodies**: inline only — `` `code` ``, `**bold**`,
   `*italic*`, line breaks. Escape-first, and the renderer returns React
   nodes — **no `dangerouslySetInnerHTML` anywhere**. No anchors, no block
   elements (overlay is click-through; ⌃⇧O already owns link opening).
5. **Celebration = A + B**: goal keeps the shipped plan-023 burst + ring
   and *adds* three staggered concentric accent rings (the prototype's
   ripple candidate), goal-signal only, one-shot, reduced-motion off.

## Why this matters

The operator reviewed the prototype and picked this exact visual direction.
None of it needs new data on the wire — the body already arrives in full —
so this lands first and derisks the three plumbing plans behind it.

## Current state (verified at `d926977`)

- `.compact` grid is `64px minmax(0,1fr) auto` with the `.tier-code` column
  (`src/styles.css:213-253`; `src/components/TierCode.tsx` renders
  icon + `tierCode(priority)` + `tierLabel(priority)` from
  `src/lib/presentation.ts`).
- `.body` is `rgba(255,255,255,0.55)` / 11px (`src/styles.css:270-280`).
- `.track` is 3-segment priority load (`src/components/Track.tsx`) — **this
  plan does NOT touch the track**; the queue-slider repurpose is plan 033.
- Goal celebration is pure CSS: `.rail-card.pulse-goal` overshoot +
  layered-radial `::after` burst + `::before` ring (`src/styles.css:68-211`),
  driven by `StatusRailCard.tsx`'s `pulse` state keyed on
  `[currentId, currentSignal]` with `PULSE_END_ANIMATION` cleanup.
- `default_card_radius()` is `8.0` (`src-tauri/src/config.rs:221`).
- `src/settings/preview-overlay.css` is the scoped mirror of `styles.css`
  (currently in sync — drift fix landed in the working tree this session).
- Settings preview samples live in `PREVIEW_SAMPLES`
  (`src/settings/SettingsApp.tsx:867-936`).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint/format gate | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |
| Rust (config default + its test) | `cargo test` from `src-tauri/` | all pass |

## Scope

**In scope**:
- `src/styles.css` — chip/grid/accent-edge rules, `.body` + `.detail-value.message`,
  markdown element styles (`code`/`strong`/`em` inside `.body`/`.detail-value`),
  goal ripple layer CSS
- `src/settings/preview-overlay.css` — **every** styles.css change mirrored
  (the drift class fixed this session must not regress)
- `src/components/StatusRailCard.tsx` — drop `<TierCode>`, mount the ripple
  layer on goal pulse
- `src/components/TierCode.tsx` — delete; `src/lib/presentation.ts` —
  delete `tierCode`/`tierLabel`
- `src/lib/markdown.tsx` (new) + `src/lib/markdown.test.tsx` (new)
- `src-tauri/src/config.rs` — `default_card_radius()` → 16.0 (+ its test)
- `src/settings/SettingsApp.tsx` — one preview sample gains a markdown body
- tests that assert on the chip / `.body` colour / preview samples
- `docs/TESTING_STRATEGY.md` §0 counts; `docs/V5_TECHNICAL_SPEC.md`
  appearance default if it names 8px

**Out of scope**:
- The Track repurpose to queue slider (plan 033), auto-expand-all (033),
  idle status rail (034), cmux rich manifest (035)
- Any rust change beyond the one default line
- The `Manifest.tsx` component (035 touches it)

## Git workflow

- Current branch; commits as the steps land (one per step is fine). Do NOT push.

## Steps

### Step 1: Chip removal + accent edge

Delete `TierCode.tsx` and its import in `StatusRailCard.tsx`; remove
`tierCode`/`tierLabel` from `lib/presentation.ts` (+ their tests).
`.compact` grid becomes `minmax(0,1fr) auto` with `padding-left: 20px`,
and gains the accent edge:

```css
.compact::before {
  content: "";
  position: absolute;
  left: 0; top: 12px; bottom: 12px;
  width: 3px;
  border-radius: 0 2px 2px 0;
  background: var(--accent);
  box-shadow: 0 0 12px var(--accent-soft);
}
```

Mirror in `preview-overlay.css`. Update `StatusRailCard.test.tsx`
(chip assertions become edge/absence assertions). **Verify**: vitest, tsc green.

### Step 2: Rounded default

`default_card_radius()` → `16.0`; update the config test pinning the old
default; `V5_TECHNICAL_SPEC.md` if it names 8px as the default.
**Verify**: `cargo test` green.

### Step 3: Body prominence

`.body` → `rgba(255,255,255,0.74)`, 12px/1.35. Add
`.detail-value.message { color: rgba(255,255,255,0.84); font-size: 12px; line-height: 1.5; }`
and apply `message` to the Message cell in `Manifest.tsx`. Mirror CSS.
**Verify**: vitest green; eyeball queued for operator.

### Step 4: Markdown renderer

New `src/lib/markdown.tsx`: `renderInlineMarkdown(text: string): ReactNode`.
Tokenizer, not regex-into-HTML: split on line breaks, then extract
`` `code` `` spans, then `**bold**`, then `*italic*`, emitting
`<code>`/`<strong>`/`<em>` elements with keys. Raw input is never
interpolated as HTML — React escapes text children by construction.
Unclosed markers render literally. Wire it into `.body` (compact) and the
Message cell (manifest). CSS: `.body code, .detail-value code` (mono, 1px
5px padding, `rgba(255,255,255,0.08)` bg, `var(--accent)` colour) — mirror.

Test cases (vitest): plain text untouched; each token; adjacent tokens;
unclosed marker literal; `<script>alert(1)</script>` renders as visible
text, never an element; CRLF/LF line breaks both become `<br/>`.

### Step 5: Celebration A+B (goal ripple)

`StatusRailCard.tsx` mounts `<div className="cele-ripple" aria-hidden="true">`
with three `<span>` children while `pulse === "pulse-goal"`, cleared by the
same `animationend` path as the burst. CSS (mirror both files):

```css
.cele-ripple { position: absolute; inset: 0; z-index: 0; pointer-events: none; }
.cele-ripple span {
  position: absolute; left: 50%; top: 50%;
  width: 22px; height: 22px; border: 2px solid var(--accent);
  border-radius: 50%; opacity: 0; transform: translate(-50%,-50%) scale(0.4);
  animation: ripple-out 720ms ease-out forwards;
}
.cele-ripple span:nth-child(2) { animation-delay: 140ms; }
.cele-ripple span:nth-child(3) { animation-delay: 280ms; }
@keyframes ripple-out {
  0% { opacity: 0.85; transform: translate(-50%,-50%) scale(0.4); }
  100% { opacity: 0; transform: translate(-50%,-50%) scale(9); }
}
@media (prefers-reduced-motion: reduce) { .cele-ripple span { animation: none; } }
```

Test: goal signal mounts three spans; red_card and generic mount none.

### Step 6: Settings preview sample

One `PREVIEW_SAMPLES` entry (the cmux one) gains a markdown body
(e.g. ``run `git push origin master`?``). Settings tests updated.

## Test plan

- vitest: markdown renderer suite (new), StatusRailCard chip-absence +
  ripple-mount assertions, presentation-table deletions, settings samples
- `cargo test`: config default pin
- `docs/TESTING_STRATEGY.md` §0 counts updated (frontend total changes)
- Visual look is manual-verify (§5), queued for operator review

## Done criteria

- [ ] No `TierCode`/`tierCode`/`tierLabel` references remain (`rg`)
- [ ] `.compact::before` accent edge present in both CSS files; the two
      files are rule-identical modulo the `.appearance-preview` scope
- [ ] `default_card_radius()` = 16.0, config test green
- [ ] Markdown renderer has the six test cases above; no
      `dangerouslySetInnerHTML` in `src/`
- [ ] Goal pulse mounts the 3-span ripple; reduced-motion CSS covers it
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`,
      `npx vite build`, `cargo test` all green
- [ ] `plans/README.md` row updated

## STOP conditions

- `preview-overlay.css` and `styles.css` have drifted again at start
  (re-sync first, note it, then proceed)
- The markdown renderer grows past inline tokens (someone adds links or
  block elements) — that reopens decision 4; stop and ask
- Any test asserts on *exact* pixel values for the ripple — visual timing
  is manual-verify by design; assert structure, not frames

## Maintenance notes

- The chip removal is a reversal of the v5.1 review that added the
  non-color cue (icon + code + track). Track (033) and Stamp survive as
  the non-color cues; the review concern is still addressed.
- Future card-content renderers (e.g. news summary) reuse
  `renderInlineMarkdown` — do not fork a second markdown path.
