# Plan index

One line per plan that ever existed. The full plan files were removed
from the working tree; every one is retrievable via
`git log -- plans/` / `git checkout <commit> -- plans/...`.

## done

- 001 — Wire the two "planned" global hotkeys: ⌃⇧] skip, ⌃⇧, open settings.
- 002 — Animation previews in the settings window's Appearance section.
- 003 — Uptime Kuma → notchtap webhook recipe (docs only; lives on as `docs/recipes/kuma-webhook.md`).
- 004 — Docs truth pass: make agent-facing docs and load-bearing comments match shipped reality.
- 004 — Per-source test notifications in the settings window (`send_test_notification`).
- 005 — Appearance config (scale/radius/opacity) with hot-apply, no relaunch.
- 005 — Verify the relocated OpenRouter key and complete its rotation.
- 006 — Stop telegram transport errors from logging the bot token.
- 007 — Supply-chain/CI hardening: pinned git dep, locked CI resolution, vuln scanning, cheaper Node CI.
- 008 — Fix Expanded semantics: auto-expand High at promotion, reset per item, guard idle toggling.
- 009 — Validate live `slot-state` event payloads; pin the event name across the rust↔TS seam.
- 010 — Harden the ESPN fetch path: gzip, size cap, redirect limit, shared client settings.
- 011 — RSS robustness: characterize `fetch_feed`, bound the entity decoder, stream the size cap.
- 012 — Harden ⌃⇧O open-story: reap the child, open the parsed URL, test the scheme gate.
- 013 — Run `validate()` at boot so hand-edited configs get the settings window's contract.
- 014 — Test the log-rotation engine and the eval-splice escaping.
- 015 — Replace the 250 ms heartbeat with deadline-based wakeups.
- 016 — Add the Biome frontend lint/format gate.
- 017 — Add the justfile: one-command local verification.
- 018 — Overlay idle-cost cut: composite the news shader via transform.
- 019 — Remove dead machinery: presentation-mode frontend channel, never-flipped polling gates, no-op dispatch.
- 020 — Serve config defaults from rust (`get_default_config`), ending frontend triplication.
- 021 — Settings save polish: preserve feed metadata, reject duplicates, pre-flight the port.
- 022 — Execute the parked deep-testing work order (queue proptest trigger fired).
- 023 — Fix the invisible goal celebration; redesign the moment (CSS, reduced-motion aware).
- 024 — Extend the queue property suite to the rotation_order rank tie-break.
- 025 — Stream-cap the ESPN scoreboard fetch; share the pollers' HTTP posture.
- 026 — Docs/DX truth pass: invoke-command counts, heartbeat prose, biome wording, `just setup`.
- 027 — Appearance section reflects Reset; App.tsx unlisten guard.
- 028 — One shared Event test builder (rust) and listen-mock harness (frontend).
- 029 — Pin GitHub Actions to commit SHAs.
- 030 — SPIKE: design OpenRouter best-effort news enrichment into EventMeta.
- 031 — SPIKE: design the live-match scoreboard card (supersession engine's first producer).
- 032 — Status Rail visual refresh: chip removal, rounded default, markdown body, celebrations.
- 033 — Queue slider track + auto-expand-all lifecycle.
- 034 — Idle source-status rail (`status-state` channel).
- 035 — Rich relay manifest: `subtitle`/`details` wire fields, CLI flags, hooks.
- 036 — Close the heartbeat's lost-wakeup race (register waiter under the queue lock).
- 037 — The Engine: one propagation module for every Slot mutation (`engine.rs`).
- 038 — `batch_done` must not count a Recurring requeue as "done".
- 039 — ESPN live-match scoreboard card (opt-in, single-match, Topic supersession).
- 040 — Weather source + idle-rail ambient presence (incl. football chip).
- 041 — ESPN event-card copy: name the event, not just scorer+minute.
- 042 — Live-match scorecard presentation (richer collapsed card).
- 043 — (superseded) Richer live-match event coverage; shipped via plan 083's rich events.
- 044 — A same-poll card must not un-retire a just-finished live-match card.
- 045 — Bump the stale `tauri-nspanel` git pin past two crash fixes.
- 046 — Docs truth pass: Engine/weather/live-card narrative, stale citations.
- 047 — Test backfill for newest modules: card team-id mismatch, weather thresholds.
- 048 — Replace `StatusState::snapshot`'s positional bool/Option signature with named construction.
- 049 — SPIKE: shared shape for per-source config (`docs/design/` doc).
- 050 — SPIKE: design a read-only `GET /status` HTTP route.
- 051 — SPIKE: let Manual/Cmux pushes carry a `link`, unlocking ⌃⇧O outside News.
- 052 — SPIKE: give News an ambient idle-rail summary.
- 053 — SPIKE: a second Topic-supersession producer for Manual/Cmux.
- 054 — (superseded) Own app icon; shipped via plan 094.
- 055 — (superseded) SPIKE: in-card pause control; closed by plan 079 decisions.
- 056 — (superseded) SPIKE: richer scorecard visual; shipped via plans 079/084.
- 057 — (superseded) SPIKE: evaluate a paid sports-data API; declined.
- 058 — `notchtap run`: wrap a long command, push a completion card.
- 059 — SPIKE: persist and browse past notifications (led to 088/089).
- 060 — (superseded) SPIKE: HUD-mode card top treatment; closed by plan 079/091 cutout language.
- 061 — Add the Settings window to the (since-removed) DESIGN.html reference.
- 062 — SPIKE: bridge phone notifications into notchtap.
- 063 — Fix idle rail overlapping other apps' menu-bar icons in notch mode.
- 064 — Topic-supersede stops dropping `meta` (live-match Clock/Cards updates).
- 065 — Rotate and de-commit the hardcoded BrightData API token.
- 066 — Validate `cmux_ttl_secs` (a saved 0 rotated cards out instantly).
- 067 — `rotation_order` self-heal dedupes pre-existing duplicates.
- 068 — Test coverage for `build_test_event`/`send_test_notification`.
- 069 — (superseded) Memoize `renderInlineMarkdown`; shipped via plan 078.
- 070 — Add tracing to the `/notify` → `Engine::accept` ingest pipeline.
- 071 — Docs truth pass: project-state narrative, test count, README table.
- 072 — Cross-tier Topic supersede respects the `max_queued_per_tier` cap.
- 073 — Decide the ambient status side-channel generalization (`AmbientSlot<T>`).
- 074 — Test `lookahead_rain_probability`'s minute-rounding/day-rollover boundary.
- 075 — SPIKE: trial-bump TypeScript 7 / Vite 8 / Vitest 4 in a worktree (GO; adoption never executed).
- 076 — Surface Telegram connector delivery health in Settings (connector later removed).
- 077 — Read-only "recent log lines" panel in Settings.
- 078 — Replace `motion` with CSS transitions in the overlay path; memoize inline markdown.
- 079 — Full overlay-card visual revamp: consolidated decision session (supersedes 043/054/055/056/057/060).
- 080 — Implement the approved news card: compact time meta + full-width expanded summary.
- 081 — TTL progress bar on every rotating card.
- 082 — Weather art: vendored Meteocons + CSS mood backdrops.
- 083 — Football backend: crest fetch/cache, structured wire fields, richer ESPN events.
- 084 — Football card display: sticky scorecard with event-driven expansion.
- 085 — Hide-when-idle option (`resting_state: rail|notch`).
- 086 — SPIKE: hover enablement — dynamic cursor-event tracking on the overlay window (doc lives on as `docs/design/hover-cursor-tracking.md`).
- 087 — Hover primitive: tracking area + rust-derived card rect + `hover-changed` event.
- 088 — Notification history: JSONL storage + Engine write hook (backend).
- 089 — Notification history: Settings History section + invoke commands.
- 090 — Fix `--card-scale` breaking notch geometry and overflowing the window.
- 091 — The cutout card shape + time-and-dots idle.
- 092 — The general card + expanded manifest in the cutout language.
- 093 — The hover consumers: idle weather peek, TTL hover-pause, scorecard reveal.
- 094 — App icon: the notch-cutout glyph.
- 095 — SPIKE: now-playing via a MediaRemote Swift helper (NO-GO; media vertical since removed).
- 096 — Cmux origin on the wire + the cmux card accent.
- 097 — Fix hover-latch desync on hotkey dismiss/skip; supersede top-up hover blind spot; clamp appearance load.
- 098 — Make the Opacity setting work again (below-block only).
- 099 — Docs and dead-code truth sweep.
- 100 — TTL bar becomes the card's bottom edge; celebrations at 2× duration.
- 101 — Football card default TTL 15s (without breaking the inherit heal).
- 102 — Slow the card's expand/contract morphs slightly.
- 103 — SPIKE: verify mediaremote-adapter delivers now-playing (media vertical since removed).
- 104 — Now-playing ambient: mediaremote stream → idle-peek media row (media vertical since removed).
- 105 — Static glow dots, weather art behind the media row, hoverable bare-notch mode.
- 106 — Docs & prototype truth sync (post-external-review drift).
- 107 — Overlay motion correctness: celebration never obscures content, compact→idle state machine.
- 108 — Settings truthfulness: resets hot-apply, actions report their outcome.
- 109 — Settings legibility & semantics: type bump, compliant contrast, real HTML.
- 110 — History richness + small verified overlay polish.
- 111 — Kill the CSS mirror: one shared overlay stylesheet + preview gallery.
- 112 — Migrate settings chrome to Tailwind v4 + shadcn.
- 113 — Shared-ui final pass: refresh token pin, adopt `--font-mono`.
- 114 — Overlay window adopts shared-ui tokens (fonts + easing).
- 115 — Settings-window token hygiene (radius scale, shadow token).
- 116 — Replace rotation-reorder unicode triangles with lucide chevrons.
- 117 — Single-source reduced-motion + coupled animation durations (overlay).
- 118 — Overlay media glyphs → lucide icons.
- 119 — Decompose SettingsApp.tsx; absorb three review deletions.
- 120 — Decompose StatusRailCard.tsx (useExitChoreography + content-branch components).
- 121 — Queue visibility + clear/skip from the settings window.
- 122 — Idle-hover peek: media/weather collision fix + richer ambient weather.
- 123 — Exit-to-bare continuous morph (kill the shape pop-in).
- 124 — Deep-review fix batch for plans 121–123.
- 125 — Idle face: kill the 24/7 idle cost, align the character motion.
- 126 — Settings-window motion polish (tokens, transition-all, list motion).
- 127 — Overlay motion core: peek interrupt, rotation swap, tokens, aria-live.
- 129 — Deep-review fix batch for the animation wave (125–127).
- 130 — Topic news + Search-now (Google News query feeds), honest pacing copy.
- 131 — Weather forecast strip in the peek (weather vertical since removed).
- 132 — External-review fix batch: crest-URL SSRF allowlist, poison-tolerant locks, de-panicked queue.
- 133 — Agent domain model + registry core (v7 ticket 1).
- 134 — `POST /agent/events` endpoint (v7 ticket 2).
- 135 — Noteworthy Agent Events become Notifications (v7 ticket 3).
- 136 — `agent-state` IPC + Agent Board resting state (v7 ticket 4).
- 137 — `[agents]` config, cmux migration, cmux deletion (v7 ticket 5).
- 138 — `notchtap-agent` helper + Claude Code adapter (v7 ticket 6).
- 139 — Codex adapter (v7 ticket 7).
- 140 — Kimi adapter, version-gated (v7 ticket 8).
- 141 — OpenCode TypeScript plugin (v7 ticket 9).
- 142 — Agent Board expanded state + scroll (v7 ticket 10).
- 143 — Settings Agents section + Adapter Health (v7 ticket 11).
- 144 — Open/Focus Session shortcut ⌃⇧A (v7 ticket 12).
- 145 — v7 manual verification + doc closeout (v7 ticket 13; its manual-verification content is superseded by `plans/2026-08-16-anti-slop-deslop.html`).
- 146 — Silenced (Silent Period + Timed Mutes + Breakthrough) and Priority Preemption.
- 147 — Source identity colours, Claude/Kimi card parity, weather TTL unification.
- 148 — Motion token cohesion: crossfade ease, disclosure spring, idle-face durations.
- 149 — Agent Board motion vitals: bounded pulse, accent morph, hero swap, adaptive tick.
- 150 — Celebration integrity: rings that finish, repeats that replay.
- 151 — Scorecard morphs, score odometer, media-bar truthfulness, drift shape.
- 152 — `notchtap-agent doctor`: read-only report of which agent hooks are wired.
- 153 — Source scope review: keep/cut/demote matrix for the ambient sources (decision doc).
- 154 — SPIKE: agent activity digest — data source + surface decision (decision doc).
- 155 — Bound the Kimi version probe; range-check the `[agents]` durations.
- 156 — Wrap the overlay in MotionConfig so reduced-motion reaches it.
- 157 — StatusRailCard: full transform strings instead of y/scale shorthand.
- 158 — Give MetaChip's active state a transition.
- 159 — Segmented control: press feedback, fix the selection-shadow pop.
- 160 — Press feedback for switch, sidebar nav, history disclosure.
- 161 — Reduced-motion coverage for the Agent Board's dot pulse/breathe.
- 162 — Reduced-motion coverage for the card's hover "breathe" scale.
- 163 — Fix `--ease-notchtap` never resolving in the overlay build (critical: every overlay transition was snapping).
- 164 — Scope the shell's pop-bounce curve out of the hover-reveal leg.
- 165 — Match the expand-toggle shell width curve to the manifest it reveals.
- 166 — Give the flank's corner-radius a real entrance transition.
- 167 — Fade the concave "gill" corners instead of hard-popping them.
- 168 — Fix the TTL-bar fill collapsing to ~0px height (grid auto-placement).
- 169 — Render the Agent Board's primary session through the shared notification template.
- 170 — Render Football's promoted match events through the shared notification template.
- 171 — Trim live-scorecard.css's dead rules (plan 170's deferred step).
- 171 — Tab-notch redesign: pull-based icon strip, selection, prefix keyboard model.
- 172 — EXPAND_MS feel-check and retune (320ms → 300ms).
- 173 — Card-chrome width→transform feasibility spike (NO-GO).
- 174 — shared-ui 0.4.0 vendor refresh: adopt the motion trio.
- 175 — Reconcile the icon strip's rust hit-test geometry with the shipped CSS.
- 176 — Place the pulled-tab below-block in its designed grid row.
- 177 — Stop blank pulls: ungated agent sessions on the wire, peek fallback for empty tabs.
- 178 — Deadline-aware prefix watchdog with self-healing forced release.
- 179 — Input-path robustness: live scale in the click monitor, poison-tolerant AppKit locks.
- 180 — Tab-wire test backfill: validator rejections, App seam, identity parity pin.
- 181 — Post-171 docs truth: five stale claims fixed.
- 182 — AgentBoard flank gets the same icon glyphs IconStrip draws.
- 183 — Revert notification manifest hover-expand back to keyboard-only.
- 184 — Wire `agent-viewed-session-changed` to the display; add auto-advance.

## retired, never executed

- 128 — Tavily search-powered news connector: filed, put on hold for
  discussion, never built; retired with the connector framework's
  removal.

## backlog — live items carried from the old plans/README.md

Verified findings that were never planned. Sites named as recorded;
re-verify against current code before acting.

- Prefix `enter`/`o` expand is a no-op: the queue's `expanded` flag never
  reaches the pulled card (plan 184 wired only the `[`/`]` half).
- Kimi version probe still parks a tokio worker: `run_bounded`
  (`kimi_version.rs`) blocks in `thread::sleep` up to 750ms on the
  `publish_if_changed` path.
- News charge fires early on multi-feed configs: `TabWire::new` treats
  the per-source cap as the whole cycle's batch.
- News pull surface is hard-wired empty (`NO_NEWS_STORIES` in
  `NewsBelowBlock`) while the charge machine advertises a batch; the
  story wire shape is an undecided design.
- Spike candidate: an "interactive regions" seam — one `(RegionId, Rect)`
  registry walked by the existing click monitor; shipped stubs (news
  batch nav, session arrows) name it, and it needs no capability change.
- Nit: `icon-strip.css` hand-copies `--overlay-coral` as an rgba literal.
- Frontend toolchain major bump (TypeScript 7 / Vite 8 / Vitest 4):
  spike returned GO; adoption never executed.
- Seventh-audit findings re-verified open at last check:
  - Agent Board's "clears in" renders raw `retentionRemainingMs` with no
    `capturedAtMs` correction (`AgentBoard.tsx`).
  - Agent wire fields (`runtime`/`kind`/`state`/`capabilities`) bypass
    `sanitize_trim`; a local process can inject forged newlines into the
    rotating log (`adapter.rs`).
  - A preempted card does not actually resume at the head of its tier:
    `push_front` is overridden by `best_index_in_tier`'s rotation-order
    rank, contradicting the documented contract (`queue.rs`).
  - The CSS↔TS colour parity pin is a whole-file `toContain`; swapped
    hexes still pass (`sourceColors.test.ts`).
  - Every event after a terminal one mints a new suffixed session
    (phantom Board rows; `reuse_generations` never pruned)
    (`registry.rs`).
  - Silent Period overruns ~1h on the spring-forward day (wall-clock
    minutes slept as real seconds; fix is a sleep cap) (`silence.rs`,
    `lib.rs`).
  - OpenCode plugin's fire-and-forget delivery can race its own
    monotonic `sequence`, dropping `permission.asked` as stale
    (`notchtap.ts`, `registry.rs`).
  - `publish_if_changed` reads `ordered_states` before taking its lock,
    so a slow reader can publish a stale snapshot at a newer revision;
    self-heals on the next change (`board.rs`).
- Motion, parked pending profiling: ambient loops animate inside
  `.card-assembly`'s `filter: drop-shadow` subtree, plausibly
  re-rasterizing the blurred surface per frame; needs Safari Web
  Inspector on a live card before restructuring.
