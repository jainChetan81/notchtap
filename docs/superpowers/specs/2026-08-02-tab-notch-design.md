# Tab-notch redesign — spec

- **Status**: DRAFT — awaiting PAL multi-model review before the implementation plan is written against it.
- **Branch**: `feat/tab-notch-redesign`
- **Design sources of truth**: the feature has SHIPPED, so the React/CSS implementation under `src/` is now authoritative for every value (durations, hexes, class names) — read the code, not this document, when the two disagree. This spec was written against two pre-implementation mocks, `prototypes/tab-notch-rest-and-morph.html` and `prototypes/tab-notch-panel.html` (both r3, operator-approved 2026-08-02 after three review rounds), which were binding *while the feature was being built* and were removed at repo close-out 2026-08-03 once shipped code superseded them. They are retrievable via `git log -- prototypes/` and on branch `feat/tab-notch-redesign`. Sections below that cite "the mock's own comment" are historical provenance, not a live tie-break rule; where this document and a decision already resolved by operator instruction disagree, this document still wins.
- **Do not relitigate**: everything under "Decisions, locked" below. Rounds r1/r2 of both mocks are dead — a big tabbed panel (r1), a permanent icon strip floating outside the shell (r2), a single-line contextual strip under the notch (r2). None of it is resurrected from git history.

## 1. What this is

The notch becomes **pull-based**. Today the overlay is push-only: something happens, a card promotes, it times out. This redesign adds a second, orthogonal way to see things — reach for a source deliberately (hover + click, or a keyboard prefix) and see its current state, with no promotion and no countdown. Push behavior (interrupts, TTL, priority preemption) is completely unchanged and takes precedence over everything pull-related, on every page, without exception.

Two new pieces of vocabulary, used throughout this spec and the plan built from it:

- **Icon strip** / **tab** — one of five neon glyphs (agent, football, music, weather, news) that appears inside the right flank on hover. "Tab" and "icon" are used interchangeably for the same thing; "tab" when talking about selection state, "icon" when talking about the glyph itself.
- **Selection** — at most one tab is selected at a time (or none). Selecting a tab does not open anything by itself; it decides which source's card the *existing* hover below-block shows the next time the notch is hovered.

## 2. Decisions, locked

Restated here because they are the spine everything else hangs off; full rationale lives in the mocks' own comments, cited by section below.

1. **Rest is bare.** Shell + approved idle face + eq bars (only while audio genuinely plays). No icons, no clock, no readouts, ever, at rest. (mock 1, §1)
2. **Icons exist only inside a painted flank.** Hidden at rest (`visibility: hidden` + `opacity: 0` + `pointer-events: none`, not just `opacity: 0` alone), revealed only after `.hovered` has already started painting `#000`. The strip's own opacity fade starts 60ms after the flank paint starts, so there is never a frame where a glyph is visible against the desktop. (mock 1, §2–3)
3. **Every icon animates**, with five distinct per-icon motions/periods so the strip never lands on a shared downbeat (agent 1.9s, football 2.8s, music 900ms bob + 3.6s hue drift, weather 7.2s, news 2.6s when charged). No "one icon stays still as an anchor" — that was r2, rejected. (mock 1, §1)
4. **Visibility rule**: weather and news are present whenever the strip is up. Agent, football, and music are present only while genuinely live (a session running, a match in play, audio playing). Presence is a width+opacity+scale collapse, never `display: none` — the strip must not jump mid-hover if a source goes live/quiet.
5. **Selection: max one, or none, remembered silently across hovers.** Click to select, click the same icon again to deselect, click a different one to move the selection. A selection whose source stops being live is cleared, not remembered. (mock 1, §3)
6. **Hover always shows the selected tab's card in COMPACT form. Never auto-expands.** Expansion is a deliberate, separate keyboard act. This is, per the mocks' own framing, the single most important behavioral line in the whole feature. (mock 1 §2, mock 2's own repeated "hover always shows compact" warning)
7. **The below-block is the card we already ship** — same masthead/kicker/Stamp/accent-stripe/fact-pills/score-block/manifest vocabulary `NotificationBody.tsx`/`AgentBoard.tsx`/`LiveMatchScorecard.tsx` already render. This spec adds **zero new card skeletons**. Per-source additions (session bar, media transport, news batch header — §7 below) are additive within the existing template, the same pattern plans 169/170 already established for football's score-block and Agent Board's hero.
8. **Keyboard model: a tmux-style prefix**, default `⌃⇧Space`, confirmed by the operator, configurable in Settings. `prefix` then one key does one thing, then disarms. The seven shipped `⌃⇧`-combos are unchanged and keep working prefix-free, forever — the prefix is additive, not a migration. (mock 1, §5)
9. **Pull vs push floor-strip semantics**: a tab-summoned (pulled) card never counts down — no TTL, because nothing promoted it, you asked for it. It keeps the shipped 4px floor-strip geometry but repurposes it as a **position indicator** (session count, batch count) with no drain. A pushed interrupt card (agent completion/permission/failure, a goal, a severe-weather alert, a manual CLI push) is **completely unaffected** — same TTL, same priority gates, same preemption, on every page, whether or not a tab is selected. (mock 2, "the floor strip, settled")
10. **No breaking-news interrupts in v1.** News is pure pull, via the charged icon. A real story still promotes through the ordinary queue exactly as it does today (unrelated to the charge model) — this spec does not touch that path.
11. **Scope is Mac mini / HUD mode only.** Every mock rig is the synthetic HUD notch. Real notch-hardware verification is explicitly out of scope for this spec (see §11).
12. **Hard non-goals, standing project rules, not oversights**: no `prefers-reduced-motion` handling anywhere in this feature, no accessibility variants. Reviewers must not flag their absence — this is not new to this feature, it is the same rule the mocks themselves state and the same rule `docs/ARCHITECTURE.md` §4's reduce-motion section already carves out narrow, deliberate exceptions for (the goal/red-card celebrations) rather than a blanket policy this feature would be violating.
13. **Originality**: the idle face and all five glyphs are original notchtap drawings (see mock 1's originality note, CLAUDE.md "naming"). No third-party icon set, mascot, or trade dress. This is a drawing-content constraint on implementation, not a spec-only note — whoever draws the final SVGs must treat the mocks' hand-authored paths as the reference, not as a placeholder to swap for a library icon.

## 3. P0 prerequisite — DONE before this spec was written

`hover.rs`'s `board_rect` used the module-level `WINDOW_HEIGHT` constant (300px) for its coordinate transform even after `try_expand_board_for_hover` had genuinely resized the native window taller — so `hovered=true` could fire for a cursor well below the painted card, in what was actually dead space in the real, taller window. This is fixed: `BoardFrameState` now tracks the *real, currently-applied* window height (set from the exact `frame.height` passed to `window.set_size`, never re-derived), threaded through `board_rect` in place of the constant. 43/43 `hover::` tests pass, including three new tests that reproduce the exact bug. Commit: `951246c` on this branch. Nothing in this spec's own interaction model should be planned around the pre-fix behavior — the hover rect is now trustworthy for the geometry described below.

## 4. Rest state

Exactly the shell (`.card-assembly`, `--flank-w: 0`, width = the 200×32 cutout, nothing else), the ALREADY-SHIPPED `<IdleFace idle={...} />` component (`src/components/IdleFace.tsx`, unmodified — see the correction below), and the eq bars (3 bars, `--media-mint`, `scaleY` off a bottom origin, collapsed to zero width when silent) whenever audio is genuinely playing. Both flanks are zero-width and paint nothing at rest — there is no reserved dead space either side.

**Correction (2026-08-02, found grounding this section against the real component before implementation): the mock's own face illustration is NOT a faithful reproduction of the shipped `IdleFace.tsx` — it is a hand-drawn, illustrative approximation for the static-HTML medium, and this spec's first draft repeated its numbers as if they were the real, unchanged behavior.** They are not; verified directly against `src/components/IdleFace.tsx` and `card-chrome.css`'s `.idle-face*` rules:

| | mock's illustration (WRONG for this spec's purposes) | real, shipped `IdleFace.tsx` (correct) |
|---|---|---|
| continuous "breathe" | `scale(1 → 1.045)`, 5.2s loop, infinite | **does not exist** — no continuous idle animation at all beyond the one-time reveal |
| blink | double: two ~96ms closes 96ms apart, fixed 6.4s cadence | single: one 140ms `scaleY(0.12)` dip, scheduled on a **random** 6000–12000ms interval (plan 125's cost-optimization) |
| eye lag | right eye 40ms behind the left (two independent animations) | **no lag** — both eyes are plain sibling `<span>`s sharing ONE parent `transform` (`.idle-face-eyes`); there is no per-eye timing at all |
| eye shape | "two 4px status-dot circles, shrunk from 9px" | `.idle-face-eye`: `3px × 4px`, `border-radius: 1.5px` (a rounded pill, not a circle) — its own dedicated CSS class, not literally a reused `.status-dot` |
| mouth | "a 1px hairline... a straight rule, never a curve" | an SVG `<path d="M1 1.5 Q7 5.5 13 1.5">` — a gentle quadratic curve, not a straight line |
| gaze | not mentioned in the mock at all | exists in the real component and is NOT mentioned here because it's irrelevant to this correction — left completely alone |

**Resolution: this feature imports and renders the real `<IdleFace />` component unmodified, in the same `.synthetic-cutout` grid cell (`grid-column: 2, grid-row: 1`) it already occupies today** — not a new, re-timed drawing matching the mock's illustrative numbers. This is a strictly SIMPLER implementation than the first draft implied (zero new face code, zero new CSS for eyes/mouth/blink/gaze) and is more consistent with this spec's own "reuse what's shipped" discipline than inventing a second, competing face implementation would have been. The mock's illustration should be read as "a face lives here, roughly like this" for review purposes, not as this feature's literal animation spec.

**Prefix-armed indicator** (delight proposal, mock 1 §5, not yet a decision — see open questions): while the prefix is armed, the face's eyes flick to pure white with a 6px halo. The mock's own accompanying claim that "the breath speeds up to 2.2s" is void per the correction above — there is no breath cycle to speed up. If this delight proposal is picked up, it needs its own small addition to `IdleFace.tsx` (an `armed` prop gating an eye-color/glow override) rather than modulating a cadence that doesn't exist; out of scope unless the operator confirms wanting it.

## 5. Hover reveal

On hover (driven by the rust tracking area's `hover-changed` event, `src-tauri/src/hover.rs` → the frontend — never CSS `:hover`, the overlay window is click-through except where §9 below carves out an exception): both flanks widen together and paint `#000`, the clock fades in on the **left**, and the icon strip arrives **inside** the right flank, 60ms behind the flank's own paint start. All on `REVEAL_MS` (260ms) / `--ease-notchtap`, except the strip's own opacity leg which additionally carries the 60ms stagger.

Flank width formula, hover state only (rest is always exactly `--flank-w: 0`):

```
--strip-w: (icon-box + icon-gap) * icon-count + flank-inset
--flank-w: max(85px * card-scale, --strip-w)
```

At `--icon-box: 18px`, `--icon-gap: 8px`, `--flank-inset: 14px`: 2 icons → 85px rail floor wins (icons alone would need 66px). 5 icons → 144px, hovered shell width 200 + 2×144 = 488px.

If a tab is selected, the below-block additionally mounts that source's compact card underneath, per §7. If nothing is selected, hover shows only the widened shell + clock + strip — §7's "none" page.

Rounding law: unchanged shipped rule (`card-chrome.css`'s ROUNDING LAW — outer rounding on the flanks OR the below-block, never both), with one new case this feature introduces: at rest the flanks are zero-width, so the *cutout itself* is both true outer ends and takes the rounding for that state alone; the instant `.hovered` applies it goes square again and the flanks take over. Implement this as its own explicit CSS rule keyed on `:not(.hovered)`, not as a special-cased JS class — see mock 1's own rule for the exact selector shape.

## 6. The icon strip

Five icon boxes, fixed order **agent — football — music — weather — news**, right-aligned inside the right flank, 18px square, 8px gap (26px flank cost per present icon). Two stacked `drop-shadow`s per icon (3px core + 7px bloom, both `currentColor` so hue and glow can never drift apart) — not a single wider shadow.

| tab | hue | present when | motion |
|---|---|---|---|
| agent | `--overlay-blue` `#0a84ff` | a session is genuinely running | 1.9s breath, blue → teal, glow swells |
| football | `--overlay-green` `#7fe08d` | a match is genuinely live | 2.8s irregular flicker (not sinusoidal), green → near-white, bursts front-loaded |
| music | `--media-mint` `#b6f5e5` | audio is genuinely playing | 900ms 1.5px bob (transform, compositor-only) + 3.6s mint → teal hue drift, two independent clocks |
| weather | `--overlay-amber` `#f0c46a` | always, whenever the strip is up | 7.2s ambient drift, ±1px translateX + ±1.6° rotate, halo breathes amber → pale gold — the slowest/lowest-amplitude motion on purpose, since it's the one icon always present |
| news | `--overlay-coral` `#ff6b57` | always, whenever the strip is up | see §8 (charging/charged is its own two-phase model, distinct from the other four's simple live/idle) |

Three luminance tiers, weight only, hue never changes per-tier (same discipline the shipped status dots already use): `is-present` alone = 0.62 opacity (present but idle — weather, quiet news); `is-present.is-live` = 1.0 (genuinely live — agent/match/audio); `is-present.is-selected` = 1.0. Press feedback on pointer-down (`:active { scale(0.9) }`), the house rule every button in Settings already follows.

Selection mark: a 3px underline dot, 6px below the glyph, in the icon's own hue — deliberately a different *shape* from the glyph itself so a dim-but-selected icon still reads as selected in peripheral vision.

## 7. Selection and the below-block

Click an icon to select; click again to deselect; click a different icon to move the selection (max one, or none). Selection persists across hovers (remembered even though nothing at rest can show it), and is cleared — not remembered — if its source stops being live while selected.

The below-block that mounts on hover-with-a-selection is **the shipped card for that source**, reused, not reinvented:

- **Agent** — the hero (one, the *viewed* session) through the same unified template `AgentHeroCard`/`AgentBoard.tsx` already render (masthead-row, real Stamp, `.title.headline`, subtitle, body, fact pills with `fp-tag` qualifiers), sitting on the existing `agent-origin` runtime wash (`card-chrome.css` ~815: a static corner radial `120% 140% at 0% 0%` of `--cat-deep`, out by 52%, plus a `--cat`-keyed hairline on `.below-block`'s top border) at **shipped card height** — hero only in compact, no roster rows. Below the hero, the **session position bar** (§8) replaces the roster stack for the compact state; the shipped one-line roster (dot, runtime tick, runtime, project, state, elapsed — never a "+N" collapse) is what `prefix+enter` opens. Cycling the viewed session (`prefix-[`/`prefix-]`) rewrites the hero's `src-<runtime>` class, so the wash's hue changes with it (terracotta/Claude Code, green/Codex, purple/OpenCode) — this is the whole point of the wash and the reason the card must stay at shipped height (a taller card was the r2 bug: the wash's radial only ever reaches ~52% of the ellipse it's drawn on, so a stacked hero+roster+pills block made the bottom two-thirds read flat black even though the CSS values were always correct).
- **Football** — the shipped scorecard verbatim (masthead+dot+Stamp, event line as `.title.headline`, `.score-block` at its existing 10px margin-top, `sc-head`/`score-row`/`cards-line`). `prefix+enter` while a match is live and football is selected opens the "crossbar" persistent variant: two stacked `.score-block`s (the second `.score-block.stacked`), no event headline, **no floor strip at all** — this is already how the shipped sticky live card behaves, not a new state.
- **Music / media** — poured into the same skeleton (`media` kicker, track title as `.title.headline`, artist as the subtitle row), then the album tile + transport buttons + the shipped `.media-bar` (the idle peek's own thin now-playing bar, `scaleX` transform, never `width`) in the body area. Transport buttons (prev/play-pause/next) are **new** — the only genuinely new interactive surface this spec adds — because the vendored MediaRemote adapter (plan 104) exposes commands, not just a read-only string; drawn in the existing chip/tile language (same tints, radii, press-scale) so the new capability doesn't read as a new visual language. `prefix+enter` adds a drag-to-seek scrubber and a 3-row queue preview. No TTL anywhere on this page — media is a live surface, not an event.
- **Weather** — the shipped card, unchanged, re-derived strictly off `weather-art.css`/`idle-peek.css` at shipped sizes (28px condition glyph, 18px mono-800 temp, the neutral `.wx-peek-condition` chip, `.wx-peek-hilo`, `.wx-peek-rain`). Do not introduce any new size/spacing variant here — the operator's explicit instruction is that the shipped weather card does not change. `prefix+enter` opens the existing manifest disclosure (the same 320ms grid-rows transition news already uses). A severe-weather alert still interrupts as a high-priority pushed card regardless of selection.
- **News** — the shipped card (source masthead + category dot, real `Wire` stamp, headline, category chip, relative age, over the existing drifting `.news-shade`), plus a **new** batch header above it ("N fresh · cycle ended Xm ago", a dot per story, prev/next) — new because a tab-summoned news card is not a promotion, so it has to say how big the pile is and let the operator walk it. One mono line in the masthead's own type, so it reads as card chrome, not a toolbar. `prefix+enter` opens the existing summary manifest.
- **None selected** — nothing under the hovered shell but the face/clock/strip. With no `.below-block` in the DOM, the rounding law hands the outer corners back to the flanks/cutout — the shipped rule, not a special case built for this page. This is a first-class, intentional state: if nothing is selected, the notch owes nothing more.

On every one of the above, without exception: a pushed high-priority interrupt (agent completion/permission/failure, a goal, a severe-weather alert) still arrives and displays exactly as it does today, regardless of what tab is selected or whether any is. Tab selection decides what the notch shows when the operator goes looking; it never decides what the notch is allowed to tell them.

## 8. The session bar / floor-strip position indicator

Both the agent tab's session bar and the news tab's batch position strip are **pulled-view position indicators**: a shipped-geometry 4px floor strip (absolutely positioned to the card floor, one grid column per item, 3px gaps) with no drain, one segment per item (registered session, or story in the current cycle), the current one bright, the rest dim. Per the mock's own "the floor strip, settled" note, this is the load-bearing distinction from a pushed card's real TTL bar: presence or absence of the drain is the whole tell, everything else about the geometry is identical.

**Agent's version is unambiguous**: `card-chrome.css`'s existing `.ttl-seg` (30% `--accent` trough) and the same paint the draining `.ttl-fill` uses (full `--accent`) for the viewed segment — no new CSS needed, just a component that never mounts a `.ttl-fill` at all. Paints from the **priority** channel (`--accent`), never the **origin** channel (`--cat`) — this is `card-chrome.css`'s standing law (the floor is card chrome like the Stamp and the priority stripe; the hero above it may be painted by origin, the floor under it may not) and must not be violated by this feature. `prefix-[`/`prefix-]` cycles the viewed session and moves the bright segment with it; only meaningful with the agent tab selected, silently ignored otherwise.

**News's version needs a plan-time decision the mock's own markup leaves ambiguous.** Mock 2's news rig renders `.ttl-bar`/`.ttl-seg.done`/`.ttl-fill` with one segment showing a *partial* `scaleX(0.55)` fill, not just a binary bright/dim split like agent's — but there is no wire concept of "how far read into a story" anywhere in this app (no scroll-position tracking, nothing to derive a fractional value from), and the mock's own prose annotation describes pure position semantics ("bright = still to come, dim = already read"), not a progress fraction. Treat the partial-fill visual in the mock as illustrative of what a mid-transition frame might look like, not a literal spec for a new "reading progress" feature. **Default: implement news's floor strip identically to agent's** — one segment per story in the cycle, the current one at full `--accent`, `.ttl-seg.done` (the shipped flat `rgba(255,255,255,0.08)`, not an accent mix) for already-viewed stories, nothing in between, `prefix`-driven prev/next (or the batch header's own prev/next buttons) moving the bright segment. If the plan finds a real reason to want an intra-story fractional fill, that is new scope beyond this spec and needs its own sign-off, not a silent addition.

## 9. The keyboard model

A tmux-style prefix, default `⌃⇧Space` (configurable in Settings — a new field, following the existing Settings-window pattern for a simple string/keybinding value). Pressing it arms a 2-second window (see open question 1 on the timeout value); the next keystroke does exactly one thing and disarms immediately, whether or not it matched anything.

| press | then | does |
|---|---|---|
| prefix | — | arms (2s window) |
| prefix | `1`…`5` | select agent/football/music/weather/news, strip order left to right; same key again deselects |
| prefix | `[` / `]` | previous/next agent session; ignored unless the agent tab is selected |
| prefix | `enter` / `o` | expand ↔ collapse the current card (the *only* expansion gesture that exists anywhere in this feature) |
| prefix | `p` | pause/resume (same pause the tray and `⌃⇧P` already drive) |
| prefix | `esc` or the prefix again | disarm, no side effect |
| prefix | anything else | disarm silently — never beep, never flash an error |

The seven shipped `⌃⇧`-combos (`⌃⇧X` dismiss, `⌃⇧P` pause, `⌃⇧N` expand, `⌃⇧O` open story, `⌃⇧]` skip current, `⌃⇧A` open/focus session, `⌃⇧,` open settings) are **completely unchanged**, still registered prefix-free, forever. `⌃⇧Space` is chosen as the default prefix specifically because it's the one combo in that family nobody's already using.

Mechanism note for the plan: the app today registers each shortcut individually via `tauri_plugin_global_shortcut`, one combo → one action. A prefix needs a *mode*: register the prefix combo globally, then take a temporary key grab for the armed window, then release it back to individual-combo dispatch. This is a genuinely different mechanism from the seven existing combos, not an extension of them, and the plan must scope it as such rather than bolting it onto the existing per-combo registration loop.

## 10. IPC / security surface (binding — read before planning any rust or capability change)

This feature is the first time the overlay window needs to react to a **click**, not just render pushed state — CLAUDE.md's ipc/security section and `docs/ARCHITECTURE.md` §14 lock the overlay as receive-only with no invoke commands; that contract is not being reopened, but a new, narrow exception needs to be scoped explicitly rather than discovered mid-implementation:

- **The overlay stays receive-only for commands.** No new `#[tauri::command]` is added to the `main` window. Icon selection is decided by which rect a click lands in; that decision is made the same way hover already is — the rust side owns the click detection (extending the same tracking-area-adjacent mechanism `hover.rs` already uses for the hover rect, per mock 1's own honest-gap note in §5/open question 6), and it is the rust side, not the frontend, that decides "this click selected the football tab" and pushes that as a new typed event to the frontend, mirroring `hover-changed`'s own shape (a `tab-selection-changed`-style event, not an invoke command). The frontend's job stays exactly what it is everywhere else in this app: render what rust tells it, never decide.
- **`set_ignore_cursor_events` cannot stay unconditionally `true` while the strip is showing.** Today the overlay is click-through at all times (`apply_overlay_native_config`). This feature needs a real click to land on an icon. The scope, per mock 1's own honest framing: the overlay must accept clicks **only inside the hovered strip's own rect, only while that rect is the current hover target**, and revert to click-through the instant hover ends — reusing the fact that hover already has a rust-derived rect (`hover.rs`), not inventing a second geometry system. This is real, new rust work (toggling `set_ignore_cursor_events` conditionally rather than once at startup) and must be scoped and reviewed as its own piece of the plan, not assumed to fall out of the existing hover machinery for free.
- **Media transport commands** (prev/play-pause/next) are the other new click-driven surface. Same posture: a click on a transport button is detected and dispatched to the vendored MediaRemote adapter (plan 104's existing supervised subprocess) from the rust side; the frontend never talks to the adapter directly, and no new invoke command reaches the `main` window's capability file. If the MediaRemote adapter's existing surface doesn't yet expose a *command* path (only the read-only now-playing stream it was built for), the plan must say so explicitly and scope adding one — do not assume it exists unverified.
- **`capabilities/default.json` does not change.** If implementation surfaces a case where it looks like it must, that is a STOP-and-report condition for whichever plan phase hits it, not a judgment call to make silently.
- **The prefix keymap** is a global shortcut registration (`tauri_plugin_global_shortcut`), the same mechanism the seven existing combos already use — no new IPC surface, just more registrations plus the temporary-grab mode described in §9.

## 11. Explicitly out of scope

- Real notch-hardware (MacBook) verification — every mock rig is the synthetic HUD notch; this spec does not claim the geometry is pixel-verified on a physical cutout. Manual verification on the macbook remains the standing per-change checklist item (CLAUDE.md).
- `prefers-reduced-motion` / accessibility variants of anything in this feature — standing project non-goal, not a gap.
- Breaking-news interrupts (news staying pure-pull via the charged icon only) — explicitly future work per the mission brief.
- The idle hover-peek's own existing weather/scorecard reveal mechanism (`IdleHoverPeek.tsx`) is untouched; this feature is a second, tab-driven way to reach a card, not a replacement for the existing ambient hover-peek behavior on cards without a selection.
- Redrawing the five glyphs to be more specific/less arguable (open question 2 below) — ship the mocks' drawings as specified; a redraw is a separate, later design pass if the operator wants one.

## 12. Open questions carried from the mocks (unresolved — the plan must either resolve these explicitly before touching the relevant code, or explicitly defer with a stated default)

1. **Prefix-armed timeout**: 2s then silent disarm (mock's choice) vs. no timeout at all (tmux's own behavior, armed until a key lands). Mock 1 flags this as a guess, not a measurement. **Default if unresolved: 2s**, matching the mock.
2. **Does the prefix work while the notch isn't hovered?** Mock draws this as yes (a keystroke from anywhere summons the surface it needs). This has a real UX cost (the notch can grow while the operator is looking elsewhere) but is "the whole point of a keyboard model" per the mock's own framing. **Default if unresolved: yes.**
3. **Rest-hidden (chosen) vs. inside-always (logged alternative)**: keep icons permanently visible behind a permanent black shoulder on the right flank (costs ~144px of permanent width at 5 sources; buys back a free glance, visible selection at rest, and a charged-news state that can summon attention without being hovered first). This spec ships rest-hidden per the operator's explicit rejection of the permanent-strip look in r2. **Not reopened by this spec** — listed here only because the mock itself flags it as a one-line CSS change (`--flank-w`'s rest value + dropping the strip's `visibility` gate) if ever revisited, not because it's live.
4. **Does a charged news batch need any resting-state mark**, given the charge is otherwise completely invisible until hovered (mock 1 §4's own warning: "may make the whole charge model pointless")? Options: accept it as-is (news isn't urgent, a real story still promotes normally); give charged-news one resting pixel (the first crack in the bare-rest rule); or drop the charge/fill model and let a "cycle ended, items waiting" state promote a normal low-priority card instead. **No default — needs an explicit operator call before implementation**, since each option has different implementation shape (pure-CSS vs. a new resting element vs. deleting scope).
5. **Fill level vs. count badge vs. both**, on the news glyph. Mock ships both (an interior `scaleY` fill plus an 8px numeral at the top-right, the only place on either mock where a drawing pokes outside its own 18px box). **Default if unresolved: ship both**, matching the mock; either can be dropped later as a pure-CSS/JS change with no structural impact.
6. **The five glyph drawings are illustrative, not final** — football's circle-with-three-seams reads close to a generic target glyph; agent's chevron-prompt is legible but generic ("a terminal," not "a coding session"). **Default: ship the mocks' drawings as specified** (§11 — redraws are out of scope for this pass).
7. **Two arguable hue choices**: agent borrows the app's own accent blue (so the strip uses the reserved accent for one source); music's `--media-mint` is very pale, though against an always-black flank (post-rest-hidden) that reads fine. **Default: ship as specified**, both already used elsewhere in the shipped app for the same semantic role.

## 13. Verification this spec expects of its own plan

- `cargo test` (`src-tauri/`) and `cargo clippy -D warnings` clean at every commit batch, per CLAUDE.md.
- `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` clean at every commit batch.
- `capabilities/default.json` byte-unchanged (git-diff-verified, the same discipline `docs/TESTING_STRATEGY.md` §4.4/plan 087 already established for hover's own native-boundary work).
- Any new `#[tauri::command]` (if the plan finds one is genuinely unavoidable, contrary to §10's stated approach) added to `build.rs`'s manifest and `capabilities/settings.json`, never left implicitly available to `main`.
- Every duration/easing token sourced from `animationTiming.ts` (the timing-parity test enforces this already for the existing surface; this feature adds no new bare literal durations where a token already exists — the mocks' own inline durations are all *named* against existing tokens, e.g. `REVEAL_MS`/`HOVER_MS`, not new values).
- A dedicated `review-animations` pass (this repo's own skill) against every new/changed animation this feature introduces, run and resolved **before** the PR opens — see the implementation plan's own closing step.
- Visual/interaction fidelity against both mocks, confirmed via PAL multimodal comparison at each implementation milestone (screenshots vs. the mock's own rendered rigs), not just at the end.
