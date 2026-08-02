# 171 — Tab-notch redesign: pull-based icon strip, selection, keyboard model

- **Status**: DRAFT — awaiting PAL multi-model review (architecture round + slicing/risk round) before any slice starts.
- **Commit**: —
- **Severity**: N/A (feature, not a bug)
- **Category**: UX / architecture — a new pull-based surface alongside the existing push-only overlay.
- **Estimated scope**: large. Rust: a new selection/click/prefix-keymap core, news charge tracking, MediaRemote command dispatch. Frontend: rest-state UI, a new icon strip, five below-block card extensions, settings UI. See slice breakdown below for exact files per slice.
- **Depends on**: the P0 hover-rect fix (`951246c`, already landed on this branch) — every slice below assumes `board_rect`/`hover_point_is_over_card` already thread the real window height correctly.
- **Spec**: `docs/superpowers/specs/2026-08-02-tab-notch-design.md` — read it in full before this plan. This plan sequences and slices the work the spec already fully designed; it does not re-derive design decisions. Every "why" below points back at a spec section rather than repeating it.

## How to read this plan

Ten slices, each independently scoped (files touched, done criteria, explicit non-goals). Slices are grouped into four phases by real dependency, not artificial parallelism — a slice only runs in parallel with siblings in the same phase group if their file sets are genuinely disjoint. Cross-slice interfaces (event shapes, prop contracts) are pinned in §0 below so parallel executors don't have to guess at each other's output.

```
Phase 1 (sequential, one owner — everything else depends on its event/type shapes)
  Slice A — rust selection/click/click-through core

Phase 2 (parallel — disjoint rust files, all depend only on Slice A's emitted event shape)
  Slice B — news charge tracking (rust)
  Slice C — MediaRemote command dispatch (rust)
  Slice D — prefix keymap state machine (rust)

Phase 3 (parallel — disjoint frontend files, all depend only on Slice A/B/C/D's event shapes)
  Slice E — rest-state UI (idle face, eq bars, icon strip rendering)
  Slice F — agent below-block (hero + session bar)
  Slice G — football below-block (crossbar variant)
  Slice H — media below-block (transport controls + scrubber + queue)
  Slice I — news below-block (batch header + position bar)
  Slice J — settings UI for the prefix keybinding

Phase 4 (sequential, one owner — the convergence point)
  Slice K — StatusRailCard.tsx/App.tsx integration, full test suite, animation lock-down, PR
```

## 0. Cross-slice contracts (pin these before dispatching Phase 2/3 — do not let each slice invent its own shape)

These are not fully specified by the design spec (which describes behavior, not wire shapes) and MUST be nailed down — either by whoever executes Slice A first (then documented back into this plan before Phase 2/3 dispatch), or by the plan author if resolved before dispatch. Treat every one of these as a STOP-and-report condition for a slice if the actual shape it finds differs from what's written here.

- **Selection-changed event**: mirrors `hover-changed`'s existing shape/dispatch pattern (a typed tauri event, rust → frontend, never a frontend invoke). Proposed name `tab-selection-changed`, payload `{ selected: "agent" | "football" | "music" | "weather" | "news" | null }`. Emitted only on an actual change (same discipline `emit_hover_changed_if_transitioned` already follows for hover), not on every click.
- **Icon presence/liveness**: the frontend needs to know, per icon, present/absent and live/idle — this is derived data the rust side already has fragments of (agent session count from the Agent Registry, football's `isLiveCard`, now-playing's own gate) but nothing currently unions them into one wire shape for the icon strip. Slice A owns defining this (likely folded into the existing `status-state` event `StatusState` struct, since that's the established ambient/idle-surface channel per plan 034/040/104 precedent — extending an existing wire type, not inventing a new one, matches this repo's stated preference for "a config/data change, not a new render path"). Document the exact field names added here once decided.
- **Prefix-armed state**: a boolean (or a richer enum if disarm-reason ends up mattering) the frontend needs for the eyes-flash delight indicator (spec §4). Likely its own small event or folded into the same status channel — Slice D's call.
- **Session-cycling / expand-toggle actions**: these are prefix-key *outcomes*, not new IPC — `prefix-[`/`prefix-]`/`prefix+enter` map onto rust-side state changes (viewed session index, expanded bool) that already have an equivalent path today (the existing `⌃⇧N` expand toggle, the Agent Board's own session concept) or need one new field threaded the same way. Slice D documents exactly which existing mechanism each keymap action reuses vs. what's new.

## Slice A — selection/click/click-through core (rust)

**Files**: `src-tauri/src/hover.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/config.rs` (a config field or two, if the icon strip's tab-order/visibility ever needs to be configurable — check the spec first, most of it isn't), possibly a new `src-tauri/src/tabs.rs` if the selection state machine is substantial enough to deserve its own module rather than living inline in `lib.rs` (the executor's call, following this codebase's existing pattern of small focused modules — `silence.rs`, `now_playing.rs` — over one growing `lib.rs`).

**What**: per spec §5/§7/§10.
1. Selection state: which tab (if any) is selected. Persists across hover cycles (spec §7's "remembered silently"), cleared if its source stops being live (spec §2 decision 5). DONE: `src-tauri/src/tabs.rs`, `TabSelection`/`Tab`, fully unit-tested (11 tests), no AppKit dependency. Not yet wired into `lib.rs`'s live app state — see item 2's open question below; it needs a real consumer before it's threaded through as `Arc<StdMutex<..>>`, matching `board_frame`'s own pattern.
2. **Click detection — OPERATOR HANDOFF: finish this directly on the Mac Mini, not from the Linux dev environment this plan was otherwise built from.** `panel_event!` (from the third-party `tauri-nspanel` crate) only exposes mouseEntered/mouseMoved/mouseExited — confirmed by reading its call sites in `lib.rs`, no mouseDown/click callback exists anywhere in that mechanism. Two candidate approaches, neither verified from Linux and both needing real hardware to settle:
   - **(a) A new native NSEvent local monitor** (`objc2_app_kit::NSEvent::addLocalMonitorForEventsMatchingMask_handler`, mask `NSEventMask::LeftMouseDown`, needs the `NSEvent` cargo feature on `objc2-app-kit` plus a direct `block2` dependency for `RcBlock` — neither currently in `Cargo.toml`). Rust-side, so it can update the same `TabSelection` a prefix key also mutates (Slice D) without any IPC boundary question. Unverified: whether a LOCAL monitor (app-scoped, not global) reliably receives mouseDown events for a window that is a `NonactivatingPanel` and — by this app's own explicit design — never becomes key/main. This is exactly the kind of thing `NSNonactivatingPanelMask` is *supposed* to support (receiving interaction without stealing focus), but it is a real, unconfirmed assumption, not a proven one.
   - **(b) A plain webview `onClick`**, relying on the fact that `set_ignore_cursor_events(false)` already proves SOME mouse events reach this same non-activating panel today without it becoming key (the shipped board-expand-for-hover code already relies on this for scrolling the expanded Agent Board's row list) — mouseDown might simply generalize the same way, needing no new native code at all. If true, this is architecturally simpler, but reopens the question of how a PREFIX-driven (rust-originated) selection change gets reflected in frontend-local click state without a second, divergent copy of "what's selected" — needs its own answer either way (e.g. the frontend's local click state still only ever mutates through a callback that also fires the same effect a `tab-selection-changed` listener would, so both input paths funnel through one React state setter even though rust never sees the click itself).
   
   **Operator action**: settle this empirically on the Mac Mini — mount a NonactivatingPanel, `set_ignore_cursor_events(false)`, and check with a plain devtools `console.log` in an `onClick` whether a real click reaches the webview at all — then implement whichever answer that gives, directly there. This is a fast, binary, real-hardware check; guessing further from a Linux VPS risks a nontrivial amount of unverifiable, possibly-wrong native code, so it was deliberately left rather than committed to blind. Everything downstream of item 2 (item 3's click-through wiring, item 4's event emission, and by extension the rest of this plan's `StatusRailCard.tsx`/`App.tsx` integration in Slice K) waits on this being resolved on real hardware — the frontend slices (E-J) below do not, and continue in parallel on the Linux side in the meantime.
3. `set_ignore_cursor_events` toggling: click-through everywhere EXCEPT the icon strip's own rect while it's the current hover target. Reverts to unconditional click-through the instant hover ends. This is real, security-relevant surface — re-read `docs/ARCHITECTURE.md` §14 and CLAUDE.md's ipc/security section before touching it, and confirm `capabilities/default.json` stays byte-identical (git-diff-verified) when done. **Deliberately NOT wired into the live `emit_hover_changed_if_transitioned` path yet** — landing the broadened click-through trigger (open on every idle hover, not just board-with-sessions) ahead of any actual click handling would be a real regression: it would start silently eating clicks that today pass through to the desktop during ordinary idle-hover (e.g. the weather peek), with no compensating functionality yet. Land this in the SAME commit as whichever answer to item 2 above gets built, not before.
4. Emits the selection-changed event (§0) on transitions only. Not yet built — depends on item 2.

**Landed so far (this session)**: `src-tauri/src/hover.rs`'s `icon_strip_rects`/`hovered_right_flank_width` (pure rect geometry, 8 tests, verified against both the mock's own worked examples — 2 icons → 370px shell, 5 icons → 488px shell) and `src-tauri/src/tabs.rs`'s `TabSelection`/`Tab` (pure state machine, 11 tests). 966/966 `cargo test` passing (947 baseline + 8 + 11). Neither is wired into the live app yet, both blocked on item 2's open question above.

**Boundaries**:
- Do NOT add a `#[tauri::command]` for click handling. If implementation surfaces a case where one seems unavoidable, STOP and report rather than adding it — see spec §10's explicit STOP condition.
- Do NOT touch `capabilities/default.json`. Confirm via `git diff` before this slice's commit, not after.
- Do NOT implement the prefix keymap here (Slice D) or news charge tracking (Slice B) — this slice is click + selection + click-through only.

**Verification**: `cargo test`/`cargo clippy -D warnings` clean. New unit tests for the selection state machine (select/deselect/move-selection/clear-on-liveness-loss) following the pure-function-first discipline `docs/TESTING_STRATEGY.md` §3 lays out for exactly this kind of deterministic logic — the click-through/AppKit boundary itself stays manual-only, same treatment `apply_overlay_native_config`/the tracking-area wiring already get (§4.4/plan 087 precedent). `capabilities/default.json` diff is empty.

## Slice B — news charge tracking (rust)

**Files**: `src-tauri/src/rss_poller.rs` or `src-tauri/src/config.rs` (wherever the poll-cycle boundary already lives), NOT `hover.rs`/`lib.rs` — genuinely disjoint from Slice A.

**What**: per spec §8/open question 4's default (ship as-is, both fill level and count badge). Track: items landed since the last charge-clear, batch size (configurable), whether the current cycle has ended with items waiting ("charged"). Feed into the same status-channel extension Slice A defined in §0 (icon presence data) — coordinate the exact field shape with Slice A's actual output, don't invent a second one.

**Boundaries**: does not touch click/selection/click-through (Slice A's territory). Does not decide the resting-mark open question (§12 item 4 in the spec) — ship hover-only visibility, exactly as specified, no resting indicator.

**Verification**: `cargo test` — new unit tests for the charge state machine (empty → charging → full-not-charged → charged → cleared-on-visit), mirroring the existing edge-triggered-alert pattern `weather_poller.rs` already uses for its own per-alert "already fired" state (§19 of `docs/ARCHITECTURE.md`).

**Landed so far**: `src-tauri/src/news_charge.rs`, `NewsCharge` — pure state machine (`item_landed`/`cycle_end`/`visit`/`fill`/`is_charged`/`count`), 10 tests, mirrors `WeatherAlertState`'s edge-trigger persistence exactly (charges once at `cycle_end`, stays charged until `visit`, never re-derived live). Resolved the plan's own open ambiguity: "charged" requires a FULL batch at cycle-end (not just any items waiting), documented in the module doc as a one-line flip in `cycle_end` if that reading proves wrong on real hardware. Not wired into `rss_poller.rs`'s poll loop or into `StatusState` — that wire shape is still Slice A's call per §0, and Slice A's remaining scope is gated on the Mac Mini hand-off (item 2, click detection). This module knows about neither and is ready to be driven by whichever lands the wiring. `#![allow(dead_code)]`, same staged-ahead-of-caller situation as `tabs.rs` (also fixed this session — `cargo clippy --locked --all-targets -D warnings`, the actual CI gate, was failing on `tabs.rs`/`hover.rs`'s icon-strip additions before this; both now pass under the exact CI invocation). 976/976 `cargo test` passing (966 baseline + 10).

## Slice C — MediaRemote command dispatch (rust)

**Files**: `src-tauri/src/now_playing.rs`, plus whatever click-routing Slice A's event mechanism exposes (read-only dependency on Slice A's shape, not a shared file).

**What**: per spec §7's media bullet — prev/play-pause/next dispatched to the vendored MediaRemote adapter (plan 104's existing supervised subprocess). **First check whether the vendored adapter's current surface exposes a command path at all** (plan 104 built it for the read-only now-playing stream) — if it doesn't, this slice's real scope is adding one, and that's a bigger, separately-risky piece of work than routing a click; say so explicitly rather than discovering it mid-slice.

**Boundaries**: read-only dependency on Slice A for "a click landed on this transport button" — does not modify Slice A's files. Does not touch the scrubber's seek functionality if that turns out to need a different adapter surface than prev/pause/next — flag it, don't improvise a partial implementation.

**Verification**: `cargo test` clean, new tests for whatever command-dispatch logic is pure/testable (the subprocess call itself stays manual-only, same posture as every other MediaRemote surface in this codebase per `docs/TESTING_STRATEGY.md`'s own note on plan 104's operator-owed live-adapter checks).

**Landed so far**: checked first, per this slice's own instruction — the vendored adapter already exposes a command path (`bin/mediaremote-adapter.pl send <MRCommand ID>`, confirmed via its own `--help` text and `include/MediaRemoteAdapter.h`'s `MRCommand` enum); plan 104 just never wired the write side. `src-tauri/src/now_playing.rs`: `MediaCommand` (Previous/PlayPause/Next) → MRCommand id, a pure tested mapping (2 tests), and `send_command`, a one-shot dispatch mirroring `run_stream_once`'s permission-check + `SYSTEM_PERL` discipline, 3s-bounded like `presentation.rs`'s own subprocess probe. Vendored tree untouched (`VENDORED.md` stays frozen). Not wired to a real caller — same Slice A click-routing gate as Slice B. `#[allow(dead_code)]` on the staged pieces, same discipline as B. 978/978 `cargo test` passing.

## Slice D — prefix keymap state machine (rust)

**Files**: `src-tauri/src/lib.rs` (the existing shortcut-registration block, ~line 700-830) plus a new module if the arm/disarm state machine is substantial (`src-tauri/src/prefix.rs` or similar — executor's call), `src-tauri/src/config.rs` (the configurable prefix keybinding field + Settings surface hookup).

**What**: per spec §9/§0. The existing seven combos (read them at `lib.rs`'s `tauri_plugin_global_shortcut::Builder::new()` call site, ~line 712, and the `.register(Shortcut::new(...))` calls following it) stay completely unchanged — this slice ADDS a prefix registration and a temporary-grab arm/disarm mode, it does not touch the seven existing `.register()` calls at all. Arm on the prefix combo (default `⌃⇧Space`, config-driven — coordinate with Slice J's settings field), 2s timeout (spec §12 open question 1's stated default), one key consumed then disarm, silent no-op on anything unmapped.

**Boundaries**: does not touch the seven existing shortcuts' own registration or handlers. Does not implement the frontend eyes-flash indicator (Slice E) — this slice only emits the armed/disarmed state (§0).

**Verification**: `cargo test` clean — the arm/disarm/timeout/key-consumption state machine is exactly the kind of pure, deterministic logic `docs/TESTING_STRATEGY.md` §3 says to TDD; the actual `tauri_plugin_global_shortcut` grab/release mechanism stays manual-only (same posture as the existing seven combos, which likewise have no automated coverage of the AppKit-level registration itself).

**Landed so far**: `src-tauri/src/prefix.rs`, `PrefixState`/`PrefixKey`/`PrefixAction` — pure arm/disarm/timeout/key-consumption state machine, 15 tests. Documents which existing mechanism each action reuses (`PrefixAction`'s own doc comments) rather than inventing new ones. Does NOT touch `tauri_plugin_global_shortcut` registration in `lib.rs` — the real wiring (dynamic register/unregister of the seven follow-up keys for the live window plus a cancellable 2s timer) is architecturally sound but a live, timing-sensitive integration this environment can't exercise; left for real-device wiring alongside Slice A's click detection. `#![allow(dead_code)]`, same discipline as B/C. 993/993 `cargo test` passing.

## Slice E — rest-state UI (frontend)

**Files**: a new `src/components/IconStrip.tsx` (or similar), a new eq-bars component/CSS, `src/App.tsx`/`src/components/StatusDots.tsx` (removing the status dots per spec §2 decision 1 — confirm this doesn't orphan the `StatusDots` component entirely elsewhere before deleting vs. just unmounting it from this surface).

**Correction (2026-08-02, found grounding this slice against the real component before implementation): `<IdleFace />` (`src/components/IdleFace.tsx`) needs ZERO changes for this slice — verified by reading it in full.** The spec's own first draft incorrectly described new face timing (double-blink/6.4s/eye-lag) that doesn't match the shipped component at all; see the spec's own §4 correction for the full diff. `<IdleFace idle={...} />` already renders in the exact grid cell (`grid-column: 2, grid-row: 1`, the cutout) this redesign doesn't touch, with its own already-correct gating prop. This slice's real face-adjacent work is: confirm the `idle` boolean this component receives still computes correctly once the surrounding shell/flank markup changes shape (it shouldn't need to — the component's own gating logic is untouched by anything in this feature), and do NOT write any new eyes/mouth/blink/gaze code.

**What**: per spec §4/§5/§6. Rest = shell + the UNMODIFIED `<IdleFace />` + eq bars only (eq bars ARE new — no existing component renders them today, confirmed via grep; build them per spec §4's description, 3 bars in `--media-mint`, `scaleY` off a bottom origin). Hover reveal geometry (`--flank-w` formula, §5). The five icons with their exact hues/animations/visibility rules (§6 table). All driven by props/state from Slice A/B's emitted events — this slice does not talk to rust directly beyond listening to the existing event channel.

**Boundaries**: does not touch the below-block content (Slices F-I) — this slice owns the shell/flank/strip only. Every duration sourced from `animationTiming.ts` tokens, not new bare literals (spec §13).

**Verification**: `npx vitest run`/`tsc`/`biome` clean. New component tests per icon (present/absent/live/idle/selected rendering), following this codebase's existing testing-library-behavior-not-implementation convention.

## Slice F — agent below-block: hero + session bar (frontend)

**Files**: `src/components/AgentBoard.tsx`/`src/components/NotificationBody.tsx` (extending `AgentHeroCard`, per spec §7's explicit "reuse the unified template, add zero new skeletons" instruction — same pattern plan 169 already established), a new session-bar sub-component (or inline, executor's call) per spec §8.

**What**: viewed-session hero at shipped card height (the r2-bug fix is already correct in the mock — do not regress it), session cycling (`prefix-[`/`]`), the no-drain position bar. Coordinate the exact prop/event shape for "which session is viewed" and "cycle to next/prev" against Slice D's prefix-action output.

**Verification**: `npx vitest run` — new tests confirming the runtime wash reads correctly at shipped height (the actual regression this spec's own §7 flags from r2), the session bar's segment count/brightness tracks the viewed index, cycling wraps correctly at both ends.

**Landed so far**: `src/components/PositionBar.tsx` (new, shared with Slice I per spec §8's own default) — the no-drain floor strip, reusing `.ttl-bar`/`.ttl-seg`/`.ttl-seg.done`/`.ttl-fill` verbatim (no new CSS), 13 tests. `src/components/AgentBelowBlock.tsx` (new) — composes `AgentHeroCard` + `PositionBar` at shipped card height (no roster rows), 12 tests, plus `cycleSessionIndex` (pure wraparound prev/next). Extracted `agentHeroPropsFor` out of `AgentBoard.tsx`'s own inline derivation so both components share it rather than drifting copies (`AgentBoard.tsx`'s own 75 tests still pass unchanged). Not wired to a real mount point — the outer hover/selection shell is Slice K's job. `npx tsc --noEmit`/biome/`npx vitest run` all clean (one pre-existing, unrelated `SettingsApp.test.tsx` timeout confirmed via `git stash` to predate this slice).

## Slice G — football below-block: crossbar variant (frontend)

**Files**: `src/components/NotificationBody.tsx`'s `FootballHeroCard` (extending, per plan 170's precedent — same file Slice F touches, so this slice and Slice F are NOT trivially parallel-safe against each other if both land in the same file region; coordinate merge order or scope each to a clearly separate function/component within the file).

**What**: the crossbar persistent variant (two stacked `.score-block`s, no headline, no floor strip) per spec §7's football bullet, reached via `prefix+enter` while football is selected and a match is live.

**Verification**: `npx vitest run` — compact vs. crossbar rendering, confirm no floor strip renders in either pulled state (matching the shipped sticky-card precedent this spec explicitly says NOT to regress).

**Landed so far**: `FootballHeroCard` extended with `secondaryMatches` (default `[]`, byte-identical to before for every existing caller — 143 pre-existing tests unchanged), rendering additional `.score-block.stacked` blocks via an extracted `ScoreBlockContent` helper. 6 new tests. **Discovered gap, flagged rather than improvised**: nothing on the wire currently surfaces more than one live match at a time (`StatusState.football.live` is a single `Option`; `poller.rs`'s snapshot map tracks every watched league but collapses to "first in-play wins" before the wire) — a real second match needs a rust wire change outside this slice's file scope. `secondaryMatches` has no real caller yet; Slice K's integration (or a dedicated follow-up plan) needs to resolve this before the crossbar variant can show a genuine second match.

## Slice H — media below-block: transport + scrubber + queue (frontend)

**Files**: a new media below-block component (`src/components/MediaCard.tsx` or similar) — genuinely new surface, not an extension of an existing card, so this is the most independently parallel-safe of the below-block slices.

**What**: per spec §7's media bullet. Transport buttons wired to Slice C's command-dispatch event, the shipped `.media-bar` reused verbatim, scrubber + queue preview behind `prefix+enter`.

**Verification**: `npx vitest run` — button click → correct command-dispatch event fired (mock the event, don't require a real MediaRemote adapter in CI, same posture as every other subprocess-backed surface here).

**Landed so far**: `MediaBelowBlock.tsx` (new) — title/subtitle, album-art glyph, three presentational transport buttons (`onCommand` callback, no `invoke()`), the shipped `.media-bar` progress indicator with its discontinuity guard ported faithfully from `IdleHoverPeek.tsx`, `expanded`-gated scrubber + optional queue preview (no real queue data source, flagged like Slice G's `secondaryMatches`). 28 tests. New `media-below-block.css` for the genuinely-new transport-button markup only; `.media-bar` itself reused verbatim from `idle-peek.css`. Discovered a pre-existing gap (not this slice's to fix): `--media-mint` has no `:root` definition anywhere despite Slice E's `icon-strip.css`/`eq-bars.css` already referencing it — flagged for Slice K.

## Slice I — news below-block: batch header + position bar (frontend)

**Files**: `src/components/NotificationBody.tsx`'s news branch (extending), a new batch-header sub-component.

**What**: the batch header (spec §7's news bullet) + the position bar resolved per spec §8's news default (binary bright/dim, same as agent's, not a fractional fill).

**Verification**: `npx vitest run` — batch count/position rendering, prev/next navigation, "visited clears the charge" wired back to Slice B's charge state.

**Landed so far**: `NewsBatchHeader.tsx` (new, presentational) + `NewsBelowBlock.tsx` (new, composes the header + a re-rendered copy of `NotificationBody.tsx`'s news card content + the existing `<Manifest>` + the shared `PositionBar`), 19 tests. Deliberately does NOT use the mock's own `.batch-dots` markup — spec §8 resolves that ambiguity to the shared `PositionBar`, not a second dot-strip. Deliberately no `.compact-hint` node (wrong-key risk, depends on Slice J's configurable prefix string). News card content rebuilt rather than imported (`NotificationBody.tsx`'s news branch isn't its own exported function, unlike `AgentHeroCard`/`ScoreBlockContent`) — verified line-for-line equivalent, flagged for a future extraction. "Visited clears the charge" documented as the intended flow only — no real wire source exists yet (`StatusState.news` is still `{enabled: bool}`), same flagged-gap posture as Slices C/G.

## Slice J — settings UI for the prefix keybinding (frontend)

**Files**: a new Settings section or an addition to an existing one (`src/settings/sections/`) — genuinely disjoint from every other frontend slice.

**What**: the configurable prefix field (spec §9), following the existing Settings-window field-editing pattern. Coordinate the exact config key name with Slice D.

**Verification**: `npx vitest run` — round-trips into the saved config, matching every other Settings field's own test pattern in this codebase.

**Landed so far**: scope corrected from "frontend only" — a persisted field genuinely needs both sides. `Config.prefix_shortcut: String` (rust, default `"⌃⇧Space"`) + matching TS field + `is_valid_prefix_shortcut`/`isValidPrefixShortcut` (kept in exact sync, both sides) + `PrefixShortcutControl` in `ShortcutsSection.tsx` (built on `SilenceWindowControl`'s idiom). 5 rust tests + 2 frontend tests. `capabilities/*.json`/`build.rs`/`prefix.rs`/`lib.rs`'s shortcut block all confirmed untouched. Data only — not wired to a live `tauri_plugin_global_shortcut` registration yet (Slice K/real-device work). `cargo test` 998/998, clippy byte-identical to the known baseline, `tsc`/vitest/biome all clean.

## Slice K — integration, full verification, animation lock-down, PR (sequential, one owner, after every above slice lands)

1. Wire everything into `src/components/StatusRailCard.tsx`/`src/App.tsx` — the actual mount points for the icon strip and the selection-driven below-block swap. This is the one place genuine cross-slice integration risk lives; budget real time for it, don't treat it as a rubber-stamp merge step.
2. Full gates: `cargo test`, `cargo clippy -D warnings`, `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` — all clean.
3. `capabilities/default.json` byte-unchanged, final check.
4. **Animation lock-down pass** (operator-requested, run here, before the PR opens): invoke this repo's `review-animations` skill against every new/changed animation this feature introduces — the icon-strip reveal/hover/select transitions, all five icon motions, the news charge fill/glow, the prefix-armed eyes indicator, the session-bar segment transitions, the hero-swap on session cycling. Fix every Block-tier finding; document or fix every lower-tier one. Do not proceed to step 5 until this is clean.
5. Push, open the PR to master linking this plan, the spec, and both mocks. Wait for CodeRabbit + PR-Agent (same posture as every PR this session — read the actual posted findings, fix or rebut with evidence, don't merge on an unread review).
6. Merge only when CI is green and both reviews are approved/addressed.

## Verification (whole-feature, checked at Slice K)

- All gates in Slice K's step 2, clean.
- Manual/visual: HUD-mode-only per spec §1 hard rule — this cannot be verified from this Linux VPS; report exactly what needs checking on the mac mini, matching CLAUDE.md's own standing per-change hardware-verification note.
- PAL multimodal comparison (screenshots vs. both mocks) at each Phase boundary, not just at the end — the advisor session (not the executors) owns triggering and reviewing these, per the mission's own instruction that this happens outside the dispatched implementation work.
- Every open question in spec §12 either resolved explicitly in this plan (items 1, 2, 3, 5, 6, 7 — defaults stated) or flagged as needing an explicit operator call before the relevant slice starts (item 4, the news resting-mark question — Slice B ships without one, per the spec's own stated default, and this is NOT re-opened by this plan).
