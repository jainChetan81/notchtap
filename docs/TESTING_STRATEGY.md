# mac-notification-nudge — testing strategy

companion to `ARCHITECTURE.md` (decisions) and `IMPLEMENTATION_PLAN.md`
(build sequence). this doc answers: what gets an automated test, what
doesn't, which framework, in what order tests get written — and, since
the 2026-07-16 merge, what is already done vs what is left. it absorbed
`DEEP_TESTING_PLAN.md` (now deleted) as §9; this is the only testing
doc.

---

## 0. status at a glance (2026-07-22) — done vs left

**done — built, green, ci-gated** (counts live here and only here;
other sections point back rather than repeating them):

| suite | size | where |
|---|---|---|
| rust unit/integration | 481 tests — poller 55 (32 + 23 with plan 083: team-logo/team-ids-by-match/patch_crests, EspnMeta population + flag-off pin, summary/plays parse + classify + dedup + fallback-chain via wiremock), settings 55 (47 + 2 with plan 085 + 5 with plan 068: one per `build_test_event` SourceKind arm, each pinning sibling priority fields to contrasting values so a cross-field read fails + 1 with plan 104: `pin_uneditable_fields` also pins `now_playing_adapter_enabled`/`now_playing_adapter_dir` to the booted value, mirroring `detect_path`), queue 84 (68 + 3 with plan 081: real-promoted_at timing, supersede-extension ttl, dedup-across-a-real-time-gap + 1 with plan 072: cross-tier supersede drops fresh content when the destination tier is full + 9 with plan 093: `hover_enter`/`hover_exit` mechanics (no-op with nothing visible, idempotent double-enter, exit-without-enter no-op, session-duration banking), a card never rotating out while hover-held past its window, rotating out only once the correct amount of ACTIVE time has elapsed after hover-exit, `remaining_ms` freezing for the whole hover session and resuming from where it froze, plus two `proptest` properties pinning both design constraints (never rotates while held; repeated hover cycles never grant more than the rotation window) across randomized hold/cycle sequences + 3 with plan 097: hover-adjusted top-up no longer over-grants extensions to a card with banked hover time, the unhovered top-up path is unchanged, and a hotkey dismiss/skip leaves the newly-promoted item's rotation unfrozen (no inherited hover state)), http 36, notifier 26, rss_poller 28, event 23 (19 + 3 with plan 083: EspnMeta serialization, skip_serializing_if omission, dedup_eq participation + 1 with plan 096: `origin` participates in `dedup_eq` — a changed origin is NOT deduped away, the tripwire the plan named by hand), config 30 (21 + 1 with plan 085 + 1 with plan 083: espn_rich_events default/override + 1 with plan 088: history_enabled default/override + 3 with plan 097: hand-edited-config appearance values clamped on load, non-finite values fall back to defaults, in-range values pass through untouched + 1 with plan 101: espn's own default (15) applies when `default_ttl` is untouched, and is still overridden by the heal when `default_ttl` is customized + 2 with plan 104: now-playing's two fields default off (feature toggle)/on (kill switch) and are independently overridable), crests 8 (new with plan 083: cache-miss/hit, one-attempt-per-team, fetch success/failure/oversized, filename sanitization), weather_poller 21 (16 + 3 with plan 082: `is_day` fixture parse, `is_day` defaults-to-zero-when-absent, alert event carries `wx-condition`/`wx-is-day` detail pairs + 2 with plan 074: lookahead rounds down just under the boundary, lookahead rolls over to the next day at midnight), presentation 11, lib 16 (13 + 3 with plan 091: `cutout_height_js_value` positive/zero/negative inset, mirroring `cutout_width_js_value`'s own two-case pin), engine 14 (10 + 1 with plan 081: the single-emit regression test; plan 073's `AmbientSlot<T>` refactor changed no counts — pure refactor, zero test edits + 3 with plan 088: `accept` records a `OneShot` event to history, `accept` never records a `Recurring` event (the core design-decision tripwire), `accept` with history disabled writes nothing), status 11 (7 + 4 with plan 104: `MediaStatus`/`NowPlayingSummary` camelCase serialization, `current` serializes as `null` when absent, `snapshot` carries the media summary + gate, and the enabled-gate-off-hides-`current` belt-and-suspenders case even when a session exists in the ambient slot), logging 7, net 4, hover 29 (net unchanged by plan 093, composition changed: the 8 `status_rail_active` tests plus the old full-window-height y-span pin — 9 tests total — removed; that predicate and its rust mirror are gone entirely, the y-span's idle-peek input became hover hysteresis, not ambient-data availability — replaced by 9 new y-span/height tests: idle-peek-closed height is the cutout height alone (both modes), idle-peek-open adds `IDLE_PEEK_BELOW_BLOCK_H`, showing/expanded each add their own conservative below-block estimate, `idle_peek_open` is ignored once `visible` is true, the height side of the `min(..., 100%)` window cap, and the actual behavioral fix — a point in the old dead zone below the idle card no longer registers as hovered; the plan-091 width-formula tests (16, `active_card_rect`'s idle/showing/expanded cases, cutout-term scale exemption, window-width cap) are untouched), history 7 (new with plan 088: append-then-read-recent round trip, missing-file-reads-as-empty, last-n truncation, malformed-line-skip-not-fatal, size-rotation-creates-a-backup, an empty current file never rotates, `clear` removes the current file and its backup), now_playing 16 (new with plan 104: the ambient now-playing source — `apply_event`'s diff-merge cases (fresh session, diff merge over an existing session, artist-less session, session-end via empty-payload/bare-null/missing-payload-key, a diff that never establishes a title stays no-session, a malformed line is ignored not fatal, fractional-seconds-to-ms conversion, `parentApplicationBundleIdentifier` preferred over `bundleIdentifier`), the `should_spawn` four-condition gate, the `changed` compare-before-push predicate, and the `Supervisor` backoff state machine (escalates 5s→10s→30s→60s then caps, resets to the floor)) | `cargo test` from `src-tauri/` |
| rust doc-tests | 3 — public `queue`/`event` apis | same `cargo test` run |
| frontend | 231 tests — presentation tables 12, presentation facts 5 (4 + 1 with plan 091: `cutoutHeight` rejects zero/negative/non-number/missing, mirroring `cutoutWidth`'s own reject list), inline markdown 7, weatherArt table 7 (plan 082: gallery day/night pairings, Cloudy/Storm/Snow texture assignment, Cloudy-never-keys-to-overcast, unknown-condition neutral fallback), useDelayedSwap hook 3, slot-state hook 30 (22 + 2 with plan 081: ttlMs/remainingMs validator accept/reject + 3 with plan 083: espn block absent/valid/malformed + 3 with plan 096: `origin` accepts each of the five wire values, rejects an unrecognized value, rejects a missing field), status-state hook 16 (15 minus 2 with plan 099: the orphaned `statusRailActive` predicate and its describe block were deleted — no production caller, superseded by 091's width-split collapse and 093's rust-mirror removal + 3 with plan 104: accepts a valid now-playing summary, accepts one with null artist/album/durationMs/appBundleId, and rejects a missing media gate or a malformed now-playing summary), StatusRailCard 70 (62 with plan 084/etc — 1 with plan 091: the plan-034 idle/idle-status width-split pair collapsed into one regression-pin test now that the split itself is gone; every weather-mood-class/news-shade-class assertion moved from the outer `.card-assembly` to `.below-block` in the same plan, selector-only, no behavior change; plan 092 retargeted the pill/masthead/manifest selectors this same file already covered onto the chip-converged/full-width vocabulary — same count, no new cases + 4 with plan 093: no below-block mounts while idle and not hovered (091's shell untouched), a `.below-block.idle-peek` mounts while idle and hovered, the peek never mounts while a card is showing regardless of `hovered`, `hoverPaused` reaches the TtlBar while showing and hovered + 3 with plan 096: the cmux accent (chip + below-block hairline) renders for `origin: "cmux"`, is byte-absent for the other four origins, and never touches the priority accent channel — same priority, different origin, identical shell classes + 2 net with plan 105 (Step C, replacing 085's 3-test "resting_state: notch" suite with 5): bare mode paints no below-block/clock/status-dots while idle and not hovered, hovering bare mode still reveals `.below-block.idle-peek` (the actual bug fix — 085's old `return null` made the mode unhoverable), a showing card renders byte-identical DOM in notch vs. rail mode, and the exit animation still plays before the shell settles bare rather than vanishing), Track slider 6, settings form 24 (16 + 1 with plan 085 + 5 with plan 089: History section renders entries newest-first from a mocked `get_history`, empty-history "nothing recorded yet" copy, history-disabled "history is off" copy, the clear control's two-step confirmation gating `clear_history`, `history_enabled` toggle round-trips into the saved config + 2 with plan 104: the `now_playing_enabled` toggle round-trips into the saved config, and the kill switch never renders a control), App render 11 (8 + 3 with plan 091: the HUD synthetic 200px/32px cutout vars apply in hud mode, a measured cutout always wins in notch mode, and the hud-mode null-coalescing fallback), TtlBar 11 (7 with plan 081: anchor/re-anchor/clamp/reduced-motion/unmount-cancel + 4 with plan 093: freezes while `hoverPaused`, resumes from where it froze granting no extra time, a bare toggle never re-anchors the countdown, byte-identical when `hoverPaused` is omitted), StatusDots 9 (6 with plan 091, replacing IdleView rail's 8 — the old text-pill rail is gone; per-source active/dim state, fixed dot order, no text content, and the no-status-prop fallback + 3 with plan 092: paused forces every dot dim even when otherwise enabled, the pause glyph renders only while paused, and only when `status` is actually present), IdleHoverPeek 20 (new with plan 093: closed by default, opens on `hovered` — never CSS `:hover`, the timeline-alone fallback with no ambient data, the weather scene + condition `.chip` (never a pill), the football scorecard reveal outranking weather, no-status-prop rendering, the mount/close-delay lifecycle including a re-open-mid-close case, and the reduced-motion immediate-unmount path + 8 with plan 104: the media row renders and outranks the weather scene when a now-playing session is available, a paused session renders ⏸, football still outranks media, no media row when `media.current` is null, and `glyphForBundleId`'s four glyph-selection cases (Music, TV, browser case-insensitive substring, fallback) + 2 with plan 105 (Step B): the weather backdrop stays mounted behind the media row instead of being replaced by it, and is absent when a live match is showing (the scorecard keeps its own visual)) | `npx vitest run` |
| ci (v4) | fmt, clippy `-D warnings` (`--locked`), cargo test (`--locked`), cargo-audit, npm audit, tsc, vitest, vite build, `sh -n` cli syntax check, swiftc compile check | every push + pr |

every example case listed in §4 for v1/v2/v3 components has a passing
test; the v4 §4.3 expansion (exhaustive status codes, queue edge
interleavings, sweep timing) is in; the v3 notifier suite (§4.9 —
telegram) landed 2026-07-16; the v3.6 single-slot rotating overlay
suite (§4.10) landed 2026-07-17, superseding §4.1's 3-item-cap queue
example cases; the v5 settings-window rust suite (§4.11), the v5
rss poller + status-rail news card suite (§4.12), and the v5.1
appearance/test-notification additions (queue test-promotion,
`send_test_notification`, `set_appearance`, AppearanceSection vitest)
all landed 2026-07-17 — §4.12's manual live-feed check is the only
piece still open, tracked in its own section. the queue's `+1` above
is the §9.1 generated-adversary property test (256 cases folded into
one `#[test]`, not 256 separate tests); http's `+5` are §9.2's burst/
boundary integration cases — both landed 2026-07-18, see §9. plan 033
landed the same day: the queue-slider batch counters, the Track slider
suite, and the expand-all rewrite of plan 008's expanded-semantics
cases (every promotion expanded, auto-retract at half the base window,
manual-only 3× window — proptest invariants 8/9 re-pinned to match).
plan 034 landed the idle source-status rail the same day too: the
`status.rs` serialization/change-guard/event-pin cases, the poller's
fixture-driven live-match summary (populated/cleared, no live network),
and the status-state hook + IdleView chip vitest suites; its manual
checks (idle "all clear" with espn off, a live fixture poll showing the
match chip) are operator-owed, same as §4.12's. plan 037 landed the
Engine (`src-tauri/src/engine.rs`) on 2026-07-19: every queue mutation
now flows through one module (`apply`/`apply_blocking`/`accept`), the
queue's enqueue interface is clock-agnostic (`now: Instant` at every
entry point, `enqueue_at` deleted), the rotation loop moved inside as
`spawn_rotation`, and the protocol's own tests live in the new `engine`
row — http's wake regression test and lib's two heartbeat tests moved
there (moved, not lost), which is why http/lib each shrank. plan 039
landed the opt-in espn live-match card the same day: the poller's three
new cases (flag-off regression pin, flag-on Topic/rotation shape,
flag-on end-to-end queue collapse + connector fan-out) — poller 19→22,
rust 296→299; frontend unchanged (fixture field additions only, no new
vitest cases). plan 040 Part B landed the weather source the same day:
the `weather_poller.rs` fixture suite (ambient summary, all four
edge-trigger cases for rain/hot/cold, the WMO-code mapping, the
nearest-hour rain lookahead) against a captured Open-Meteo response
(`tests/fixtures/open-meteo-bangalore.json`, no live network), the
engine's `update_weather` wake-only-on-change twin, the status.rs
`WeatherSummary`/`WeatherStatus` serialization cases, and the config/
settings surface (weather fields, validate ranges, the 5-source
rotation-order permutation) — rust 302→321; frontend 107→110 (weather
chip in IdleView, the status-state validator's weather branch, fixture
updates). the outer spawn loop's live HTTP call is operator-verified,
same as every other poller. plan 042 landed the live-match scorecard
presentation the same day: per-side (yellow, red) card bucketing in the
poller (`MatchSnapshot.home_cards`/`away_cards` replacing the aggregate
`cards`, gated on the structural `team.id` cross-reference), Clock +
per-side Cards `meta.details` cells on flag-on match events, and the
collapsed StatusRailCard detail lines — poller 25→30, rust 321→326;
StatusRailCard 21→23, frontend 110→112.

plan 083 landed the football backend (crest fetch/cache/serve,
structured `EspnMeta` on the wire, richer ESPN events with a mandatory
summary→plays fallback chain) in three commits from a 352/144 baseline:
workstream b added `EspnMeta` (league/abbrev/score/clock/cards, plus
crest-path fields) as an `EventMeta`/`SlotState::Showing` field, gated
behind `espn_live_card` exactly like the existing Clock/Cards detail
cells, with a `skip_serializing_if`-backed flag-off byte-identical pin
at the JSON level (event 19→22); workstream a added the `crests` module
(fetch-on-cache-miss, one attempt per team per process lifetime, atomic
writes, silent failure) and wired `SbTeam.logo` parsing plus a
`patch_crests` step in the poll loop, served to the webview via tauri's
asset protocol (poller 32→55 combined with workstream c below, crests
0→8); workstream c added the opt-in `espn_rich_events` flag (default
false) and the `summary`/`plays` fetch chain — four new `EventSignal`
variants (foul/offside/var_check/substitution), a `classify_rich_type`
that drops scoreboard-owned and unrecognized types outright, a
per-match (kind, clock) dedup set, and wiremock tests exercising the
real fallback orchestration (`poll_rich_events`) for both trigger
conditions (404, empty) plus the newest-page-only pagination rule
(config 22→23). Total: rust 352→387, frontend 144→147 (the `useSlotState`
validator's absent/valid/malformed `espn` block cases). The one real
network path this plan touches (the crest fetch itself, and the
summary/plays endpoints during a live match) is unverifiable in CI by
design, same posture as every other poller here — operator-owed.

plan 087 landed the hover primitive from a 390/181 baseline (080–086
merged): a new `hover.rs` module — `active_card_rect` (mirrors
`styles.css`'s width breakpoints, `--card-scale`-aware), `status_rail_active`
(a rust port of `useStatusState.ts`'s `statusRailActive`, since rust
had the underlying data but not the predicate), and
`css_top_down_to_appkit_y` (the one named helper for the AppKit
bottom-left-origin vs CSS top-down y-flip) — plus its own 30-case
table-driven suite (rust 390→420); a `panel_event!` tracking-area
handler on the existing `OverlayPanel` (`lib.rs`) wiring
mouseEntered/mouseMoved/mouseExited to a `hover-changed` event, emitted
only on transitions, never per mouse-move; and `Engine::status_snapshot_blocking`,
a non-emitting sibling of `emit_current_status_blocking` the hover
handler reads on every tracking-area callback so it never re-emits
`status-state` off a mouse move. `set_ignore_cursor_events(true)` and
`capabilities/default.json` are untouched (git-diff-verified) — the
whole mechanism rides the empirical finding in
`docs/design/hover-cursor-tracking.md` §2 that a tracking area fires
independent of click-through. Frontend: one `.hovered` diagnostic CSS
class (mirrored in `preview-overlay.css`) and its two StatusRailCard
cases (frontend 181→183) — no actual hover FEATURE (081's TTL
hover-pause, 082's weather peek, 084's rail→scorecard reveal, idle
expanded-on-hover) is built here; each consumes this signal as its own
follow-on plan. The tracking-area wiring itself has no automated test
(AppKit callback plumbing, manual-only per §5, same treatment
`apply_overlay_native_config` already gets) — the manual smoke check
(hover near a card's top/bottom edge, then confirm a menu-bar icon
under the window's dead margin still responds to a real click) is
operator-owed and unverified in this pass (no notch macbook, no live
GUI session in this environment).

plan 104 landed the now-playing ambient source from a 458/214 baseline:
the vendored, SHA-pinned `mediaremote-adapter`
(`src-tauri/vendor/mediaremote-adapter/`, commit
`3ac3d4bdf862c7b5399b4fba4df5689f5c38609a`, plan 103's own inspected
tree) plus a new `now_playing.rs` module owning the supervised streaming
child (`apply_event`'s pure diff-merge core, the `should_spawn`
two-config-flag-plus-two-file-exists gate, and a `Supervisor` backoff
state machine distinct from `poller::Backoff` — reset only after 5
minutes of continuous healthy runtime, not on every good line) — rust
458→481 (config +2, status +4, settings +1, now_playing +16 new).
Frontend: `useStatusState.ts`'s `isValidNowPlaying` validator,
`IdleHoverPeek.tsx`'s new media row (football > media > weather >
timeline precedence) with its `glyphForBundleId` app-glyph map, and one
settings toggle — frontend 214→227. Ambient-only by design decision: no
new `SourceKind`, no queue/card interaction (`event.rs` byte-untouched,
git-diff-verified). This machine's `/usr/local/lib` is not writable
without sudo (a real environment fact, not a bug) — the vendored tree
was built and empirically probed (entitlement `test` subcommand exit 0;
`stream` mode's connection-time payload confirmed live as
`{"type":"data","diff":false,"payload":{}}`, matching 103's own finding)
from its own local, git-ignored `build/` directory before that finding
prompted a same-day revision: `now_playing_adapter_dir`'s default moved
to `"$HOME/Library/Application Support/notchtap/mediaremote-adapter/"`
(the macOS-conventional, user-writable location — `dirs::home_dir()`,
the same resolution `Config::load` already uses, falling back to the
old `/usr/local/lib` path only if home can't be determined at all), so
an operator's first `just build-media-adapter` no longer needs root.
Re-verified end-to-end on this machine after the revision: `mkdir`/`cp`
into the new path succeeded with no sudo, and the installed copy's
`test` subcommand — run from that installed path, not the local
`build/` tree — exited 0. The settings-toggle live smoke check (opting
into `now_playing_enabled` and confirming a real session appears in the
idle peek) remains operator-owed, same posture as every other
subprocess-backed source here.

**left — each is a decision with an owner section, not a gap:**

| item | status | where |
|---|---|---|
| deep testing work order — poller fuzz, frontend timing fuzz | §9.1 (queue proptest) and §9.2 (http burst/boundary) **landed 2026-07-18**; §9.3/§9.4 remain **parked** — no new trigger for those two | §9 (the full, implementation-ready plan) |
| ~~outbound connector tests~~ | **landed with v3 (telegram)** 2026-07-16 | §4.9 |
| ~~single-slot rotating overlay tests~~ | **landed with v3.6** 2026-07-17 (branch `v3.6-rotating-overlay`, not yet merged) | §4.10 |
| ~~v5 rss poller + news-card suite~~ | **landed 2026-07-17** (see §0) | §4.12 |
| v5 settings-window suite | **landed 2026-07-17, rust side and ui side both** (see §0) — validate/mask/round-trip/merge/write paths, plus the settings form + vitest cases (`IMPLEMENTATION_PLAN.md` §4.5 step 5) | §4.11, `V5_TECHNICAL_SPEC.md` §7 |
| manual checks not yet run (v3.6 hotkey keypress + Spaces/fullscreen survival, v5 news live-feed check) | needs the macbook + (for news) `rss_enabled = true` | §4.10, §4.12, §5, `IMPLEMENTATION_PLAN.md` §3.6.1/§4.6.1/§6 |
| `test-cli.sh` for the `notchtap` script | only if the script grows | §8 |
| manual hardware checklist | recurring per change — never "done" | §5, `IMPLEMENTATION_PLAN.md` §6 |

---

## 1. shape of the pyramid for this project

this project's pyramid is unusually bottom-heavy and has almost no
automated top layer, for a concrete reason: the highest-risk logic
(queue ordering, ttl expiry, event parsing) is pure and deterministic,
while the highest-visibility behaviour (notch-cutout geometry on real
hardware, css animation timing) depends on things a test runner can't
see — real `NSScreen` data on two specific physical macs, and rendered
visual output. don't fight that; put the automation budget where it
pays off.

```
        /  manual only   \      2 physical machines, real windows,
       /  (§5, checklist)  \     visual correctness — not automatable
      /------------------  \
     /   integration (some)  \   http layer, tauri command dispatch
    /------------------------  \
   /   unit tests (many, fast)   \  queue, event bus, parsing, reducers
```

---

## 2. framework choices

| layer | framework | why |
|---|---|---|
| rust unit/integration | built-in `cargo test` (`#[test]`, `#[tokio::test]`) | no extra dependency needed; idiomatic default; tauri already pulls in tokio, so async tests are free |
| rust http layer | `axum` + `tower`'s `ServiceExt::oneshot` (dev-dependency: `tower = { features = ["util"] }`) | lets the `/notify` route be tested in-process — no real socket bind, no port cleanup, no flaky "address in use" failures. this is also why axum is the pick over `tiny_http`: `IMPLEMENTATION_PLAN.md` §1.2 didn't pin an http crate — closing that gap here, in favour of axum specifically for this testability property |
| rust external http mocking (v2 espn poller) | none — dropped 2026-07-16 (was `wiremock`) | the poller design (v2 spec §3) keeps the fetch loop thin and untested; parsing, delta logic, and backoff are pure functions tested directly against captured fixture files, so nothing needs an http mock |
| frontend unit/component | `vitest` + `@testing-library/react` | vitest is free — the tauri react template already runs on vite, so it shares config and is fast; testing-library tests behaviour (what's rendered) not implementation details |
| rust doc-tests | built-in (`cargo test` runs them) | added 2026-07-16 — the public `queue`/`event` apis carry runnable examples that double as documentation. these are *not* the coverage layer (the `#[cfg(test)]` modules are); a doc-test exists to keep the documented usage honest, so keep them few and lifecycle-shaped, not exhaustive |
| deep testing, when un-parked (§9) | `proptest` (rust), `fast-check` (web, droppable) | dev-dependencies only; see §9 for the full rationale per section |

---

## 3. where to actually do tdd

red-green-refactor (write the failing test first) is worth the
discipline where the logic is pure, deterministic, and wrong-by-default
if untested. that's a short, specific list:

- **single-slot priority queue** (v3.6, formerly "notification queue" —
  fifo ordering, cap-3 enforcement, ttl-based expiry; see §4.10) —
  tier-strict promotion, fast-path never-jump, rotation (one-shot vs
  recurring), topic supersession with a capped extension
- **event bus / dispatch router** — event type routing, malformed
  payload rejection
- **`/notify` http handler** — request parsing, validation, status
  codes
- **notch/hud mode decision function** — see §4, this is the one piece
  of the native layer that's actually a pure function once isolated

tdd is **not** worth it, and shouldn't be forced, for:

- css animation timing/easing — write the animation, eyeball it, adjust
- the native swift `NSScreen` shim itself — there's nothing to assert
  against without the physical screen
- tauri window creation/positioning calls — thin wrappers around a
  native api; a unit test here would just be re-asserting the mock

---

## 4. component-by-component test plan

each subsection notes its status; "done" means every example case
listed has a passing test (the §6 bar), not that the component is
frozen.

### 4.1 notification queue (rust) — superseded by §4.10 (v3.6, 2026-07-17)

kept for historical record: this section describes the pre-v3.6
3-item-cap, pure-fifo `NotificationQueue`. that type no longer exists
— `queue.rs` now holds `SingleSlotQueue` (single slot, priority-tiered
waiting, rotation instead of ttl). the current, accurate example-case
list is §4.10; don't add new cases here.

- **type**: unit, tdd
- **coverage target**: every state transition (~100% branch coverage —
  small, deterministic module, this is the core value of the app)
- **example cases**:
  - enqueue 1 item → visible immediately
  - enqueue 4 items with cap=3 → 4th stays queued, not visible
  - item ttl expires → removed from visible set, next queued item
    promoted
  - queue empty → no-op, no panic
  - two items with identical ttl expire in enqueue order (fifo
    tie-break)
  - enqueue the 51st item while 50 are already waiting (`max_queued`,
    locked in `ARCHITECTURE.md` §3) → rejected, queue state unchanged,
    matches the `429` asserted at the http layer in §4.3 below
  - paused + free visible slot + enqueue → item lands in `waiting`,
    not `visible` (pause disables promotion even inside enqueue)
  - paused + visible item's ttl elapses → item removed on the next
    `expire_and_promote`, freed slot stays empty (expiry runs while
    paused; promotion doesn't)
  - resume followed by `expire_and_promote` → buffered items promote
    fifo into the free slots
  - `max_queued` enforced identically while paused (51st waiting item
    rejected)
  - (v4 expansion) burst-at-cap fifo/ttl accounting, exact expiry
    boundary (`>=` semantics), pause/resume interleavings with
    exactly-once promotion, resume promotes only up to cap
- property-based invariants on top of these: §9.1, parked

### 4.2 event bus / dispatch router (rust) — ✅ done

- **type**: unit, tdd
- **coverage target**: ~100% — every event type + malformed-input path
- **example cases**:
  - well-formed `generic` event → routed to queue
  - unknown event `type` field → rejected, not silently dropped
  - missing required field (`title` or `body`) → rejected with a
    specific error, not a panic

### 4.3 `/notify` http handler (rust) — ✅ done

- **type**: integration, tdd, via `tower::ServiceExt::oneshot`
- **coverage target**: every response code path
  (200/202/400/413/429 + method-not-allowed all asserted)
- **example cases**:
  - valid POST body → 200 with `{"status": "accepted"}`, event
    forwarded to bus
  - malformed json → 400, no crash
  - wrong content-type → 400
  - request from anything other than loopback — confirm the listener
    is bound to `127.0.0.1` only (this is a security boundary, not just
    a correctness one — worth a dedicated test asserting the bind
    address, not just handler behaviour)
  - queue already at `max_queued` (50, locked in `ARCHITECTURE.md` §3)
    → `429`, not `500` or a silently dropped event
  - queue paused → `202` with `{"status": "paused", "queued": <n>}`,
    event buffered into `waiting`, not dropped — and still `429` when
    full while paused
  - **plan 035 rich relay**: POST with `subtitle` + `details` round-trips
    into `current_slot_state` (subtitle string, details `{label,value}`
    pairs); caps enforced server-side (9 pairs → 8; label/value truncated
    with `…`; empty-label pairs dropped; empty subtitle → `None`); and a
    payload with **neither** field yields `None`/empty — the byte-identical
    back-compat guarantee. `sanitize_subtitle`/`sanitize_details` are also
    unit-tested directly for the boundary numbers (120/40/200 chars)
- burst accounting and exact 413/ttl boundaries: §9.2, parked

### 4.4 notch/hud mode decision (rust, `presentation.rs`) — ✅ done

- **type**: unit, tdd
- **coverage target**: 100% — cheap, and it's the one native-adjacent
  decision that's actually testable
- **example cases**: isolated as a pure function —
  `fn presentation_mode(safe_area_top_inset: f64) -> Mode` — so the
  test can pass in `0.0` (mac mini → hud) and a positive value
  (macbook → notch) without touching `NSScreen` at all. the actual
  `NSScreen.main?.safeAreaInsets.top` call stays a thin, untested
  boundary that feeds this function — don't let the untestable native
  call and the testable decision logic live in the same function.

**subprocess boundary (`notchtap-detect`)**: `ARCHITECTURE.md` §5 locks
the swift↔rust integration as a standalone cli (`notchtap-detect`)
invoked via `std::process::Command`, printing json to stdout. that json
parsing step is a second testable unit distinct from the pure decision
function above — the subprocess call itself (spawning `notchtap-detect`)
stays untested, same reasoning as `NSScreen`, but everything downstream
of "here is a string of stdout" is fair game:

- **type**: unit, tdd
- **coverage target**: every parse/failure path
- **example cases**:
  - well-formed json on stdout → parsed into the expected struct
  - malformed/truncated json → handled explicitly (fall back to hud
    mode, log, don't panic), not an unwrap
  - non-zero exit code from `notchtap-detect` → same explicit fallback,
    not a panic
  - binary not found on `PATH` → same explicit fallback — this is the
    most likely real-world failure (a fresh macos install, or the shim
    not yet built) and the one most worth a dedicated test

### 4.5 frontend visible-notification render state (react/ts) — ✅ done

- **type**: unit (vitest), tdd
- **coverage target**: every transition in the enter → hold → exit
  lifecycle, plus cap/ttl interaction with the rust-side queue's
  contract
- **example cases**:
  - receiving a tauri event adds an item to visible state
  - item past ttl removes itself from visible state
  - (v2 hardening) an item whose wall-clock deadline has passed is
    removed by the 1s sweep even if its setTimeout timers never fired —
    simulates system sleep / webview timer throttling (v2 spec §6.1);
    multi-item and not-yet-due sweep cases included (v4 expansion)
  - the frontend renders every `notification-promoted` event it
    receives without enforcing any cap itself — cap and promotion
    authority live rust-side (spec §8's queue-authority resolution); a
    4th visible item can only appear because rust promoted it. (an
    earlier draft of this case read "4th concurrent item does not
    render until a slot frees" — that predates the queue-authority
    resolution and described a frontend-side cap that must not exist.)
- generated emit/clock-jump schedules: §9.4, parked

### 4.6 animation rendering (css/react) — manual by design

- **type**: manual only
- **why no automated visual regression**: the tooling cost (screenshot
  diffing, baseline management) isn't justified for a single generic
  template on a personal tool. revisit only if v2's per-event-type
  animation table (§4 of `ARCHITECTURE.md`) grows large enough that
  regressions become hard to eyeball.
- **revisit trigger evaluated 2026-07-16**: v2's animation table landed
  (three event types: `generic`, `score_update`, `match_state`). three
  keyframe sets are still trivially eyeball-able, so the decision
  stands. re-evaluate if the table reaches ~6+ types or per-type
  styling starts regressing during unrelated css edits.
- **manual check**: covered by `IMPLEMENTATION_PLAN.md` §6 checklist

### 4.7 espn scoreboard poller (v2, rust) — ✅ done

- **type**: unit. the fetch loop stays thin and untested (§5.1);
  parsing and all delta logic live in pure functions
  (`parse_scoreboard`, `diff_scoreboard(prev, fetched)`) tested
  directly against captured fixture files — no http mocking (wiremock
  dropped 2026-07-16, see §2). not tdd-first (the external api shape
  was observed before tests asserted against it, per
  `IMPLEMENTATION_PLAN.md` §2.1)
- **example cases**:
  - well-formed scoreboard response → normalized `score-update` event
  - score delta against the snapshot → one `ScoreUpdate` per changed
    match; unchanged matches emit nothing
  - status delta (pre→in, in→halftime, →final) → one `MatchState`
  - first sighting of a match → no event (silent baseline, no restart
    flood)
  - match gone final → snapshot entry evicted immediately (after the
    full-time event)
  - match merely absent from a poll → carried forward, not evicted; a
    goal scored during the blip is still caught on reappearance;
    sustained absence (10 consecutive misses) evicts
  - malformed/empty json → no crash, no event emitted
  - http timeout / 5xx from espn → per-league backoff, no event
    emitted, no crash; the other leagues keep polling
  - never call the live espn endpoint from a test — fixtures only
- parse fuzz beyond hand-picked malformed fixtures: §9.3, parked

### 4.8 cmux relay ingestion (v2) — manual by design, live-verified

- **type**: manual (revised 2026-07-16 — this section originally
  planned rust unit tests for env-var parsing, written before the cli
  was locked as a flags-only shell script. there is no rust env-var
  parsing to test: cmux's notification command passes
  `$CMUX_NOTIFICATION_*` through shell expansion into `notchtap`'s
  flags, and the fold/empty-subtitle logic lives in the script.)
- an optional automated `test-cli.sh` is tracked in the v2 spec §8 —
  add it only if the script grows
- **example checks (manual)**:
  - all three env vars present → notification shows
    "subtitle — body" folded correctly
  - empty/unset `CMUX_NOTIFICATION_SUBTITLE` → body passes through
    untouched, no stray separator
  - end-to-end: live-verified 2026-07-16 on the mac mini (real claude
    code "needs input" alert surfaced through the overlay)

### 4.9 outbound connectors (v3 — telegram; whatsapp/twilio demoted)

(rewritten 2026-07-16 when v3 locked telegram-first — see
`IMPLEMENTATION_PLAN.md` §3; the old whatsapp/twilio framing predated
that decision. the no-live-calls rule is unchanged and applies to any
future connector too.)

- **type**: unit (pure fns, tdd) + `wiremock` integration (send path)
  + http-layer fan-out cases in §4.3's suite
- **rule**: no test ever sends a real telegram message. wiremock only.
- **pure, tdd'd first**:
  - `format_message` per event type + a nasty-characters escaping case
    (`<b>`, `&`, underscores, backticks in the body)
  - `escape_html` ampersand-first (no double-escaping)
  - `on_send_failure` — every arm: first transient → retry, first 400 →
    plain resend, fatal → drop, any second failure → drop
  - `ConnectorHandle::offer` — drop-on-full, never blocks
  - secrets loader against temp files (never `$HOME`): valid `0600`
    loads; missing file, non-`0600` perms, malformed toml each yield
    their specific `SecretsError` variant
  - config gate: `[connectors.telegram] enabled` parses, defaults to
    `false`
- **wiremock (send path)**:
  - 200 → exactly one request, html `parse_mode` present
  - 400 → exactly one plain-text resend, `parse_mode` absent
  - 5xx → exactly one retry, then drop
  - 401 (fatal) → no retry at all
- **http fan-out (in §4.3's suite)**: accepted push lands in a test
  connector channel; 429-rejected push does not; paused `202` push
  **does** (acceptance succeeded — v3 spec §1)

### 4.10 single-slot rotating overlay (v3.6 — priority queue, rotation, hotkey expand)

landed 2026-07-17 (branch `v3.6-rotating-overlay`, code-level contract
in `docs/V3_6_TECHNICAL_SPEC.md`). supersedes §4.1's notification-queue
example-case list — same file (`queue.rs`), renamed type
(`SingleSlotQueue`), materially different model (one slot, not three;
priority tiers, not pure fifo; rotation, not ttl).

- **type**: unit, written alongside the implementation against the
  frozen types in `V3_6_TECHNICAL_SPEC.md` §3/§4
- **coverage target**: every state transition in the single-slot
  model — tick/rotation, tier-strict promotion, fast-path,
  supersession (including the hard extension cap), pause/resume, and
  the `slot_state_if_changed` change-guard
- **example cases** (`queue.rs`, see §0 for the current count):
  - `tick`: never-interrupt (a `High` enqueue while something is
    Visible does not promote until the Visible item's own rotation
    elapses), tier-strict promotion order, fifo within a tier,
    `OneShot` drops forever, `Recurring` requeues to the back of its
    **own** tier
  - fast-path: a push with any tier non-empty never fast-path-promotes,
    even a `High` push arriving while only `Low` is waiting — the
    3-tier generalization of the old `fast_path_never_jumps_waiting_items`
  - supersession: a visible-item supersede updates
    payload/priority/rotation and grants a capped extension only when
    remaining time is already below the 2s floor (`promoted_at` is
    never mutated — only `extension_secs`); a burst of 25 rapid
    below-floor supersedes still rotates the item out at exactly
    `base_window + 6s`, never later, regardless of how many land; a
    same-tier waiting supersede keeps its queue position; a
    priority-changing supersede moves to the back of its *new* tier
    (not its old one, not the front)
  - per-tier cap: a full `Low` tier rejects a new `Low` push while a
    simultaneous `High` push is still accepted (`max_queued_per_tier`,
    independent per tier — a `Low` burst can't starve `High`'s own
    waiting room)
  - pause/resume: pause gates promotion, not rotation (an already-
    Visible item still ages out while paused); resume promotes
    immediately on the next `tick`, not the next heartbeat
   - `slot_state_if_changed`: suppresses a re-emit when nothing changed
     between two ticks; an actual promotion, rotation-to-empty, or
     expand toggle always emits
   - **expanded semantics** (plan 008, 2026-07-17 — `queue.rs`):
     automatic for `High` on both promotion call sites (the
     `enqueue_new` immediate-promote fast path, and `promote_next` via
     `tick`/rotation), reset to `false` for every non-`High` promotion
     (a leftover manual expand from the previous item never leaks onto
     the next one), the expanded rotation window applies to an
     auto-expanded `High` item exactly as it does to a manually-toggled
     one, and `toggle_expanded` is a no-op while the slot is Empty (an
     idle press arms nothing for whatever promotes next)
- **`Priority` ordering** (`event.rs`): `Low < Medium < High` pinned by
  a dedicated test — the array-index promotion logic in `queue.rs`
  depends on declaration order matching `Ord`, so a rustfmt/refactor
  reorder would silently break promotion without this test
- **`SlotState` wire contract** (`event.rs`): a `serde_json::to_value`
  snapshot test on `SlotState::Showing` pins the exact camelCase field
  names. this caught a real bug during implementation:
  `#[serde(rename_all = "camelCase")]` on the enum only renames the
  variant *tag* ("showing"), not fields inside the struct variant
  (`event_type` stayed `event_type` instead of becoming `eventType`)
  — needs the additional `rename_all_fields = "camelCase"` attribute
  too. flagged here because it's exactly the drift
  `V3_6_TECHNICAL_SPEC.md` §5.2's "integration risk" note warned
  about, and the snapshot test caught it on the first real run rather
  than shipping a frontend that silently never renders anything.
- **hotkey no-op branch** (`lib.rs`): `toggle_manual_expand`'s pure
  decision (no-op while a `High`-priority item is Visible — because
  it's already auto-expanded, per plan 008 — toggles otherwise) is
  unit-tested directly against a `SingleSlotQueue` and a
  `tauri::test::mock_app()` handle, bypassing the actual OS hotkey —
  same split as §4.4's subprocess boundary
- **frontend** (`useSlotState.ts` + `App.tsx`, 10 of the 14 total
  frontend tests): renders `empty` as nothing; renders `showing` with
  the right priority/expanded classes; a new `slot-state` payload
  replaces content directly, without an intermediate empty frame;
  listener cleanup on unmount
- **cli** (`notchtap --priority low|medium|high`): manual only, same
  as the rest of the script (§4.8) — `sh -n` syntax check is the only
  automated gate

**manual-only, not automatable** (extends §5):
- the global hotkey keypress actually toggling expand on real
  hardware — the pure decision logic above is unit-tested; os-level
  registration and keypress delivery are not
- the window surviving a Spaces switch and staying visible over a
  fullscreen app (`NSWindowCollectionBehavior`)
- a live espn goal auto-expanding (`High` priority) and rotating out
  correctly under the new single-slot model, on the macbook

### 4.11 settings window (v5 — rust side landed 2026-07-17; form held for the ui migration)

code-level contract in `V5_TECHNICAL_SPEC.md` (§7 is the source this
section mirrors); build sequence in `IMPLEMENTATION_PLAN.md` §4.5.
this is the app's first frontend→rust invoke surface, so the suite's
job is twofold: the usual pure-logic coverage, plus pinning the
security boundary (the overlay must stay receive-only).

- **type**: unit (pure fns, tdd) + temp-dir integration (write paths)
  + vitest (form) — no new frameworks, no new dependencies
- **pure, tdd'd first** (`settings.rs`):
  - `validate` — every rule's accept/reject boundary (`port` 1024
    floor, `default_ttl` 1..=3600, `max_queued_per_tier` 1..=1000,
    `espn_poll_secs` 5..=3600, league entries non-empty/no-whitespace,
    empty league list rejected only when `espn_enabled = true`)
  - `mask` — long value → `set (…last4)`, short value → `set` (no
    partial leak), boundary length
  - config serialize→parse round-trip — a non-default `Config`
    through `toml::to_string_pretty` then `Config::parse` compares
    equal; pins the new `Serialize` derive against field drift (same
    spirit as §4.10's `SlotState` snapshot test)
  - secrets merge — setting the openrouter key preserves an existing
    telegram table and vice versa; a malformed existing file yields
    an error, never a clobber; `SecretField` covers exactly the three
    allowed fields
- **temp-dir integration** (never `$HOME` — same rule as §4.9's
  secrets-loader tests):
  - atomic config write: result parses via `Config::parse`, the temp
    file is gone after rename, a missing parent dir is created
  - secrets write: resulting file is mode `0600` and loads through
    the existing `load_secrets`
- **2026-07-17 review-round additions**:
  - malformed secrets containing a sentinel never echo that material
    through either the settings-facing error or the connector error's
    `Display`
  - unknown top-level tables and unknown fields inside known secret
    tables survive a write
  - leading/trailing clipboard whitespace is trimmed before secret
    validation and storage; interior whitespace remains invalid
  - a stale permissive fixed-name temp file is never reused or written
    into
  - submitted `detect_path` is replaced with the booted value before a
    config save
  - rss feed validation requires a fully parsed http(s) url with a host,
    not just a matching prefix
  - `ensure_settings_window` accepts the `settings` label and rejects
    `main`, using `tauri::test::mock_app()` + `WebviewWindowBuilder`
- **frontend (vitest, small)**: form renders values from a mocked
  `get_config`; a mocked `save_config_and_relaunch` rejection renders
  the error list. `@tauri-apps/api/core`'s `invoke` is mocked — no
  webview in ci. overlay tests untouched.
- **security boundary, pinned two ways**:
  - automated: `capabilities/default.json` unchanged in the diff
    (review-level check, called out in the v5 exit criteria)
  - manual, once: `invoke("get_config")` from the *main* window's
    devtools console is denied — verifies the `build.rs`
    `AppManifest::commands` opt-in + per-window capability actually
    gate (v4 §4.4's "does the gate gate" discipline)
- **untested by design** (extends §5.1's list): lazy settings-window
  creation and tray-item wiring remain thin native glue, but window
  construction is now partially covered by the label-gate test's mock
  `WebviewWindowBuilder`; `app.restart()` remains untested (kills the
  process — nothing to assert from inside it); the openrouter key's
  *use* remains untested because no consumer exists until the first ai
  feature

### 4.12 rss news poller + status-rail news cards (v5 news — landed 2026-07-17)

code-level detail lives with the feature, not a separate spec; build
sequence in `IMPLEMENTATION_PLAN.md` §4.6. `rss_feeds`/`rss_enabled`/
`rss_poll_secs`/`rss_ttl_secs`/`rss_max_per_poll` validation rules live
in `settings.rs` and are already covered by §4.11 (folded into that
suite's count the same day, not duplicated here) — this section is the
poller and the frontend render path only.

- **type**: unit (pure fns, tdd) — same shape as §4.7's espn poller:
  the fetch loop stays thin and untested (§5.1), parsing/dedup/diff
  logic are pure functions tested directly against fixtures, including
  real-shaped ndtv captures. no live rss fetch in any test.
- **example cases** (`rss_poller.rs`, see §0 for the current count):
  - `SeenStore`: bounds enforcement (1k keys, oldest evicted first),
    7-day eviction, guid dedup, canonical-link fallback when a guid is
    absent, cross-feed duplicate guard (the same story from two feeds
    only surfaces once)
  - sanitize: strips markup/entities from real-shaped ndtv fixture
    items without mangling plain text
  - diff/baseline: first poll per feed is silent (no restart flood,
    same rule as §4.7's espn baseline); subsequent polls emit only
    unseen items in feed order; `rss_max_per_poll` caps a single poll's
    emissions (replay bug-guard) without dropping the excess from
    `SeenStore` (they're still marked seen, not re-offered next poll)
  - metadata derivation: category from entry `<category>` tags via the
    keyword table, falling back to the feed's configured default;
    source from `[[rss_feeds]]` config, falling back to the parsed
    feed title
  - malformed/empty feed body → no crash, no event emitted (same
    failure-mode contract as §4.7)
- **frontend (vitest, part of the frontend total — see §0 for current
  counts — presentation tables and `StatusRailCard` cover the news
  branch)**: masthead render
  (`{source} · Wire`), 2-line clamped headline, category + age pill
  content, `stampFor`/`categoryClass`/`ageLabel`/`publishedLabel`
  lookup-table cases including unknown-category fallback, null-metadata
  fallback (non-news `SlotState` items render with no source/category/
  publishedAtMs and don't crash the news branch), the news manifest's
  3-column layout
- **untested by design** (extends §5.1): the category-hued gradient
  shader's visual output and reduced-motion behaviour — same
  eyeball-only reasoning as §4.6; `fetch_feed`'s decision surface (304
  short-circuit, validator-persist-only-after-success ordering, the
  content-length/streamed size cap) is wiremock-tested (`rss_poller.rs`'s
  `fetch_feed_tests`), so only the spawn loop itself stays thin-by-design,
  same as §4.7's espn poller
- **manual, not yet run**: `rss_enabled = true` against the live ndtv
  feed — first-poll silence, masthead/shader/pill rendering, the news
  manifest hotkey, a `High`-priority push preempting a queued headline;
  tracked in `IMPLEMENTATION_PLAN.md` §4.6.1 and §6

---

## 5. what stays manual, and why (maps to `IMPLEMENTATION_PLAN.md` §6)

these aren't gaps to close later — they're inherent to what's being
tested:

- **notch-cutout anchoring on the actual macbook, hud rendering on the
  actual mac mini** — needs two specific physical machines; no ci
  runner reproduces this
- **animation look/feel** — subjective, visual, cheap to eyeball,
  expensive to automate for a one-person tool
- **cmux "agent needs input" end-to-end** — needs a real cmux session
  raising a real claude code permission prompt; can't be synthesized
  without also faking cmux itself, which would test the fake, not the
  integration

### 5.1 modules with no test module, and why (recorded 2026-07-16)

silence is ambiguous — this list makes "untested by design" explicit
per module, so a missing `#[cfg(test)]` block is never mistaken for an
oversight:

- **`lib.rs`** — partially tested: the pure hotkey handlers
  (`toggle_manual_expand`, `dismiss_current`, `toggle_pause`) have their
  own suite, §4.10, and the eval-splice escaping is extracted as the
  tested pure `escape_for_eval_splice` (all `webview.eval` json splices
  route through it). the rest — window, tray construction, heartbeat
  spawn, page-load gate — stays untested by design: thin orchestration
  of native apis; §3's "don't test thin wrappers" rule. the logic it
  calls (queue, emit rule) is tested where it lives.
- **`logging.rs`** — subscriber init + double fmt-layer glue only; the
  size-rotation engine (threshold, cascade, reset) grew real decision
  logic and so came off this list per the rule below — it has its own
  temp-dir test module now (§0).
- **`login_item.rs`** — `SMAppService` registration shim. only
  observable against a real macos session; manual checklist territory.
- **`error.rs`** — `thiserror` declarations only; the variants are
  asserted where they're produced (queue, event, http tests).
- **`poller.rs` fetch loop** (`fetch_league` + the spawn loop) —
  deliberately thin per v2 spec §3; everything downstream of "here is
  a response body string" (parse, diff, backoff decisions) is the
  tested surface.
- **`presentation.rs` subprocess spawn** — the `std::process::Command`
  call to `notchtap-detect`; §4.4 already covers why only the parse +
  fallback paths downstream of it are tested.

if a module on this list grows a real decision (a branch someone could
get wrong), it comes off the list and gets a test module in the same
change.

---

## 6. no global coverage percentage gate

resist tracking one repo-wide coverage number — it rewards testing
trivial getters and framework glue, and this repo has almost none of
that to begin with. instead, each phase's exit criteria
(`IMPLEMENTATION_PLAN.md` §6) should require: every example case listed
in §4 above for that phase's components has a passing test before the
phase is called done.

## 7. running the suite

- `cargo test` (from `src-tauri/`) — all rust unit + integration tests,
  including the doc-tests on the public `queue`/`event` apis
- `npx vitest run` (from repo root) — all frontend unit tests
- both should run clean before any phase in `IMPLEMENTATION_PLAN.md` is
  marked complete — this is now also reflected in that doc's §6
- ci (v4) runs the same two commands plus `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `npx tsc --noEmit`, `npx vite build`,
  and a `swiftc` compile check — nothing ci-only

---

## 8. planned / deliberately-not-yet (as of 2026-07-16)

tracked here so "not done" is a decision with a trigger, not a gap:

- **deep testing (§9)** — the one genuine rigor upgrade available; the
  full implementation-ready work order is §9 below. reviewed and
  **parked 2026-07-16**: the example-based suite covers every listed
  transition and the extra rigor wasn't judged worth the work yet.
  **trigger to un-park**: first queue regression the example cases
  miss, the next queue-semantics change (e.g. a priority lane), or the
  user asking for it. when picked up, §9 is the work order — don't
  re-plan. **trigger fired 2026-07-18**: the queue's semantics changed
  repeatedly since the park (v3.6 three priority tiers, v6
  rotation-order tie-break, dismiss/skip, per-item expanded semantics),
  and `8b216ee` already showed the example suite letting one gap
  through. §9.1 (queue proptest) and §9.2 (http burst/boundary) are now
  executed and landed; §9.3/§9.4 stay parked, scoped out of this pass.
- ~~**v3 connector tests**~~ — landed with v3 (telegram, not
  twilio/whatsapp — that demotion is in `IMPLEMENTATION_PLAN.md` §3);
  see §4.9. the no-live-calls rule held: wiremock only.
- **`test-cli.sh` for the `notchtap` script** — v2 spec §8, add only if
  the script grows beyond flag-parsing + one curl.
- **visual regression for animations** — §4.6's trigger, re-evaluated
  and declined 2026-07-16; see there.

---

## 9. deep testing work order — un-parked 2026-07-18, §9.1/§9.2 landed

**status: un-parked 2026-07-18.** the trigger in §8 fired (queue
semantics changed repeatedly since the 2026-07-16 park — v3.6 three
priority tiers, v6 rotation-order tie-break, dismiss/skip, per-item
expanded semantics). §9.1 (queue proptest) and §9.2 (http burst/
boundary) are now implemented and green — see their landed-markers
below. §9.3 (poller fuzz) and §9.4 (frontend timing fuzz) stay parked,
scoped out of this pass; §9.5/§9.6 are unchanged reference material.

like `V3_6_TECHNICAL_SPEC.md`, this section is not locked: adjust freely
as implementation surfaces friction; fold any *decision* changes back
into §1–§8.

**retargeted 2026-07-18** (was flagged stale 2026-07-17 against the
pre-v3.6 `NotificationQueue`/`max_concurrent` shape): §9.1 below is
rewritten against today's `SingleSlotQueue` — three priority-tier
waiting lanes, `tick`/rotation naming, `Skip` as a distinct op from
`Dismiss`, and topic supersession's extension cap. this retarget is
itself the record of that pass; no separate stale-note remains.

### 9.0 what "deep" means here, and what it doesn't

the existing suite is example-based: every listed transition has a
hand-written case. deep testing adds *machine-generated adversaries* —
random operation interleavings and malformed inputs — checked against
*invariants* (properties that must hold after every step, no matter the
sequence). it finds the interleavings nobody thought to write.

explicitly **not** in this section (unchanged from §5):

- notch geometry, hud placement, animation look — physical/manual
- live espn or twilio calls — fixtures and mocks only, in ci and locally
- visual regression — §4.6's trigger was re-evaluated and declined
- coverage percentage gates — still banned (§6's reasoning stands)

### 9.1 queue property tests (rust, `proptest`) — the core

**landed 2026-07-18.** `src-tauri/src/queue.rs`, new
`#[cfg(test)] mod proptest_queue` as a sibling of the existing
example-based `mod tests` (same file, so it can reuse the private
field access to `visible`/`waiting`/`expanded` the same way
`mod tests` already does; since plan 037 the queue is clock-agnostic —
`enqueue(event, now)` takes the simulated clock at the public
interface, which is what the harness's `apply_enqueue` drives).

**dependency**: `proptest = "1"` under `[dev-dependencies]` in
`src-tauri/Cargo.toml`. dev-only — no shipped-binary impact.

**the operation model** — drive a `SingleSlotQueue` with a generated
script of operations against a simulated clock (a `now: Instant`
advanced only by `Tick`, never a real sleep):

```rust
enum Op {
    Enqueue { priority: Priority, rotation: RotKind, topic: Option<u8>, origin: SourceKind },
    Tick(u64),          // advance_secs in 0..=12, then queue.tick(now)
    Dismiss,             // dismiss_visible(now)
    Skip,                 // skip_visible(now)
    ToggleExpanded,
    Pause,
    Resume,
}
// RotKind::OneShot(1..=10) | RotKind::Recurring(1..=10)
```

this is the full state-mutating `pub fn` surface of `SingleSlotQueue`
with two pre-cleared exceptions: `enqueue_test` (a test-only variant of
`enqueue` used by `send_test_notification` — the production `/notify`
op model doesn't need a distinct variant for it) and
`slot_state_if_changed` (mutates `last_emitted`, but it's the *probe*
for invariant 7, not an op — called every step, never generated).
`with_rotation_order` is a per-case queue *parameter*, not a scripted
op: the harness leaves it unset (empty), so same-tier tie-breaks
degenerate to plain arrival-order FIFO — v6's rotation-order rank
logic already has its own dedicated example tests
(`rotation_order_breaks_same_tier_ties_ahead_of_arrival_order` et al.)
and re-deriving rank-based tie-breaking here would just re-encode that
logic rather than test against it independently.

generator shape: `vec(any_op(), 0..50)` operations, with
`max_queued_per_tier` itself generated per case, `1..=10`. small bounds
on purpose — proptest shrinks failures toward minimal scripts, and
small state spaces shrink better.

**the invariants (checked after every single op)**:

1. **at most one Visible item ever** — structurally guaranteed by
   `visible: Option<QueueItem>`; asserted as documentation, not a
   meaningful adversarial target on its own.
2. **per-tier waiting cap, checked ONLY immediately after an `Enqueue`
   that lands in `waiting`** — never after every op. three documented
   bypasses legally exceed `max_queued_per_tier` and would false-fail
   an always-on assertion: (a) the immediate-promote fast path (slot
   empty, all tiers empty, not paused — cap never evaluated); (b)
   Recurring requeue (`tick` rotation and `Skip` `push_back` with no
   cap check); (c) cross-tier Topic supersede (relocates an existing
   waiting item to a different tier's back with no cap check). the
   harness distinguishes "landed in waiting as a new item" from "merged
   into an existing item via supersede" by comparing total item count
   (visible + all waiting) before/after the call — a merge leaves the
   total unchanged, a genuine new item increases it by exactly one.
3. **no premature rotation**: a `Tick` whose `advance_secs` doesn't
   reach the visible item's `promoted_at + window + extension_secs`
   must leave that exact item visible, unchanged. early removal only
   ever happens via an explicit `Dismiss` or `Skip`.
4. **promotion picks the highest non-empty tier, minimum
   `rotation_order` rank within it, FIFO on a rank tie**: whenever
   `Tick`/`Dismiss`/`Skip` causes a new item to become visible, its id
   must equal the item of lowest `rotation_order` rank (ties broken by
   earliest arrival) in the highest-index non-empty waiting tier as it
   stood immediately before promotion — including, for `Tick`/`Skip`, a
   same-turn Recurring requeue landing back in its own tier before
   promotion is evaluated. `rotation_order` is now generated per case
   (empty, a partial subset, or a full permutation of the four
   `SourceKind` variants) rather than always left empty, and the
   predictor mirrors production `best_index_in_tier` exactly — highest
   tier, then minimum rank, then FIFO — instead of checking pure FIFO.
5. **pause gates promotion, not aging; nothing enqueued while paused is
   lost** (count conservation): while paused, `Tick`/`Dismiss`/`Skip`
   never promote (visible stays `None` after any rotation/removal).
   the harness tracks `enqueued_accepted` (incremented once per
   genuinely-new accepted item, per invariant 2's merge-vs-new
   distinction) against `rotated_out_dropped` (a `Tick` ages out a
   OneShot Visible item), `dismissed` (`Dismiss` had a Visible item —
   Recurring or OneShot, `dismiss_visible` always drops), and
   `skipped_oneshot_dropped` (`Skip` had a Visible *OneShot* item only —
   a skipped Recurring item requeues, it is not a drop). asserted every
   step: `enqueued_accepted == (visible?1:0) + total_waiting +
   rotated_out_dropped + dismissed + skipped_oneshot_dropped`.
6. **supersession never creates a second item** — covered by the same
   conservation equation as invariant 5 (a merge never increments
   `enqueued_accepted`). two *separate*, differently-scoped extension
   caps: (i) a visible Topic supersede tops up remaining time bounded
   by `MAX_EXTENSION_ON_SUPERSEDE_SECS` (`6`) — asserted every step,
   whenever visible is `Some`, `extension_secs <= 6`; (ii) a cross-tier
   waiting supersede is invariant 2(c)'s per-tier cap bypass and is
   *not* bounded here — noted, not asserted.
7. **`slot_state_if_changed` never returns two consecutive equal
   states** — called once per op (the harness's one and only probe
   site); a returned `Some` is compared against the last `Some` seen,
   never against the immediately-preceding call if that one was `None`.
8. **expanded resets on promotion**: immediately after any op promotes
   a new item to visible, `expanded == (priority == Priority::High)`.
9. **`next_deadline()` invariant**: whenever `Some`, it equals
   `promoted_at + Duration::from_secs(rotation_window(expanded) +
   extension_secs)` of the current visible item — asserted every step.

invariants 3, 4, 5, and 9 are the ones example-based tests can't
honestly claim — they quantify over *all* interleavings of
pause/resume/dismiss/skip/rotation windows, not just the hand-picked
sequences in `mod tests`.

**expected size and cost**: one proptest
(`#[test] fn queue_invariants_hold_under_any_op_script()`) at
`ProptestConfig::with_cases(256)`. runtime target: well under 1s; all
clock math is simulated (`now: Instant` advanced only by `Tick`).

**exit criteria** (all met at landing):

- the property passes 256 cases locally, run twice to shake out flaky
  generation
- the existing example-based queue tests stay untouched and green —
  the property *supplements* them, it does not replace them
- zero production-code changes to `queue.rs` logic — tests only

### 9.2 http layer — burst and boundary integration cases

**landed 2026-07-18.** `src-tauri/src/http.rs`, extending the existing
`mod tests`. no new dependencies (tower `oneshot`, as everywhere in
§4.3). retargeted from the pre-v3.6 `max_concurrent`/`max_queued`
framing to today's single-slot-plus-per-tier-cap model: only one item
is ever visible, so "burst accounting" means bursting one priority
tier's `waiting` up to and past its `max_queued_per_tier` cap, not
filling multiple concurrent visible slots.

- **burst accounting**: with `max_queued_per_tier = 5`, fire 8
  sequential same-tier posts at the router. the first fast-path-
  promotes to visible; assert exactly 6 total succeed (200) — 1 visible
  + 5 waiting — and 2 are rejected (429), matching invariant 2.
- **paused burst accounting**: same tier cap, paused from the start (no
  fast path — every push lands in `waiting`): assert exactly 5× 202
  then 3× 429, nothing visible.
- **boundary body sizes**: a body whose exact serialized length is the
  64 KiB `DefaultBodyLimit` (`65536` bytes) is accepted; one byte over
  is rejected with 413. (the limit existed already but was only tested
  with a grossly oversized body — this pins the exact boundary.)
- **ttl is wire-immutable, not clamped**: v1 spec §3 documents that
  `/notify` never accepts a client-supplied ttl field at all — `NotifyRequest`
  has no `ttlSecs` field, full stop; `ttl_secs` is always constructed
  server-side from `Config::default_ttl` / `Config::cmux_ttl_secs`. this
  is documented behaviour, not a spec gap, so there is no "clamp 0 /
  clamp absurd value" case to write. what *is* worth pinning: an extra,
  unrecognized `ttlSecs` field on the wire is silently ignored (no
  `deny_unknown_fields` on `NotifyRequest`) and the configured default
  still applies — asserted via `next_deadline()` landing at
  `now + default_ttl`, not at the attempted wire value.

true concurrency (simultaneous in-flight requests) is deliberately not
simulated: the queue sits behind a mutex, so interleaving reduces to
ordering — which §9.1 already covers exhaustively.

**exit criteria** (all met at landing): each case above written and
green; the existing http tests untouched — no case above duplicated one
already present (the pre-existing `full_queue_returns_429`,
`full_queue_returns_429_while_paused`, and `oversized_body_returns_413`
cover the *shape* of rejection with a minimal 1-2 request setup; the
cases here specifically pin the multi-request boundary count and the
exact byte-length edge, which those don't).

### 9.3 poller robustness — parse fuzz (rust, `proptest`)

**file**: `src-tauri/src/poller.rs`, new `#[cfg(test)] mod proptests`.
reuses the §9.1 dependency.

- **`parse_scoreboard` never panics**: feed arbitrary strings
  (including non-utf8-boundary junk via `\PC*` and truncated prefixes of
  a real fixture) — the result is `Ok` or `Err`, never a panic. this
  hardens the "undocumented public endpoint changes shape without
  notice" failure mode (`IMPLEMENTATION_PLAN.md` §2.1) beyond the
  hand-picked malformed fixtures.
- **fixture-mutation fuzz**: take the committed real fixture, apply
  generated structural mutations (delete a random key, null a random
  value, retype a number to a string), and assert `parse_scoreboard` +
  `diff_scoreboard` combined never panic and never emit an event with an
  empty title. this catches the "half-changed payload shape" case that
  pure junk strings don't reach.

**exit criteria**: both properties green at 256 cases; no live network
anywhere (unchanged rule).

### 9.4 frontend deep timing tests (vitest + `fast-check`)

**file**: `src/useVisibleNotifications.test.tsx` (or a sibling
`useVisibleNotifications.property.test.tsx` if the file gets long).

**dependency**: `fast-check` as a devDependency (the vitest-ecosystem
proptest equivalent; integrates with fake timers cleanly).

- **no immortal cards**: for a generated sequence of
  (emit, advance-fake-clock) steps — including advances that skip past
  deadlines in one jump, simulating sleep/timer-throttling — after
  advancing past every emitted item's deadline **plus one sweep
  interval**, visible state is empty. this generalizes the hand-written
  sweep cases (v2 spec §6.1) to arbitrary schedules.
- **no duplicate renders**: duplicate ids across the generated emits
  never yield two simultaneous cards with the same id.
- **phase monotonicity**: a card's phase only ever moves
  enter → hold → exit (never backwards) across any advance schedule.

scope guard: the hook remains rendered through the real
`renderHook`/fake-timer harness the existing tests use — no new render
infrastructure. if fast-check + fake timers fight each other in
practice, fall back to a seeded hand-rolled fuzz loop (a plain test
generating 100 random schedules from a fixed seed) — the invariants
matter, the framework doesn't.

**exit criteria**: three properties green; the existing frontend tests
untouched; `npx vitest run` stays under ~5s.

### 9.5 deliberately still out (even when §9 is un-parked)

- **proptest on `diff_scoreboard` semantics** (beyond §9.3's
  never-panic): the delta logic's *meaning* (which transitions emit
  what) is spec-by-example; the fixture cases are the spec. a property
  here would just re-encode the implementation. skip.
- **doc-tests beyond queue/event**: the other pub-worthy surfaces
  (`http::router`, `poller::parse_scoreboard`) are internal; making
  them pub just for doc-tests inverts the §2 doc-test rule (few,
  lifecycle-shaped, on genuinely public api). skip.
- **mutation testing (`cargo-mutants`)**: interesting, but the §9.1 /
  v4-style "break it once, watch it fail" manual check buys most of the
  value at zero tooling cost for a one-person repo. note as a future
  idea only.

### 9.6 build order and review gates

1. §9.1 queue proptest (highest value, pure, no new test infra beyond
   the dep) → review the shrunk-failure ergonomics before proceeding
2. §9.2 http burst/boundary cases (no deps, quick)
3. §9.3 poller fuzz (reuses proptest)
4. §9.4 frontend fast-check (new dep on the web side — last, so a
   decision to drop it doesn't block the rust work)

each step lands with `cargo fmt --check`, `cargo clippy -- -D warnings`,
`cargo test`, `npx tsc --noEmit`, `npx vitest run`, `npx vite build`
all green — same gates as ci, no exceptions. as each section lands,
update §0's status table and the per-component pointers in §4.
