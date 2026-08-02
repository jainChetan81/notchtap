# Plan 178: Make the prefix watchdog deadline-aware and its forced release self-healing

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src-tauri/src/lib.rs src-tauri/src/tabs.rs`
> If either file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Plan 171's prefix keymap registers eleven BARE system-wide key grabs
(`1`-`5`, `[`, `]`, `enter`, `o`, `p`, `esc`) for a 2-second armed window.
A 5-second watchdog exists as the dead-man's switch that force-releases
the grabs if anything wedges — because a stuck bare `Enter` breaks the
user's typing in **every application** until notchtap restarts. Two flaws
in the shipped watchdog defeat its own purpose:

1. **It kills legitimate re-armed windows.** Every arm spawns a watchdog
   that sleeps 5s and then force-releases unconditionally. Arm at t0, let
   the window lapse, arm again anywhere in (t0+3s, t0+5s) — an ordinary
   "hesitate, time out, try again" rhythm — and the FIRST arm's watchdog
   fires mid-way through the SECOND arm's live window, silently releasing
   its grabs. The follow-up key the user then presses does nothing in
   notchtap and leaks to the focused app.
2. **A failed forced release never retries.** `force_release_prefix_followups`
   clears `followups_registered` FIRST (`swap(false)`) and discards the
   result of the actual unregister call. If that release partially fails
   — the exact catastrophic case the code's own PAL-consensus comment
   documents — the flag now says "released", every later watchdog
   early-returns on it, and the grab is stuck until restart. This inverts
   the module's own stated invariant ("a failed release keeps
   `followups_registered` true so the watchdog retries"), which the other
   two callers honour.

## Current state

All code is in `src-tauri/src/lib.rs` (macOS-gated). `src-tauri/src/tabs.rs`
holds `TabWire` (the shared state bundle: `prefix: Mutex<PrefixState>`,
`prefix_generation: AtomicU64`, `followups_registered: AtomicBool`, …).

`lib.rs:1940-1944` — the timeout:

```rust
/// ... Comfortably past `PREFIX_ARM_WINDOW` (2s) so it never
/// races a legitimate window, short enough that a stuck bare `Enter` is
/// measured in seconds rather than "until the app restarts".
const PREFIX_WATCHDOG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
```

`lib.rs:2018-2040` — `set_prefix_followups_registered(app, on) -> bool`
registers/unregisters all eleven grabs and returns whether EVERY key
reached the requested state (failed release logs at ERROR).

`lib.rs:2047-2059` — the forced release with flaw 2:

```rust
fn force_release_prefix_followups<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    tab_wire: &Arc<tabs::TabWire>,
) {
    use std::sync::atomic::Ordering;
    if !tab_wire.followups_registered.swap(false, Ordering::SeqCst) {
        return;
    }
    tracing::warn!("prefix watchdog: force-releasing follow-up grabs");
    let _ = set_prefix_followups_registered(app, false);
    *tab_wire.prefix.lock().unwrap_or_else(|e| e.into_inner()) = prefix::PrefixState::Disarmed;
}
```

`lib.rs:2064-2113` — `handle_prefix_fire`: takes the state lock, calls
`st.on_prefix(now)`, bumps `prefix_generation`, registers grabs, and (when
armed) spawns TWO tasks:
- the generation-guarded 2s disarm timer (acts only if
  `prefix_generation` still equals its captured value);
- the watchdog with flaw 1:

```rust
let watchdog_app = app.clone();
let watchdog_wire = tab_wire.clone();
tauri::async_runtime::spawn(async move {
    tokio::time::sleep(PREFIX_WATCHDOG_TIMEOUT).await;
    force_release_prefix_followups(&watchdog_app, &watchdog_wire);
});
```

The watchdog is deliberately generation-blind (its comment: the net "that
catches what the generation-guarded timer above cannot — a wedged runtime,
a lost timer, a panic that unwound past the release, or an unregister that
failed per-key"). That property must be **preserved**: the fix is a time
deadline, not a generation gate — a generation gate would blind the
watchdog to the "follow-up consumed (generation bumped) but its release
failed" case, which only the watchdog can catch.

`handle_prefix_followup` (`lib.rs:2120-2135`) already keeps the flag true
on failed release: `if set_prefix_followups_registered(app, false) {
store(false) }`. `handle_prefix_fire` likewise:
`.store(armed || !all_ok, ...)`.

Repo conventions that apply:

- Pure decision + thin apply wrapper is the house pattern for exactly this
  kind of logic (see `silence_should_flip`'s comment at `lib.rs` ~2242:
  "pure decision, thin apply wrapper" — and `docs/TESTING_STRATEGY.md`
  §4.4). The watchdog's decision must be a pure, unit-tested function;
  the async loop stays thin.
- Real-timer async tests are an accepted exception ONLY for two existing
  `engine.rs` tests (recorded in `plans/README.md`'s rejected-findings).
  Do NOT add a third; test the pure function instead.

## Commands you will need

Run from `src-tauri/`.

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked` | all pass |
| Rust lints | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Prefix-only tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked prefix` | all pass |

(The frontend is untouched; `npx vitest run` must still be green at the
end, unchanged.)

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/lib.rs` (the prefix region: ~1940-2135)
- `src-tauri/src/tabs.rs` (only if the deadline field lands on `TabWire`)
- `docs/TESTING_STRATEGY.md` §0 (counts)

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/prefix.rs` — the pure ARM/DISARM state machine is
  correct; its stale header comment is plan 181's docs work, not yours.
- The generation-guarded 2s disarm timer — its logic is sound; only the
  watchdog changes.
- `set_prefix_followups_registered` itself — its return contract is
  already right.
- The Exit-hook call site that force-releases on quit — it may keep
  calling the unconditional function.

## Git workflow

- Branch: `advisor/178-prefix-watchdog`
- Commit style: conventional, e.g. `fix(prefix): deadline-aware watchdog + self-healing forced release`
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Track the newest arm instant

Add to `TabWire` (in `tabs.rs`, next to the existing prefix fields) a
`last_arm_at: Mutex<Option<std::time::Instant>>` (poison-tolerant access,
matching the file's existing `unwrap_or_else(|e| e.into_inner())` idiom).
Set it to `Some(now)` in `handle_prefix_fire` whenever `armed` becomes
true (the same `now` already taken at the top of the function).

**Verify**: `cargo test --locked` → all pass (no behaviour change yet).

### Step 2: Extract the pure watchdog decision

Add to `lib.rs` (near the prefix helpers) a pure function plus enum:

```rust
enum WatchdogVerdict {
    /// Nothing registered — the net is not needed.
    Done,
    /// A newer arm is still inside its legitimate watchdog budget —
    /// sleep again until `retry_after` past that arm.
    Reschedule(std::time::Duration),
    /// No arm within budget explains the live registration — release.
    Release,
}

fn watchdog_verdict(
    followups_registered: bool,
    last_arm_at: Option<std::time::Instant>,
    now: std::time::Instant,
    timeout: std::time::Duration,
) -> WatchdogVerdict
```

Semantics: not registered → `Done`. Registered and `last_arm_at` is
within `timeout` of `now` → `Reschedule(last_arm_at + timeout - now)`.
Otherwise (no arm recorded, or the newest arm is older than `timeout`) →
`Release`. Unit-test this exhaustively (table test, no timers): the
re-arm-at-4s scenario from "Why this matters" must yield `Reschedule`,
the lapsed-and-stuck scenario `Release`, the clean case `Done`, and the
`None` + registered case `Release`.

**Verify**: `cargo test --locked watchdog_verdict` → new tests pass.

### Step 3: Make the watchdog a verdict-driven loop

Replace the watchdog spawn's body with a loop:

```rust
tauri::async_runtime::spawn(async move {
    let mut sleep_for = PREFIX_WATCHDOG_TIMEOUT;
    loop {
        tokio::time::sleep(sleep_for).await;
        let last_arm = *watchdog_wire.last_arm_at.lock().unwrap_or_else(|e| e.into_inner());
        match watchdog_verdict(
            watchdog_wire.followups_registered.load(Ordering::SeqCst),
            last_arm,
            std::time::Instant::now(),
            PREFIX_WATCHDOG_TIMEOUT,
        ) {
            WatchdogVerdict::Done => return,
            WatchdogVerdict::Reschedule(d) => sleep_for = d,
            WatchdogVerdict::Release => {
                force_release_prefix_followups(&watchdog_app, &watchdog_wire);
                // Step 4 makes a failed release keep the flag true, so
                // loop again: the next verdict is Done on success,
                // Release again (bounded retry) on failure.
                sleep_for = PREFIX_WATCHDOG_TIMEOUT;
            }
        }
    }
});
```

Overlapping loops from rapid re-arms stay harmless: every loop converges
to `Done` once nothing is registered. Update the watchdog comment: keep
its list of caught failure modes, replace "ignoring generation entirely /
idempotent so overlapping watchdogs are harmless" with the deadline
rationale (generation-blind BY DESIGN, deadline-aware so it cannot kill a
newer legitimate window).

**Verify**: `cargo clippy --locked --all-targets -- -D warnings` → exit 0.

### Step 4: Let the forced release fail without lying

Rewrite `force_release_prefix_followups` so a failed release keeps the
retry flag true:

```rust
if !tab_wire.followups_registered.swap(false, Ordering::SeqCst) {
    return;
}
tracing::warn!("prefix watchdog: force-releasing follow-up grabs");
let all_ok = set_prefix_followups_registered(app, false);
if !all_ok {
    // The invariant every other caller honours (see handle_prefix_fire):
    // a failed RELEASE keeps the flag true so the watchdog retries.
    tab_wire.followups_registered.store(true, Ordering::SeqCst);
}
*tab_wire.prefix.lock().unwrap_or_else(|e| e.into_inner()) = prefix::PrefixState::Disarmed;
```

(The brief false window between `swap` and `store` is acceptable: the only
readers are the verdict loop — which retries anyway — and the handlers,
which re-register on the next arm.)

**Verify**: `cargo test --locked` → all pass.

### Step 5: Full gates + counts

Both cargo commands green from `src-tauri/`; `npx vitest run` from root
still green (untouched); recount `docs/TESTING_STRATEGY.md` §0.

## Test plan

- New rust unit tests for `watchdog_verdict` (Step 2's table: Done /
  Reschedule with exact duration / Release / None-but-registered). Place
  them in `lib.rs`'s existing `#[cfg(test)]` region beside the other
  prefix tests (find with `grep -n "mod tests" src-tauri/src/lib.rs` or the
  existing prefix test names).
- No real-timer async test — the loop stays untested by design (the pure
  verdict carries the logic); note that in a test-module comment.

## Done criteria

- [ ] `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` exit 0 (from `src-tauri/`)
- [ ] `watchdog_verdict` exists with ≥4 table-test cases, all passing
- [ ] `grep -n "let _ = set_prefix_followups_registered" src-tauri/src/lib.rs` → no match (the discarded-result call is gone)
- [ ] `npx vitest run` exits 0 (unchanged)
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated; §0 recounted

## STOP conditions

Stop and report back (do not improvise) if:

- The watchdog/force-release code no longer matches the excerpts (a
  concurrent fix landed).
- `TabWire` already carries an arm-instant or deadline field under another
  name — reuse it instead of adding a twin, and report the naming.
- The Exit hook (search `force_release` in `lib.rs`'s run/exit wiring)
  relies on the flag being cleared even on failure — if keeping the flag
  true breaks quit-time cleanup, report rather than special-casing.
- You are tempted to gate the watchdog on `prefix_generation` — that is
  explicitly the wrong fix (see "Current state"); stop and re-read.

## Maintenance notes

- The verdict function is the single place watchdog policy lives; any
  future change to `PREFIX_ARM_WINDOW`/`PREFIX_WATCHDOG_TIMEOUT` should
  re-run its table in review.
- Reviewer focus: Step 4's swap/store window, and that the loop cannot
  spin hot (every branch sleeps ≥ the rescheduled duration or returns).
- Live verification on hardware (operator-owed, same class as plan 171's
  TCC checks): arm, wait 4s, re-arm, confirm the second window survives
  its full 2s; and the `esc` disarm still releases immediately.
