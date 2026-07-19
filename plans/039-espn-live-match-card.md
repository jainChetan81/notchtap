# Plan 039: ESPN live-match scoreboard card (opt-in, single-match)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report — do not
> improvise. When done, update this plan's status row in
> `plans/README.md`.
>
> **Dependency gate (hard — STOP if unmet)**:
> 1. `plans/README.md` row for **038** reads DONE. (037 already does —
>    merged to master at `6b53c32`; this review-plan pass re-grounded
>    every Engine-facing detail below against that landed code, see
>    "Re-grounded against the Engine" below. 038 is what's still
>    missing: without it, a `Recurring` live card — this plan's first
>    real producer of one — hits the pre-existing `batch_done` bug the
>    design spike surfaced, pinning the 033 queue-slider near "complete"
>    while the match is still cycling.)
> 2. `git status` clean for `src-tauri/src/poller.rs`,
>    `src-tauri/src/config.rs`, `src-tauri/src/settings.rs`,
>    `src-tauri/src/queue.rs`, `src/settings/SettingsApp.tsx` — STOP if
>    any is dirty.
>
> **Drift check (run second)**: `git diff --stat 882cdb6..HEAD --
> src-tauri/src/poller.rs src-tauri/src/config.rs
> src-tauri/src/settings.rs src-tauri/src/queue.rs src-tauri/src/engine.rs
> src/settings/SettingsApp.tsx src/settings/SettingsApp.test.tsx`.
> `882cdb6` is this review-plan pass's baseline — every excerpt, line
> number, and function signature below was re-verified against live
> code at that commit. A nonempty diff means one of these files moved
> since; re-open it and compare against this plan's excerpts before
> starting Step 1.

## Status

- **BLOCKED — gated on 038.** 037 landed (`6b53c32`) and this plan's
  Engine-facing risk is now resolved by direct evidence (see below), not
  just "the machinery should work." The only remaining blocker is 038.
- **Priority**: P2
- **Effort**: M
- **Risk**: MEDIUM — first production user of the queue's
  Topic/supersession/`Recurring` machinery (built + tested, zero
  producers today, and now independently confirmed to work correctly
  through `Engine::accept` — see below). Opt-in default-off contains
  the blast radius.
- **Depends on**: **038 (hard, TODO)**. **037 (done — DONE at
  `6b53c32`)**: no further coordination needed. Soft: inherits the
  Topic-identity + rotation-kind from the 031 design doc verbatim.
- **Planned at**: commit `882cdb6`, 2026-07-19. Design spike:
  `docs/design/scoreboard-topic-card.md` (plan 031, reviewed APPROVE).
  **Review-plan pass (this one, 2026-07-19)**, run after 037 landed:
  re-grounded every "confirm against the Engine" placeholder in the
  original filing against the actual `engine.rs`/`poller.rs`/`config.rs`
  code, resolved the plan's one open STOP-condition risk with evidence
  (see below), corrected the Config-surface section (the struct lives
  in `config.rs`, not `settings.rs`), added the frontend TS interface +
  test-fixture sites the original Scope missed, and wrote concrete
  numbered Steps (the original filing had none — Design spine + Scope
  only). Still BLOCKED: 038 hasn't landed, so this plan is grounded and
  ready but not yet dispatchable.

## Re-grounded against the Engine (this review-plan pass)

The original filing's two open risks — "does the Engine's
`apply`/`accept` surface cleanly carry a `Recurring` Topic from a
producer" and "does connector fan-out still survive supersession" —
are now resolved by reading the landed code, not by re-derivation from
the design doc:

- **`Engine::accept` is Topic/Rotation-agnostic by construction**
  (`engine.rs:149-175`). It takes a fully-formed `Event` and passes it
  straight to `q.enqueue(event, now)` (`engine.rs:161`) — nothing in
  `accept` inspects or special-cases `event.topic`/`event.rotation`.
  All Topic-supersession and `Recurring`-requeue logic lives entirely
  in `queue.rs` (`supersede_if_topic_matches`, `queue.rs:183-211`;
  `rotate_out_if_elapsed`/`skip_visible`'s `Recurring` requeue arms,
  `queue.rs:247-260`/`389-399`) — all pre-existing, unit- and
  proptest-covered, and untouched by plan 037. A producer that builds
  an `Event` with `topic: Some(...)` and
  `rotation: RotationSpec::Recurring { .. }` and calls
  `engine.accept(event, false)` gets the exact same queue behavior the
  design doc describes. **No Engine change is needed for this plan.**
- **Connector fan-out still survives supersession under `accept`**
  (confirmed, not assumed): `accept` clones the event into `to_offer`
  **before** calling `q.enqueue` (`engine.rs:154-161`), and the offer
  loop (`engine.rs:169-173`) runs unconditionally after enqueue
  succeeds — regardless of whether `enqueue` merged the event into an
  already-visible item via topic-supersede or promoted it as new. This
  is the same property the design doc's §5 relied on for the old
  `enqueue_and_fan_out` (deleted by 037); `accept` preserves it via a
  different code path. **The design doc's own `enqueue_and_fan_out`
  references (`docs/design/scoreboard-topic-card.md:359,369,503`) are
  now stale** — they describe pre-037 code. Don't follow those excerpts
  literally; the property they establish still holds, just via
  `accept`, as shown above.
- **Full-time correctly retires a cycling card without any special
  teardown**: `apply_fresh_content` (`queue.rs:519-524`), which
  topic-supersede uses to merge a fresh event into the visible one, ALSO
  copies `existing.rotation = fresh.rotation`. So when the full-time
  `MatchState` (built `OneShot`, same Topic) supersedes the visible
  `Recurring` match card, the visible item's rotation flips to `OneShot`
  in place — it then rotates out via the ordinary non-Recurring path on
  its next `tick()`, exactly like any other one-shot card. This also
  means 038's dependency is specifically about the *cycling* phase
  (kickoff→goal→goal, all `Recurring`, each natural rotation-out
  wrongly counting as "done" without the fix) — the full-time exit is
  unaffected by 038 either way, since the item is no longer `Recurring`
  by the time it rotates out.

## Governing design

`docs/design/scoreboard-topic-card.md` (plan 031 spike, reviewed
APPROVE). This plan is the *build* of that doc; it does not re-decide
anything §§1–7/9 settled. The §10 maintainer decisions (2026-07-19) that
shape scope:

1. **Build it, opt-in.** New config flag `espn_live_card`, default
   `false`. Today's burst-of-one-shot-cards stays the default; the flag
   flips one live match into a single updating card. Zero behavior change
   when off.
2. **Defer multi-match.** Scope is single-match correctness. Multiple
   concurrent live matches each get their own `Recurring` Topic and share
   the tier via rotation-order/FIFO exactly as today's multi-match burst
   already does — no special arbitration in this plan.
3. **Reuse `ttl_secs`.** The live card's `display_secs` = the existing
   `espn_ttl_secs` (default 8s). No new dwell knob; add `live_card_secs`
   only in a later plan if 8s proves wrong on hardware.
4. **Counter fix is separate (038), lands first** — not bundled here.

## Design spine

- **Topic identity**: `espn:{league}:{match_id}` — every event for one
  match shares it, so kickoff/goal/card/half-time/full-time supersede
  each other in the single Slot instead of queueing as separate items.
  `match_id` is `MatchView::id` (`poller.rs:157`, a `&str` — espn's own
  event id), already in scope inside `diff_scoreboard`'s per-match loop.
- **Rotation kind**: `RotationSpec::Recurring { display_secs: <ttl> }`
  for the live match while it is in play; the **full-time** `MatchState`
  is emitted `OneShot` on the **same Topic** — see "Re-grounded against
  the Engine" above for exactly why that retires the card with no
  bespoke teardown.
- **Producer**: `poller.rs`'s ESPN diff is the only changed producer.
  When `espn_live_card` is on, its per-match events carry
  `topic: Some("espn:{league}:{match_id}")` and the `Recurring`/`OneShot`
  rotation above, instead of today's `topic: None` one-shots.
- **Render**: reuse the existing card rendering — no new render path
  (the card shows the latest superseding `MatchState`).
- **Connector semantics**: verified above (this pass) — every delta
  still relays to connectors even though the overlay shows one
  consolidated card.

## Current state (grounded, this review-plan pass)

- `make_event` (`poller.rs:294-321`) is the single event-construction
  function `diff_scoreboard` calls for every kickoff/goal/card/
  half-time/full-time emission. It hardcodes `topic: None` (`:315`,
  with a comment saying no source constructs `Recurring` "in this
  pass" — that comment becomes stale once this plan lands and must be
  updated) and `rotation: RotationSpec::OneShot { ttl_secs }` (`:310`).
- `diff_scoreboard(prev, fetched, ttl_secs, league, priority)`
  (`poller.rs:346-352` for the signature) calls `make_event` at five
  call sites (score update, kickoff, half-time, full-time, card —
  `poller.rs:376,387,397,407,424`), each already inside a branch that
  knows the match's `v.id`/`league` and whether this is the full-time
  branch (`:406`, `final_now && old.state != "post"`).
- `spawn_espn_poller(engine: Engine, leagues, poll_secs, ttl_secs,
  priority)` (`poller.rs:553-559`, 5 args post-037) calls
  `diff_scoreboard` once per league per poll (`poller.rs:602`) and
  feeds every returned event through `engine.accept(event, false)`
  (`poller.rs:608-612`).
- The `lib.rs` call site (`lib.rs:317-325`) passes `espn_leagues`,
  `espn_poll_secs`, `espn_ttl_secs`, `espn_priority` — all cloned out of
  `config` earlier in `run()` (`lib.rs:126-127` for the ttl/priority
  pair). A new `espn_live_card` flag follows the identical pattern.
- `Config` (`src-tauri/src/config.rs:9`, NOT `settings.rs` — that file
  only holds the `#[tauri::command]`s and `validate()`) is a flat
  `#[serde(default)]` struct with a hand-written `impl Default for
  Config` (`config.rs:238+`, e.g. `espn_enabled: default_espn_enabled(),`
  at `:245`). `settings::validate` (`settings.rs:49-90`) only range-
  checks numeric/string fields — a plain bool needs no entry there.
  `get_config`/`get_default_config` (`settings.rs:582-599`) serialize
  the whole `Config` struct with no per-field code — adding the field
  to the struct is sufficient; **neither command needs to change**.
- The frontend hand-mirrors `Config` as a TypeScript interface
  (`src/settings/SettingsApp.tsx:33-51`) — NOT generated from Rust, so
  it needs the field added by hand or it silently type-checks against a
  stale shape. `SettingsApp.test.tsx` (`:12`, `:47`) hardcodes two
  fixture `Config` objects for its tests — both need the new field too,
  or they'll fail to satisfy the (now-wider) `Config` type once the
  interface changes.
- The ESPN toggle pattern to follow for the new one:
  `ToggleControl` for `espn_enabled` at `SettingsApp.tsx:646-653`
  (`id`, `name`, `help`, `label`, `checked={config.X}`,
  `onChange={(X) => patchConfig({ X })}`), inside the `"Score polling"`
  `SettingsGroup` (`:645`) — the new toggle belongs in the same group.

## Config surface

- `src-tauri/src/config.rs`: add `pub espn_live_card: bool` to the
  `Config` struct (near the other `espn_*` fields, after
  `espn_ttl_secs`) and `espn_live_card: false,` (a literal, or a
  `default_espn_live_card() -> bool { false }` fn matching the
  `default_espn_enabled` pattern if you want a doc-comment anchor —
  either is fine, this is a default-`false` bool) to the `impl Default`
  block. No `#[serde(default = "...")]` attribute needed — the struct's
  top-level `#[serde(default)]` already means a config.toml missing
  this key falls back to `Config::default()`'s value.
- `settings::validate` — no new rule (bool, nothing to range-check).
- `settings::get_config`/`get_default_config` — no change (whole-struct
  passthrough).
- Frontend: `src/settings/SettingsApp.tsx`'s `Config` interface gains
  `espn_live_card: boolean;`, and a new `ToggleControl` renders in the
  `"Score polling"` group per the exemplar above. `SettingsApp.test.tsx`'s
  two fixture objects (`:12`, `:47`) each gain `espn_live_card: false`
  (or `true` if the specific test wants it on — check what each fixture
  is exercising before picking).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cd src-tauri && cargo test --locked` | all pass; recount against `docs/TESTING_STRATEGY.md` §0 at your actual HEAD |
| Gates | `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Frontend gates | `npx biome ci . && npx tsc --noEmit && npx vite build` | exit 0 |
| Full local gate | `just test-all` | all green |

## Scope

**In scope**:
- `src-tauri/src/config.rs` — `espn_live_card: bool` field + `Default`.
- `src-tauri/src/poller.rs` — `make_event` gains a topic/rotation
  parameter (or a sibling constructor); `diff_scoreboard` threads the
  flag + builds the Topic string per match, and picks `Recurring` vs.
  the full-time `OneShot`; `spawn_espn_poller` gains one `bool`
  parameter (6 args total — still under clippy's 7-arg
  `too_many_arguments` threshold, no `#[allow(...)]` needed).
- `src-tauri/src/lib.rs` — thread `config.espn_live_card` through the
  `spawn_espn_poller` call site (`lib.rs:317-325`), same pattern as
  `espn_ttl_secs`/`espn_priority`.
- `src/settings/SettingsApp.tsx` — the `Config` TS interface + the new
  toggle.
- `src/settings/SettingsApp.test.tsx` — the two fixture objects.
- `docs/ARCHITECTURE.md` (record the first Topic/Recurring producer +
  the opt-in default-off decision), `docs/V3_6_TECHNICAL_SPEC.md`
  (config field), `docs/TESTING_STRATEGY.md` §0 counts.

**Out of scope**:
- `src-tauri/src/engine.rs` — confirmed above: needs zero changes.
- `src-tauri/src/queue.rs` — the Topic/supersession/`Recurring`
  machinery already exists and is already tested; this plan is its
  first caller, not a change to it. (038's `batch_done` fix is a
  separate plan, not part of this one, even though it's a prerequisite.)
- Multi-match arbitration (§10 decision 2 — deferred).
- A new `live_card_secs` dwell config (§10 decision 3 — deferred; reuse
  `espn_ttl_secs`).
- Any `#[tauri::command]` addition or `capabilities/*.json` change —
  this plan adds a passive config field only, served by the two
  existing whole-struct commands.

## Git workflow

- Branch: `advisor/039-espn-live-match-card` (or per operator dispatch).
- Commit style (from `git log`): lowercase `area: imperative summary`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: config flag

Add `espn_live_card: bool` to `Config` (`config.rs`) and its `Default`
impl, per "Config surface" above.

**Verify**: `cargo test --locked config::` → all pass (existing config
tests parse/roundtrip the whole struct — a new field with a working
`Default` should not break any of them; if one does, it's asserting an
exact field list somewhere — find it and add the new field rather than
special-casing it out).

### Step 2: poller producer changes

In `poller.rs`:
1. Give `make_event` (or a new sibling next to it — your call, whichever
   keeps `diff_scoreboard`'s five call sites readable) a way to receive
   `topic: Option<String>` and pick the rotation: `Recurring {
   display_secs: ttl_secs }` when `espn_live_card` is on and this isn't
   the full-time branch, `OneShot { ttl_secs }` otherwise (i.e. flag
   off, OR flag on but this is the full-time event).
2. Thread an `espn_live_card: bool` parameter through
   `diff_scoreboard`'s signature; build `format!("espn:{league}:{}",
   v.id)` once per match iteration and pass it to each of the five
   `make_event` call sites only when the flag is on (pass `None`
   unchanged when off — this is the "zero behavior change when off"
   done-criterion).
3. Add the same `espn_live_card: bool` parameter to
   `spawn_espn_poller`, passed through to each `diff_scoreboard` call.
4. Update the stale `poller.rs:311-314` comment ("no source constructs
   Recurring... topic is a Recurring-adjacent concern") — it's no
   longer true once this lands; rewrite it to describe the new
   conditional behavior.
5. `lib.rs:317-325`: pass `config.espn_live_card` (cloned out earlier,
   same pattern as `espn_ttl_secs` at `lib.rs:127`) into the
   `spawn_espn_poller` call.

**Verify**: `cargo build --locked` → exit 0 (signature changes ripple
through call sites; fix any the compiler finds before moving on).

### Step 3: rust tests

Follow the existing `diff_scoreboard`/`poller` test pattern
(`poller.rs:629+`, using the `USA`/`UCL` scoreboard fixtures already
loaded via `include_str!`). Add, per the design doc's Done criteria:
1. `espn_live_card=false` (the default): a match's full kickoff→goal→
   full-time sequence still emits `topic: None`/`OneShot` events,
   byte-identical to today — a regression pin, not a new behavior.
2. `espn_live_card=true`: the same sequence emits `topic: Some(...)`
   events sharing one Topic for every non-final event, with the
   full-time event `OneShot` on that same Topic.
3. `espn_live_card=true`, feeding the resulting events through a real
   `Engine`/`SingleSlotQueue` (follow `poller_accepted_events_fan_out_and_rejected_do_not`'s
   pattern, `poller.rs:659+`, which already builds a queue + fake
   connector): confirm the sequence collapses to one Slot occupant
   across the cycle, and that every event still reaches the connector
   channel (the fan-out-survives-supersession property from "Re-grounded
   against the Engine" above, exercised end-to-end).

**Verify**: `cargo test --locked poller::` → all pass, including the
three new cases; `cargo test --locked` (full) → all pass, totals match
the updated `docs/TESTING_STRATEGY.md` §0.

### Step 4: frontend

Add the interface field, fixture fields, and toggle per "Config
surface" above.

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run` → all pass;
`npx biome ci .` → exit 0.

### Step 5: docs + status

Update `docs/ARCHITECTURE.md` and `docs/V3_6_TECHNICAL_SPEC.md` per
Scope. Bump `docs/TESTING_STRATEGY.md` §0 counts for the new rust +
frontend tests. Flip this plan's `plans/README.md` row to DONE.

**Verify**: `cargo test --locked 2>&1 | grep "test result"` totals
match §0 exactly; `npx vitest run` count matches §0.

## Test plan

- New rust: the three cases in Step 3 (flag-off regression pin,
  flag-on Topic/rotation shape, flag-on end-to-end queue collapse +
  connector fan-out).
- New frontend: none required by this plan beyond keeping the existing
  `SettingsApp.test.tsx` suite green with the two updated fixtures — add
  a toggle-renders-and-patches test only if you want direct coverage of
  the new `ToggleControl`, following the existing `espn_enabled` toggle
  test as the pattern if one exists (`rg -n "espn-enabled" src/settings/SettingsApp.test.tsx`).
- Everything else stays green unchanged: the existing `diff_scoreboard`/
  poller suite (signature-only ripple from the new parameter), the full
  queue/engine suites (untouched — see "Out of scope").

## Done criteria

- [ ] `cd src-tauri && cargo test --locked` exits 0; totals match §0
- [ ] `cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` exit 0
- [ ] `npx vitest run && npx tsc --noEmit && npx biome ci .` all exit 0/pass
- [ ] Flag off: `rg -n "topic: None" src-tauri/src/poller.rs` still
      matches at every call site when `espn_live_card` is false (i.e.
      the flag-off path is provably unchanged, not just tested)
- [ ] `capabilities/default.json` / `capabilities/settings.json`:
      `git diff` → empty (no new invoke command, receive-only guarantee
      intact)
- [ ] `docs/TESTING_STRATEGY.md` §0 counts updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- 038 not DONE → STOP (dependency gate) — do not layer a `Recurring`
  producer onto the still-buggy counter.
- `queue.rs`/`engine.rs`/`poller.rs`/`config.rs`/`SettingsApp.tsx`
  dirty in the working tree → STOP and coordinate.
- The drift check is nonempty and an excerpt above no longer matches
  live code → reconcile before proceeding, don't guess.
- A new producer test reveals the Topic/`Recurring`/supersession
  machinery does NOT behave as "Re-grounded against the Engine" above
  claims (i.e. the evidence trail in this plan turns out to be wrong
  once exercised end-to-end with real ESPN-shaped events) → STOP and
  report exactly which claim broke; that's a signal this plan's
  grounding needs another pass, not something to improvise around.

## Maintenance notes

- This is the first production caller of `topic`/`RotationSpec::Recurring`
  outside `#[cfg(test)]` — if a future producer (a second live-status
  source, say) wants the same pattern, `poller.rs`'s Step 2 changes are
  the reference implementation, not `queue.rs` (which needed none).
- 038's `batch_done` fix note applies directly here: once this plan
  ships with the flag on, a live cycling card is the first real-world
  exercise of that fix — worth a manual eyeball on hardware alongside
  this plan's own manual checks.
- If `docs/design/scoreboard-topic-card.md` is read again later for
  another build, remember its `enqueue_and_fan_out` references
  (`:359,369,503`) describe pre-037 code — the property they establish
  still holds, now via `Engine::accept` (see "Re-grounded against the
  Engine" above), but the function name itself no longer exists.
