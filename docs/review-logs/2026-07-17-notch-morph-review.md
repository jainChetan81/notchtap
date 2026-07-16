# review log — notch-morph (§3.5) + docs restructure — 2026-07-17

**setup**: 2 reviewers, `moonshotai/kimi-k2.7-code` (for) + `z-ai/glm-5.2` (against), via pal `consensus`. one transport drop before any model responded; retried per the skill's rule, then both completed. subject: the full uncommitted working tree (notch-morph implementation, docs/review→docs/archive, plan retrofit, glossary "Notifier" amendment, telegram e2e checkmark). key code inlined verbatim in the reviewer prompt.

reviewed by: moonshotai/kimi-k2.7-code (for) + z-ai/glm-5.2 (against) — both completed.

## round 1

**executor's diff**: morphShape.ts (pill for score_update/match_state, grow default), presentationMode.ts (hud-default hook + one-shot presentation-mode event), App.tsx shape class by mode, lib.rs position_window anchoring to CutoutGeometry center via LogicalPosition + mode emit at page load, presentation.rs DetectOutput cutout fields (serde(default), zero-width → None), main.swift auxiliaryTop*Area reporting, styles.css pill/grow/mini shape rules, prototype deleted, docs restructure. gates at review time: cargo 92/92 + 4 doc-tests, vitest 15/15, tsc clean. executor pre-flagged: the mode-event race, coordinate-space ambiguity, pill clipping, unverified .mini rule.

**reviewer 1 (kimi-k2.7-code, for)**: needs-changes (6/10) — confirms the race as critical ("silently lost, macbook renders mini forever"; wants a handshake-class shield) and the NSScreen-points→LogicalPosition translation as a latent multi-monitor misposition bug. verified the enter-side css timing sound (fill-mode both + durations under the 300ms tear-off). flags pill clipping without ellipsis rules and the unverified .mini rule. docs restructure verified coherent.

**reviewer 2 (glm-5.2, against)**: needs-changes (8/10) — same two blockers, with sharper mechanics: the scale_factor conversion touches only the window's own size while the cutout coordinate is never scaled (invariant: shim points == logical px — must be documented/guarded); y=0.0 ignores the inset (works because a notched screen's top-anchor is the intent; wants it deliberate, not accidental); demanded .mini verification (would be a hud regression if absent) and an exit-timing check. explicitly cleared: enter-side timing math (no snap — final keyframes match base rules), serde graceful degradation, docs consistency.

**disagreement surfaced**: no — both needs-changes, same blockers.

**model substitutions**: none (one transport-drop retry, same roster).

**action taken** (all fixed same session, before commit):
1. **race** → double-shield keeping the frontend receive-only: rust now `eval`s `window.__NOTCHTAP_MODE__` AND emits `presentation-mode` on every PageLoadEvent::Finished (mode_once removed); the hook reads the global as its initial state and keeps the listener — whichever side of the mount-timing race occurs, one delivery path lands. 4 new vitest cases (default, global-initial, event-update, garbage-global).
2. **coordinates** → invariant documented at the call site (shim points == logical px, x-axis shared on the primary display); defensive fallback added: computed x outside the current monitor's logical bounds → warn + top-center (yesterday's placement) instead of an invisible window. y=0.0 documented as deliberate (cards sit flush in the notch band).
3. **pill clipping** → nowrap+ellipsis on pill title (max 55%) and body (single line, line-clamp unset).
4. **.mini rule** → false alarm: exists at styles.css:227 with its own drop-in/out animations; reviewers saw a condensed excerpt.
5. **exit timing** → verified: 220/240/260ms all under EXIT_DURATION_MS (300ms). no change needed.

post-fix gates: cargo test 92/92 + 4 doc-tests, clippy `-D warnings` clean, fmt clean, tsc + vite build clean, vitest 19/19. residual macbook-only verification (real cutout alignment, morph look, multi-display behaviour) remains in plan §3.5.1's manual checklist — now with a graceful-degradation floor instead of a silent failure mode. committed on the user's standing instruction for this review ("we will close all of it then").
