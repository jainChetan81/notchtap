# notchtap — v2 technical spec (v0 draft)

concrete contract for v2 (`IMPLEMENTATION_PLAN.md` §2). same status
rules as the v1 spec: `ARCHITECTURE.md` (§16 for v2) holds the locked
decisions; everything here is adjustable the moment implementation
disagrees with it. read `docs/V1_TECHNICAL_SPEC.md` first — v2 is a
delta on top of it, and nothing there is re-stated here.

v2 decisions locked 2026-07-16 (`ARCHITECTURE.md` §16): leagues =
premier league / champions league / la liga; trigger scope = every
scoreboard delta espn reports; animation = css keyframes, no new
dependency; v2 absorbs the three v1 hardening fixes; notch-precise
positioning stays deferred.

---

## 0. scope

- espn scoreboard poller → typed match events into the existing queue
- per-event-type animation via a config table (css keyframes)
- three hardening fixes carried over from the v1 consensus review
- **already done**: the cmux relay (`IMPLEMENTATION_PLAN.md` §2.2) was
  live-verified on the mac mini on 2026-07-16 with a real claude code
  "needs input" alert — no v2 work remains on it except configuring
  the same cmux setting on the macbook

not in v2: posture module, outbound connectors (v3), notch-precise
positioning, queue-level dedup (`ARCHITECTURE.md` §13 stands — the
poller emits deltas by construction, so it never needs the queue to
dedupe for it).

---

## 1. build order (mirrors `IMPLEMENTATION_PLAN.md` §2.0–2.3)

1. **hardening fixes (§6)** — small, independent, land first
2. **event model + animation table (§2, §5)** — extends the wire
   contract and frontend. *automated* coverage needs no espn: vitest
   synthesizes `notification-promoted` events with any `eventType`,
   and rust serde tests cover the new variants. but `/notify` stays
   generic-only (§2), so the *visual* eyeball of the non-generic
   animations waits for the poller's first real events — §7's manual
   row. (a dev-only typed-injection endpoint was considered and
   rejected as speculative for a personal tool; revisit in §8 only if
   the wait actually hurts.)
3. **espn poller (§3, §4)** — observe the real payload shape first,
   then fixtures, then tests (`TESTING_STRATEGY.md` §4.7 explicitly
   orders it this way)

---

## 2. event model changes

```rust
// event.rs — EventType grows real variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Generic,
    ScoreUpdate, // a goal: the score number changed
    MatchState,  // kickoff, half-time, full-time
}
```

- `Priority` stays single-variant; nothing in v2 needs priority
  ordering yet.
- `TESTING_STRATEGY.md` §4.2's "unknown type" case becomes genuinely
  exercisable (serde now has real variants to mismatch against).
- card/situation events ("everything espn reports"): the scoreboard
  endpoint's coverage of cards is inconsistent across leagues.
  **rule**: emit them as `MatchState` with descriptive title/body if
  the observed payload carries them; add a dedicated `CardEvent`
  variant only if the real shape justifies it. do not design the
  variant before seeing the data.

**wire payload** (v1 spec §7 `NotificationPayload`) gains one field:

```rust
pub event_type: EventType, // serializes as "eventType": "score_update" etc.
```

```ts
type NotificationPayload = {
  id: string;
  title: string;
  body: string;
  ttlSecs: number;
  eventType: "generic" | "score_update" | "match_state";
};
```

the `/notify` http request schema is **unchanged** — external pushes
are always `Generic`. only the poller constructs the new types,
internally.

---

## 3. espn poller

new module `src-tauri/src/poller.rs`, spawned from `lib.rs` at startup
(after config load; skipped entirely when `espn_enabled = false`).

- **endpoint**: `GET https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard`
  per configured league, via `reqwest` (new dependency, v2-only as
  already listed in v1 spec §13).
- **cadence**: every `espn_poll_secs` (default 30s), leagues polled
  sequentially within one tick (3 leagues × 1 request — no need to
  parallelize or stagger at this scale).
- **delta detection**: keep an in-memory snapshot per espn event id
  (`score home/away`, `status`). each poll, compare against the
  snapshot and emit one typed `Event` per changed fact:
  - score changed → `ScoreUpdate` (title: `"LIV 2–1 MUN"`, body: the
    scorer/clock text if the payload has it, else `"goal"`)
  - status changed (`pre→in`, `in→halftime`, `→final`) → `MatchState`
  - first sighting of a match does **not** emit — the baseline
    snapshot is silent, otherwise every restart floods with the
    current state of every live match.
- **failure mode** (locked, `IMPLEMENTATION_PLAN.md` §2.1): malformed
  json / timeout / 5xx → `tracing::warn`, skip that league's cycle,
  exponential backoff **per league** (30s → 60s → 120s, cap 300s,
  reset on that league's first success) — one flapping league must
  never stop the others from updating. the poller can never panic the
  app or block the queue.
- **snapshot eviction**: when a match reports final or disappears from
  its league's feed, drop its snapshot entry at the end of that poll.
  the snapshot never outlives the scoreboard it mirrors, so memory is
  bounded by however many matches espn currently lists — no unbounded
  growth across weeks of uptime.
- **ordering**: multiple deltas in one poll emit in feed order (league
  order, then match order within the feed) — deterministic; fifo
  handles the rest downstream.
- **separation for testability** (same pattern as v1's
  `presentation_mode`): the pure function
  `fn diff_scoreboard(prev: &Snapshot, fetched: &Scoreboard, ttl_secs: u64) -> (Vec<Event>, Snapshot)`
  holds all delta logic and is unit-tested against fixtures. returning
  the next snapshot (rather than mutating in place) is what makes
  eviction fall out of construction. the fetch loop around it stays
  thin and untested.

---

## 4. config additions (`~/.config/notchtap/config.toml`)

```toml
espn_enabled = true
espn_leagues = ["eng.1", "uefa.champions", "esp.1"]
espn_poll_secs = 30
```

all three `serde(default)` like the v1 fields — a v1 config file keeps
working untouched (espn defaults to on with the three locked leagues).

note stated plainly: `espn_enabled = true` by default means a machine
upgrading from v1 starts making background network calls to espn on
next launch with no config change. intentional (`ARCHITECTURE.md`
§16 — the owner picked the leagues), recorded here so it's never a
surprise.

---

## 5. animation table (frontend)

- `styles.css` defines one animation set per event type; the
  notification div's class becomes
  `notification ${eventType} ${phase}` (today: `notification ${phase}`).
- mapping is **css-only**: `.score_update.enter { … bounce … }`,
  `.match_state.enter { … slide … }`, `.generic.enter` keeps the v1
  template. unknown/missing `eventType` falls back to `generic`
  (frontend defaults the field if absent, so a v1-era rust core still
  renders).
- no config file on the frontend side — the "config table" locked in
  `ARCHITECTURE.md` §4 *is* the stylesheet keyed by type; adding a v3
  type means adding css, not code. (if per-user remapping is ever
  wanted, that's a v3 settings concern.)

---

## 6. hardening fixes (from `docs/review-logs/2026-07-16-v1-implementation-review.md`)

1. **frontend wall-clock deadline recheck** — `useVisibleNotifications`
   stores an absolute `deadline = Date.now() + enter + ttl + exit` per
   item and runs a 1s sweep interval that force-removes anything past
   its deadline. fixes stale cards after system sleep or webview timer
   throttling; the existing setTimeout chain stays as the happy path.
2. **`app_handle.exit(1)` instead of `std::process::exit(1)`** in the
   server task's bind-failure branch (`lib.rs`), so tauri cleanup and
   the log flush guard run.
3. **runtime-thread guard before `blocking_lock`** in the tray handler:
   `debug_assert!(tokio::runtime::Handle::try_current().is_err(), …)`
   — makes the "menu events are off-runtime" assumption explicit and
   loud in dev if a refactor ever moves them. (dev builds only — it
   compiles out of release. accepted: the guard exists to catch
   refactors during development, not to protect production.)

---

## 7. testing crosswalk

| this doc | `TESTING_STRATEGY.md` |
|---|---|
| §3 `diff_scoreboard` + poller failure modes | §4.7 (fixtures via `wiremock` for the fetch layer; pure-function tests need no mocking at all) |
| §2 new `EventType` variants / serde | §4.2 (unknown-type case now real) |
| §5 animation table | §4.6 (stays manual — eyeball each type once) |
| §6.1 deadline sweep | §4.5's wall-clock deadline case (added 2026-07-16): item past deadline is removed even if its timers never fired |
| cmux relay | §4.8 — reduced by the flags-only cli: the "env-var parsing" is the shell's, not ours. the fold/empty-subtitle logic is the script's and is covered by manual verification; see §8 |

fixture rule restated from §4.7: **never call the live espn endpoint
from a test.** capture real responses once (a `docs/fixtures/` or
`src-tauri/tests/fixtures/` folder), assert against those.

---

## 8. what's genuinely open

- whether the cli shell script gets automated tests (a tiny
  `test-cli.sh` asserting fold/port behaviour) or stays
  manually-verified — currently manual; revisit if the script grows.

resolved 2026-07-16 by capturing real payloads
(`src-tauri/tests/fixtures/scoreboard-*.json`, five leagues incl. one
finished ucl match):

- **scorer/clock fields**: `competitions[0].details[]` carries
  `scoringPlay`/`redCard` booleans, `type.text` ("Goal", "Yellow
  Card", "Penalty - Scored"), `clock.displayValue` ("6'"), and
  `athletesInvolved[].shortName` ("K. Havertz"). half-time is
  `status.type.name == "STATUS_HALFTIME"` while `state` stays "in".
  scores arrive as strings.
- **`CardEvent` does not earn a variant**: cards emit as `MatchState`
  with the card detail as body ("Yellow Card — B. Saka 54'"). yellows
  included — "everything espn reports" (`ARCHITECTURE.md` §16) was
  chosen with eyes open; a busy match means several card
  notifications, and narrowing to red-only is a two-line filter if it
  proves too chatty in practice.
