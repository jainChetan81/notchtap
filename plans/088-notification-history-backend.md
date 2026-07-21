# Plan 088: Notification history — JSONL storage layer + Engine write hook (backend only)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 172fd63..HEAD -- src-tauri/src/engine.rs src-tauri/src/config.rs src-tauri/src/lib.rs src-tauri/src/poller.rs src-tauri/src/http.rs src-tauri/src/event.rs src-tauri/src/logging.rs docs/TESTING_STRATEGY.md`
> Expected: empty. If any of these changed, compare the "Current state"
> excerpts against the live files before proceeding; on a mismatch, treat it
> as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: MED — first persistent storage of notification content in this
  app's history. The privacy posture change is deliberate and operator-
  decided (see "Why this matters"), but the default-off gate is
  load-bearing and must not be weakened.
- **Depends on**: none (plan 059's decision memo pinned the design; no code
  dependency)
- **Category**: direction
- **Planned at**: commit `68edd66`, 2026-07-21; **re-baselined to `172fd63`
  after attempt 1 (see "Attempt 1" below)**

## Attempt 1 — STOPPED correctly, plan was wrong (2026-07-21)

A first executor ran this plan at `68edd66`, completed Steps 1, 2, 4 and
most of 3, then **correctly hit the call-site STOP condition and reported
instead of guessing**. It was right and the plan was wrong:

- **`Engine::new` has 9 call sites, not the 7 this plan claimed.** The two
  the plan missed are `src-tauri/src/poller.rs:1420` and `:2130`, both
  test-only (`tauri::test::mock_app()`), and both already present at the
  original baseline — a planning miscount, not drift. `poller.rs` was
  therefore absent from the in-scope list, so the executor could not add the
  8th argument there and `cargo test` would not compile. **Fixed below**:
  the count now reads 9 and `poller.rs` is in scope for those two lines only.
- **Separately, `engine.rs` was refactored underneath that run** by plan 073
  (`48a6d66`, AmbientSlot generalization), which landed mid-execution. It
  changed `live`/`weather` from `Arc<StdMutex<Option<T>>>` to
  `AmbientSlot<T>` — the struct excerpt below is updated accordingly.
  `Engine::new`'s signature and `accept`'s body were verified unchanged by
  073, so every other excerpt in this plan still holds exactly.

Attempt 1's worktree was discarded rather than rebased (its `engine.rs`
edits predate 073). Its `history.rs` was reviewed and matched this plan's
Step 1 contract precisely — that part of the spec is known-good; you are
re-deriving it on a correct base, not fixing it.

## Why this matters

Today this app persists **nothing** about notification content. Once an item
leaves the single-slot queue (dismissed or rotated out), it is gone — an
operator who glances away and misses a card cannot recover what it said.
That gap was filed as plan 059 and put to the operator as a decision memo.

The operator's decisions (2026-07-21 session, recorded in
`plans/059-notification-history.md` under "DECIDED"):

1. **Persist all sources, including cmux/manual** — the privacy tradeoff was
   explicitly accepted (cmux payloads can carry commands and project paths).
2. **Retention: size-capped, plus a manual "Clear history" control** — no
   time-based pruning; follow `logging.rs`'s size-rotation precedent.
3. **Storage: append-only JSONL** at `~/.config/notchtap/history.jsonl` —
   no SQLite, no new dependency.
4. Two follow-up decisions made when this plan was filed: history records
   **one-shot events only** (see Step 3's rationale), and `history_enabled`
   **defaults to `false`**, matching every other opt-in surface in this repo.

This plan builds the **backend only** — the store, the config flag, and the
write hook — and ships dark: with the flag off (the default), behavior is
byte-identical to today. The Settings UI (a History section, plus the
`get_history`/`clear_history` invoke commands) is deliberately a separate
plan, mirroring how plan 083 (football backend) and plan 084 (football
display) were split in this same repo.

## Current state

Files you will modify, each with its role:

- `src-tauri/src/history.rs` — **new file**, the storage layer.
- `src-tauri/src/lib.rs` — declares modules; constructs the `Engine` at
  the one production call site (line ~261).
- `src-tauri/src/engine.rs` — holds `Engine`; `accept` is the one ingest
  path every event flows through.
- `src-tauri/src/config.rs` — the `Config` struct, its serde defaults, and
  its parse-heal tests.
- `src-tauri/src/poller.rs` and `src-tauri/src/http.rs` — **test call sites
  of `Engine::new` only** (three lines total; see the call-site table).
- `docs/TESTING_STRATEGY.md` — §0 records live test counts.

### The event being recorded (`src-tauri/src/event.rs:1-20`, do NOT modify)

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub priority: Priority,
    pub rotation: RotationSpec,
    pub topic: Option<String>,
    pub payload: EventPayload,
    #[serde(default)]
    pub meta: EventMeta,
    pub signal: EventSignal,
    /// Which source produced this event (v6: rotation-order tie-break) —
    /// orthogonal to `Priority`, which still decides cross-tier order
    /// first. Always server-assigned, never accepted from the `/notify`
    /// wire (same rule as `rotation`/`topic`).
    pub origin: SourceKind,
}
```

`Event` already derives `Serialize + Deserialize`, so it round-trips through
JSON with no new code. `RotationSpec` (`event.rs:91-96`) is the discriminator
this plan gates on:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RotationSpec {
    OneShot { ttl_secs: u64 },
    Recurring { display_secs: u64 },
}
```

### The rotation exemplar to mirror (`src-tauri/src/logging.rs:101-128`)

This is the pattern the operator's decision named. Match its **semantics**
exactly (see Step 1 for the one deliberate divergence):

```rust
    fn rotate_if_needed(&self, buf_len: usize) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.size + buf_len as u64 > inner.max_size && inner.size > 0 {
            Self::rotate_locked(&mut inner)?;
        }
        Ok(())
    }

    fn rotate_locked(inner: &mut Inner) -> io::Result<()> {
        for i in (1..inner.max_files).rev() {
            let src = inner.dir.join(format!("{}.{}", inner.filename, i));
            let dst = inner.dir.join(format!("{}.{}", inner.filename, i + 1));
            if src.exists() {
                fs::rename(&src, &dst)?;
            }
        }

        let current = inner.dir.join(&inner.filename);
        let backup = inner.dir.join(format!("{}.{}", inner.filename, 1));
        fs::rename(&current, &backup)?;

        inner.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current)?;
        inner.size = 0;
        Ok(())
    }
```

Note the two semantics its own tests pin (`logging.rs:157-233`) and that
yours must reproduce: rotation fires only when `size + buf > max && size > 0`
(so an oversized single line lands whole in an empty file rather than
rotating forever), and the cascade caps at `max_files`.

### The tail-read exemplar (`src-tauri/src/logging.rs:41-53`)

```rust
pub fn read_recent_lines(n: usize) -> anyhow::Result<Vec<String>> {
    read_recent_lines_from(&log_dir()?.join("notchtap.log"), n)
}

fn read_recent_lines_from(path: &Path, n: usize) -> anyhow::Result<Vec<String>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let lines: Vec<String> = contents.lines().map(str::to_string).collect();
    Ok(lines[lines.len().saturating_sub(n)..].to_vec())
}
```

Two conventions to carry over: a **missing file reads as an empty Vec, not
an error** (fresh install), and the tail slice uses `saturating_sub`.

### The ingest path to hook (`src-tauri/src/engine.rs:234-261`)

```rust
    pub async fn accept(
        &self,
        event: Event,
        bypass_pause_when_slot_empty: bool,
    ) -> Result<(), QueueError> {
        let to_offer = event.clone();
        let now = Instant::now();
        let slot_change = {
            let mut q = self.queue.lock().await;
            let enqueue_result = if bypass_pause_when_slot_empty {
                q.enqueue_test(event, now)
            } else {
                q.enqueue(event, now)
            };
            if let Err(ref e) = enqueue_result {
                tracing::warn!(id = %to_offer.id, origin = ?to_offer.origin, error = ?e, "accept: enqueue rejected");
            }
            enqueue_result?;
            tracing::debug!(id = %to_offer.id, origin = ?to_offer.origin, priority = ?to_offer.priority, "accept: enqueued");
            q.slot_state_if_changed()
        };
        self.wake.notify_waiters();
        if let Some(state) = slot_change {
            emit_slot_state(&self.app, state);
        }
        if to_offer.origin != SourceKind::News {
            for connector in self.connectors.iter() {
                connector.offer(&to_offer);
            }
        }
        Ok(())
    }
```

Note: this logging is deliberately **content-clean** — id/origin/priority
only, never title or body. Your new log lines must match that (Step 3).

### The `Engine` struct + hand-written Clone (`src-tauri/src/engine.rs:92-120`)

Post-073 shape (`engine.rs:92-120`) — note `live`/`weather` are now
`AmbientSlot<T>`, not `Arc<StdMutex<Option<T>>>`:

```rust
pub struct Engine<R: tauri::Runtime = tauri::Wry> {
    queue: Arc<Mutex<SingleSlotQueue>>,
    wake: Arc<tokio::sync::Notify>,
    app: tauri::AppHandle<R>,
    connectors: Arc<Vec<ConnectorHandle>>,
    telegram_health: Arc<StdMutex<ConnectorHealth>>,
    live: AmbientSlot<LiveMatchSummary>,
    weather: AmbientSlot<WeatherSummary>,
    espn_enabled: bool,
    rss_enabled: bool,
    weather_enabled: bool,
}

impl<R: tauri::Runtime> Clone for Engine<R> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            wake: self.wake.clone(),
            app: self.app.clone(),
            connectors: self.connectors.clone(),
            telegram_health: self.telegram_health.clone(),
            live: self.live.clone(),
            weather: self.weather.clone(),
            espn_enabled: self.espn_enabled,
            rss_enabled: self.rss_enabled,
            weather_enabled: self.weather_enabled,
        }
    }
}
```

Your `history: Option<Arc<HistoryStore>>` field goes alongside the three
bools, and gets one `history: self.history.clone()` line in that Clone.
Do NOT convert it to an `AmbientSlot` — that type is for the ambient
status side-channels (live/weather), a different concern entirely.

and its constructor (`engine.rs:125-146`):

```rust
    pub fn new(
        queue: SingleSlotQueue,
        app: tauri::AppHandle<R>,
        connectors: Arc<Vec<ConnectorHandle>>,
        telegram_health: Arc<StdMutex<ConnectorHealth>>,
        espn_enabled: bool,
        rss_enabled: bool,
        weather_enabled: bool,
    ) -> Self {
```

**`Engine::new` has exactly 9 call sites** — verified at `172fd63` with
`grep -rn "Engine::new(" src-tauri/src/`:

| File:line | Kind |
|---|---|
| `lib.rs:261` | **the one production site** |
| `lib.rs:992` | test |
| `engine.rs:437` | test helper |
| `engine.rs:506` | test |
| `engine.rs:535` | test |
| `engine.rs:762` | test |
| `poller.rs:1420` | test |
| `poller.rs:2130` | test |
| `http.rs:274` | test |

Adding a parameter touches all nine — expected mechanical churn, not scope
creep; the compiler names every one. **The two `poller.rs` sites are the
ones an earlier attempt of this plan tripped over** (the plan said 7); they
are plain test call sites that need `None,` as the new final argument and
nothing else. If you count anything other than 9, STOP and report.

### The config-field convention (`src-tauri/src/config.rs`)

Follow plan 085's `resting_state` exactly — field + serde default fn +
initializer + tests:

```rust
    /// plan 085: the overlay's RESTING (idle) render choice — the cheap
    /// half of plan 079 item 17. ...
    #[serde(default = "default_resting_state")]
    pub resting_state: RestingState,
```

with `fn default_resting_state() -> RestingState` at `config.rs:275`, the
initializer at `config.rs:376`, and tests at `config.rs:523` and
`config.rs:540-550`:

```rust
    fn resting_state_defaults_to_rail_and_is_overridable() {
        // ...
        assert_eq!(healed.resting_state, RestingState::Rail);

        let notch = Config::parse("resting_state = \"notch\"\n").unwrap();
        assert_eq!(notch.resting_state, RestingState::Notch);
```

The config directory helper already exists (`config.rs:401-403`):

```rust
    pub fn dir_from_home(home: &std::path::Path) -> PathBuf {
        home.join(".config").join("notchtap")
    }
```

### Module declarations (`src-tauri/src/lib.rs:1-21`)

Alphabetical. `mod history;` goes between `pub mod event;` and `mod hover;`.

### Production `Engine::new` call site (`src-tauri/src/lib.rs:255-265`)

```rust
            let engine = Engine::new(
                initial_queue,
                app.handle().clone(),
                connectors.clone(),
                telegram_health.clone(),
                espn_enabled,
                rss_enabled,
                weather_enabled,
            );
```

Config fields are hoisted into locals earlier (`lib.rs:200-202`) — follow
that precedent:

```rust
    let weather_temp_hot_c = config.weather_temp_hot_c;
    let weather_temp_cold_c = config.weather_temp_cold_c;
    let weather_priority = config.weather_priority;
```

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass; total = 428 + your new tests |
| Rust lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Test inventory | `cd src-tauri && cargo test --locked -- --list 2>/dev/null \| grep -c ': test$'` | the live total, for §0 |
| Frontend tests | `npx vitest run` (repo root) | 183 pass, unchanged (you touch no frontend file) |

If `cargo` is not on PATH, prefix with `PATH="$HOME/.cargo/bin:$PATH"`.

Baseline at `68edd66`: **428 rust tests + 3 doc-tests / 183 frontend.**
Re-derive live rather than trusting these numbers.

## Scope

**In scope** (the only files you may modify):
- `src-tauri/src/history.rs` (create)
- `src-tauri/src/lib.rs` (module declaration + store construction + the one
  `Engine::new` call site + its test call site at ~:992)
- `src-tauri/src/engine.rs` (field, Clone, `new` signature, `accept` hook,
  test call sites, new tests)
- `src-tauri/src/http.rs` (**only** the `Engine::new` test call site at
  ~:274 — one added argument, nothing else)
- `src-tauri/src/poller.rs` (**only** the two `Engine::new` test call sites
  at ~:1420 and ~:2130 — one added `None,` argument each, nothing else. Any
  other change to this file is out of scope.)
- `src-tauri/src/config.rs` (one field + default fn + initializer + tests)
- `docs/TESTING_STRATEGY.md` (§0 counts only)

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/logging.rs` — do NOT refactor `SizeRotatingAppender` to be
  shared. It is private, tuned for high-frequency tracing writes, and owns a
  cached file handle this plan deliberately does not need. Duplicating ~30
  lines of rotation logic is the correct call here; a shared abstraction
  would couple two things with different write profiles. Copy the pattern,
  not the code.
- `src-tauri/build.rs`, `src-tauri/capabilities/*.json` — no new
  `#[tauri::command]` in this plan. The invoke commands are plan 089's job.
  Adding one here without the `build.rs` opt-in would silently expose it to
  the overlay window, breaking the receive-only guarantee (CLAUDE.md).
- Any frontend file (`src/**`) — this plan ships no UI.
- `src-tauri/src/event.rs` — `Event` already serializes; it needs no change.
- `src-tauri/src/notifier.rs` — the title-on-drop log line was explicitly
  decided to stay as-is. Do not "clean it up."
- `src-tauri/src/queue.rs` — no queue semantics change.

## Git workflow

- Branch: whatever worktree branch you were dispatched on; do not create
  another.
- Conventional commits, matching `git log` style — e.g.
  `feat(history): JSONL store + engine write hook (plan 088)`.
- Commit per logical step or once at the end; do NOT push or open a PR.

## Steps

### Step 1: Create `src-tauri/src/history.rs` — the store

Create the file with a `HistoryEntry` record and a `HistoryStore`.

Target shape (write idiomatic code in this repo's style; this is the
contract, not a transcript):

```rust
/// One recorded notification. `recorded_at_ms` is wall-clock epoch millis
/// (the queue's own timing is `Instant`-based and deliberately clock-free,
/// so history stamps its own time here). Matches the `_ms: i64` convention
/// already used by `EventMeta::published_at_ms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub recorded_at_ms: i64,
    pub event: Event,
}

pub struct HistoryStore {
    path: PathBuf,
    max_size: u64,
    max_files: usize,
    // serializes append+rotate; a torn rotation is the only real race here
    lock: Mutex<()>,
}
```

Required API:

- `pub fn new(dir: impl AsRef<Path>) -> io::Result<Self>` — `create_dir_all`
  the dir, `path = dir.join("history.jsonl")`, defaults
  `max_size = 5 * 1024 * 1024`, `max_files = 2` (current + one `.1` backup).
- `pub fn with_limits(dir: impl AsRef<Path>, max_size: u64, max_files: usize) -> io::Result<Self>`
  — same, with explicit caps. **Tests must use this**; a test that writes to
  the real `~/.config/notchtap/` is a bug.
- `pub fn append(&self, event: &Event) -> io::Result<()>` — take `lock`,
  serialize a `HistoryEntry { recorded_at_ms: now_ms(), event: event.clone() }`
  via `serde_json::to_string`, rotate if needed (same predicate as
  `logging.rs`: `size + line_len > max_size && size > 0`), then append the
  line plus `\n` to an `OpenOptions::new().create(true).append(true)` handle.
- `pub fn read_recent(&self, n: usize) -> io::Result<Vec<HistoryEntry>>` —
  read the current file only (rotated backups stay out of scope, exactly
  like `logging.rs:35-40`'s documented choice); missing file → `Ok(vec![])`;
  parse each line with `serde_json::from_str`, **skipping** lines that fail
  to parse (a crash can leave a torn final line; one bad line must never
  poison the whole read); return the last `n` in file order, oldest → newest
  (same ordering contract as `read_recent_lines`; the UI reverses).
- `pub fn clear(&self) -> io::Result<()>` — remove `history.jsonl` and every
  `history.jsonl.{i}` backup; a missing file is success, not an error.

**Deliberate divergence from `logging.rs`, document it in a header comment**:
this store stats the file per append instead of caching an open handle and a
running size. `logging.rs` writes thousands of lines/second and needs the
cached handle; history writes a handful per hour, so the simpler stat-per-
append avoids holding a file handle inside the `Engine` for the app's entire
lifetime. Say this in the file, so a future reader doesn't "fix" it.

Add `mod history;` to `src-tauri/src/lib.rs` between `pub mod event;` and
`mod hover;`.

**Verify**: `cd src-tauri && cargo build --locked` → exit 0.

### Step 2: Add the `history_enabled` config flag

In `src-tauri/src/config.rs`, following the `resting_state` precedent
exactly:

- Field on `Config`, with a doc comment naming the plan and the default
  rationale:

```rust
    /// plan 088 (from plan 059's operator decision): persist accepted
    /// one-shot notifications to `~/.config/notchtap/history.jsonl` for
    /// later browsing. Defaults to `false` like every other opt-in surface
    /// here (`rss_enabled`, `weather_enabled`, `espn_live_card`,
    /// `espn_rich_events`) — this one writes notification CONTENT to disk,
    /// including cmux payloads, so off-by-default is load-bearing, not
    /// stylistic.
    #[serde(default = "default_history_enabled")]
    pub history_enabled: bool,
```

- `fn default_history_enabled() -> bool { false }` beside the other
  `default_*` fns (~`config.rs:275`).
- The initializer in the defaults constructor (~`config.rs:376`).

**Verify**: `cd src-tauri && cargo test --locked config::` → all pass.

### Step 3: Wire the store into `Engine` and hook `accept`

In `src-tauri/src/engine.rs`:

- Add field `history: Option<Arc<HistoryStore>>` to `Engine`.
- Add it to the hand-written `Clone` impl (`history: self.history.clone()`).
- Add `history: Option<Arc<HistoryStore>>` as the **last** parameter of
  `Engine::new`, and update all 9 call sites. Every test call site passes
  `None` except the new tests in Step 5.

  Why `Option<Arc<HistoryStore>>` rather than a `history_enabled: bool`
  alongside the other three bools: "disabled" and "where do I write" are one
  concept, and injecting the store is what makes the hook testable against a
  temp dir instead of the operator's real config directory.

- In `accept`, immediately before the final `Ok(())`:

```rust
        // plan 088: best-effort history append. ONE-SHOT ONLY —
        // `Recurring` is the ambient live-scoreboard card, which
        // topic-supersedes on every poll cycle; recording it would bury
        // the discrete notifications this feature exists to recover under
        // ~100 near-identical score updates per match. A write failure
        // must never fail an accept: the notification already promoted.
        // Log id/origin only — never title/body, matching this function's
        // existing content-clean logging.
        if let Some(store) = &self.history {
            if matches!(to_offer.rotation, RotationSpec::OneShot { .. }) {
                if let Err(e) = store.append(&to_offer) {
                    tracing::warn!(id = %to_offer.id, origin = ?to_offer.origin, error = %e, "history append failed");
                }
            }
        }
```

Place it **after** the wake, emit, and connector fan-out — a slow disk must
never delay the visible promotion or the outbound relay.

**Verify**: `cd src-tauri && cargo build --locked` → exit 0, and
`cargo test --locked engine::` → all existing engine tests still pass.

### Step 4: Construct the store in `lib.rs`

At the production `Engine::new` call site (`lib.rs:255-265`), build the
store when the flag is on. Follow the `weather_enabled` hoisting precedent
(`lib.rs:200-202`) for reading the config field.

Target shape:

```rust
    // plan 088: `None` when disabled (the default) — the Engine's hook is
    // then a no-op and behavior is byte-identical to pre-088. A store that
    // fails to open (unwritable config dir) degrades to `None` with a
    // warning rather than failing boot; history is a convenience, not a
    // correctness requirement.
    let history = if config.history_enabled {
        match dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))
            .and_then(|h| Ok(history::HistoryStore::new(Config::dir_from_home(&h))?))
        {
            Ok(store) => Some(std::sync::Arc::new(store)),
            Err(e) => {
                tracing::warn!(error = %e, "history disabled: could not open store");
                None
            }
        }
    } else {
        None
    };
```

Adapt to whatever error plumbing already exists at that point in `run()` —
match the surrounding code rather than importing new error machinery. Then
pass `history` as the new last argument.

**Verify**: `cd src-tauri && cargo build --locked` → exit 0.

### Step 5: Tests

Write these in `src-tauri/src/history.rs`'s own `#[cfg(test)] mod tests`,
modelled structurally on `logging.rs:145-281` — note especially its
`temp_dir()` helper, which uses a fresh UUID-named directory per test **and
explains why** (a shared dir would silently shift the rotation arithmetic):

```rust
    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("notchtap-historytest-{}", Uuid::new_v4()))
    }
```

Required cases:

1. `append_then_read_recent_round_trips` — append 3 events, read back 3
   entries, oldest → newest, with title/body/origin intact.
2. `missing_file_reads_as_empty_not_error` — `read_recent` on a store whose
   file was never created returns `Ok(vec![])`.
3. `read_recent_returns_only_the_last_n` — append 10, read 3, assert you get
   entries 8/9/10 in that order.
4. `malformed_line_is_skipped_not_fatal` — hand-write a file with two valid
   JSON lines and one garbage line between them; assert `read_recent`
   returns exactly the 2 valid entries.
5. `rotation_at_threshold_creates_backup` — use `with_limits` with a tiny
   `max_size`, append enough to cross it, assert `history.jsonl.1` exists
   and the live file restarted.
6. `empty_current_file_never_rotates` — mirror `logging.rs:220-233`: a
   single line larger than `max_size` lands whole in the empty file, no
   backup created.
7. `clear_removes_current_and_backup` — after rotation, `clear()` leaves
   neither `history.jsonl` nor `history.jsonl.1`, and a subsequent
   `read_recent` returns empty.

In `src-tauri/src/engine.rs`'s test module:

8. `accept_records_one_shot_event_to_history` — build an Engine with a
   `with_limits` store in a temp dir, `accept` a `OneShot` event, assert
   `read_recent(10)` returns exactly 1 entry whose title matches.
9. `accept_does_not_record_recurring_event` — same setup, `accept` a
   `Recurring` event, assert `read_recent(10)` is empty. **This is the
   tripwire for the core design decision** — if someone later "simplifies"
   the gate away, this test must fail.
10. `accept_with_history_disabled_writes_nothing` — Engine built with
    `None`; assert no `history.jsonl` is created in the temp dir.

In `src-tauri/src/config.rs`'s test module:

11. `history_enabled_defaults_to_false_and_is_overridable` — model on
    `resting_state_defaults_to_rail_and_is_overridable` (`config.rs:540`):
    an empty/healed config is `false`; `history_enabled = true` parses as
    `true`.

**Verify**: `cd src-tauri && cargo test --locked` → all pass, total =
baseline + 11.

### Step 6: Update `docs/TESTING_STRATEGY.md` §0

Re-derive the counts live (`cargo test --locked -- --list`, grouped by
module) and update the rust row, adding a `history N (new with plan 088: …)`
entry and bumping the `engine` and `config` figures with their own
parentheticals, matching the existing per-plan attribution style already in
that row. Do NOT touch the frontend row — this plan changes no frontend
file, so it must still read 183.

**Verify**: your written total equals a fresh
`cargo test --locked -- --list 2>/dev/null | grep -c ': test$'`, and the
per-module figures sum to it.

### Step 7: Full gate run

```
cd src-tauri && cargo test --locked
cd src-tauri && cargo clippy --locked --all-targets -- -D warnings
cd src-tauri && cargo fmt --check
cd .. && npx vitest run
```

**Verify**: all four exit 0; vitest still reports 183.

## Test plan

Covered above in Step 5 — 11 new tests: 7 storage-layer (round-trip,
missing file, tail-n, malformed-line tolerance, rotation, no-rotate-when-
empty, clear), 3 engine-hook (one-shot recorded, recurring skipped, disabled
writes nothing), 1 config (default false + override). Structural pattern:
`src-tauri/src/logging.rs:145-281` for the storage tests, the existing
`engine::tests` module for the hook tests, and
`config.rs:540-550` for the config test.

The single most important assertion is #9 (`Recurring` is not recorded) —
it pins the decision that separates "notifications I missed" from live-
scoreboard noise.

## Done criteria

ALL must hold:

- [ ] `cd src-tauri && cargo test --locked` exits 0; total = the `68edd66`
      baseline (428) + 11, re-derived live, not assumed
- [ ] `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings`
      exits 0
- [ ] `cd src-tauri && cargo fmt --check` exits 0
- [ ] `npx vitest run` still reports **183** (no frontend file touched)
- [ ] `grep -c "history" src-tauri/build.rs` returns **0** — this plan adds
      no invoke command
- [ ] `git diff --stat -- src-tauri/capabilities/` is **empty**
- [ ] `git diff --stat -- src/` is **empty** (no frontend change)
- [ ] `git status` shows no modified file outside the in-scope list
- [ ] `docs/TESTING_STRATEGY.md` §0's rust total matches a fresh
      `cargo test -- --list` count exactly
- [ ] With `history_enabled` absent from a config (the default), a built
      binary creates no `history.jsonl` — proven by test #10, not by hand

## STOP conditions

Stop and report — do not improvise — if:

- The drift check shows any in-scope file changed since `68edd66` and the
  "Current state" excerpts no longer match the live code.
- `Engine::new` turns out to have a different number of call sites than the
  **9** named in the table above — report the count rather than guessing
  which are tests. (An earlier attempt hit exactly this: the plan said 7,
  reality was 9. The table is now verified, but verify it yourself.)
- Adding the `history` field to `Engine` forces a change to `Engine`'s
  generic bounds or to `AppState` — the plan assumes it does not; if it
  does, the design needs review, not a workaround.
- Any test in Step 5 requires writing to a real (non-temp) directory to
  pass. Tests must never touch `~/.config/notchtap/`.
- You find yourself needing to modify `logging.rs`, `build.rs`,
  `capabilities/`, or any file under `src/` to make something work — all
  four are hard out-of-scope boundaries here.
- `cargo clippy` demands you make `HistoryStore` fields public or derive
  traits that would leak the file path into the wire format.

## Maintenance notes

- **Plan 089 (the UI) consumes this**: it adds `get_history` and
  `clear_history` invoke commands calling `read_recent`/`clear`, and both
  MUST be added to `src-tauri/build.rs`'s command list and
  `capabilities/settings.json` — otherwise they become callable from the
  overlay window, breaking the receive-only guarantee. That is the single
  highest-risk part of the follow-up.
- **The ordering contract is oldest → newest.** `read_recent` mirrors
  `logging.rs::read_recent_lines`. If a future UI wants newest-first, it
  reverses at the display layer; do not flip the rust contract, or plan
  089's tests will silently invert.
- **The one-shot gate is a product decision, not an optimization.** If
  someone later wants live-match history, the right shape is capturing the
  final state per topic at full-time, not removing the gate.
- **Rotation drops old history silently** — by design (size-capped was the
  operator's choice). If "I know it happened last month" ever becomes a real
  need, that is a retention-policy change, not a bug fix.
- What a reviewer should scrutinize: that `accept`'s hook sits after the
  wake/emit/fan-out; that the new log line carries no title or body; that
  every test uses a temp dir; and that test #9 genuinely fails if the
  `matches!` gate is removed.
