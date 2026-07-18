# Plan 014: Test the log-rotation engine and the eval-splice escaping (two untested load-bearing pure surfaces)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat b43a7ca..HEAD -- src-tauri/src/logging.rs src-tauri/src/lib.rs docs/TESTING_STRATEGY.md`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none (coordinates with plan 004's TESTING_STRATEGY edits — see Maintenance notes)
- **Category**: tests
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18 (excerpts re-verified unchanged)

## Why this matters

Two pieces of genuinely decision-carrying pure logic have zero tests, and
both violate the repo's own recorded rule (`docs/TESTING_STRATEGY.md`
§5.1: "if a module on this list grows a real decision … it comes off the
list and gets a test module in the same change"):

1. **`logging.rs` grew a hand-rolled size-rotation engine** (~80 lines:
   threshold check, cascade rename `.1→.2→.3`, size accounting) while
   §5.1 still describes it as "rotation glue … no decision logic worth
   asserting". An off-by-one here silently destroys or unboundedly grows
   the log — the artifact the docs call "the only tell" when the app
   misbehaves.
2. **The eval-splice escaping in `lib.rs`** — three hand-rolled
   replacements (U+2028, U+2029, `<`) protecting a `webview.eval(...)`
   splice fed by external RSS/cmux text. The comment itself documents the
   `</script>` breakout a missing replacement would reopen. Untested, and
   not on §5.1's exemption list. Per the repo's §4.4 discipline the fix
   is: extract the pure function, test it, leave the eval call thin.

## Current state

`src-tauri/src/logging.rs` — the appender (no `#[cfg(test)]` module in
this file today):

```rust
// logging.rs:56-78 (constructor takes injectable dir/size/count — already testable)
impl SizeRotatingAppender {
    fn new(dir: impl AsRef<Path>, filename: impl AsRef<str>, max_size: u64, max_files: usize) -> io::Result<Self> { … }

// logging.rs:81-87
    fn rotate_if_needed(&self, buf_len: usize) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.size + buf_len as u64 > inner.max_size && inner.size > 0 {
            Self::rotate_locked(&mut inner)?;
        }
        Ok(())
    }

// logging.rs:89-108 — the cascade
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
        inner.file = OpenOptions::new().create(true).append(true).open(&current)?;
        inner.size = 0;
        Ok(())
    }
```

`SizeRotatingAppender` is `struct` + `impl Write` (write → rotate_if_needed
→ write to file, size += written). Note: `new()` and the struct are
currently private — tests inside the same file's `#[cfg(test)]` module can
use them without visibility changes.

`src-tauri/src/lib.rs:337-346` — the splice shield (inside `on_page_load`):

```rust
let safe_json = state_json
    .replace('\u{2028}', "\\u2028")
    .replace('\u{2029}', "\\u2029")
    .replace('<', "\\u003c");
let _ = webview.eval(format!("window.__NOTCHTAP_SLOT_STATE__ = {safe_json};"));
```

(`state_json` is `serde_json::to_string(&current_state)` of a `SlotState`
whose title/body carry arbitrary external text.)

Temp-dir test pattern to copy: `settings.rs`'s write-path tests use temp
dirs (find them via `rg -n "tempfile\|TempDir\|std::env::temp_dir" src-tauri/src/settings.rs` and match whatever mechanism they use — the repo has no
tempfile crate in dev-deps unless settings tests pull one; read first).
Counts live ONLY in `docs/TESTING_STRATEGY.md` §0.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Module tests | `cargo test logging::` / `cargo test escape_for_eval` (from `src-tauri/`) | all pass |
| Full suite | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/logging.rs` (add `#[cfg(test)] mod tests` only — no
  production-logic changes)
- `src-tauri/src/lib.rs` (extract `escape_for_eval_splice` — a pure,
  behavior-identical refactor — plus tests in the existing test module)
- `docs/TESTING_STRATEGY.md` (§0 counts; §5.1 — remove/reword the
  logging.rs entry per its own de-listing rule)
- `plans/README.md` (status row)

**Out of scope**:
- Changing rotation size/count, log paths, filters, or the double
  fmt-layer (a separate low-priority audit note).
- The emit/eval mechanism itself or the `slot-state` event (plan 009).

## Git workflow

- Current branch; commits:
  1. `logging: test the size-rotation engine (threshold, cascade, reset)`
  2. `lib: extract + test escape_for_eval_splice (u+2028/u+2029/</script> shield)`
- Do NOT push.

## Steps

### Step 1: Rotation tests

Add `#[cfg(test)] mod tests` to `logging.rs`. Use a unique temp dir per
test (`std::env::temp_dir().join(format!("notchtap-logtest-{}", uuid))` —
`uuid` is a dependency; or match settings.rs's exact pattern). Drive the
appender through `io::Write` with small sizes (e.g. `max_size = 100`,
`max_files = 3`). Cases:

1. `no_rotation_below_threshold` — write 50 bytes twice (total 100, not
   `> 100`): one file, no `.1`.
2. `rotation_at_threshold_creates_backup_and_resets` — write 60 + 60:
   second write triggers rotation first (`60 + 60 > 100 && 60 > 0`); after
   it, `notchtap.log` contains only the second write's bytes, `.1` the
   first's.
3. `cascade_caps_at_max_files` — force 4 rotations; assert exactly
   `notchtap.log`, `.1`, `.2`, `.3` exist and no `.4`. **Read
   `rotate_locked` carefully when writing the expected values**: the loop
   `for i in (1..max_files).rev()` with `max_files = 3` renames `.2→.3`
   then `.1→.2` then current→`.1` — meaning `.3` is written and a
   previous `.3` is overwritten by the rename; total retained = current +
   3 backups. If your reading of the retention count differs from the
   docs' "keep 3", the TEST should pin actual behavior and your report
   should flag the doc/code mismatch (do not change the code).
4. `empty_current_file_never_rotates` — first-ever write larger than
   max_size: `size == 0` so the `inner.size > 0` guard skips rotation and
   the oversized line lands in the current file.

**Verify**: `cargo test logging::` → 4 pass.

### Step 2: Extract and test the splice escape

In `lib.rs`, hoist the three replacements into:

```rust
/// Makes a serde_json string safe to splice into eval'd JS source:
/// U+2028/U+2029 are legal in JSON but illegal raw in JS source, and
/// `<` closes the gap JSON leaves (it doesn't escape `/`, so a literal
/// "</script>" would otherwise break out of the script context).
fn escape_for_eval_splice(json: &str) -> String {
    json.replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .replace('<', "\\u003c")
}
```

Call it from `on_page_load` (behavior-identical: same three replacements,
same order). Move the original inline comment onto the function. Tests
(pure, in lib.rs's test module — note that module is gated
`#[cfg(all(test, target_os = "macos"))]`; these tests are platform-free
but living there is fine):

1. `script_close_tag_cannot_survive` — input containing `</script>` →
   output contains no `<` character at all.
2. `line_separators_escaped` — input with a real U+2028 and U+2029 →
   output contains `\\u2028` / `\\u2029` and neither raw char.
3. `round_trips_as_json` — for a `SlotState`-shaped JSON string with all
   three hazards in the title, `serde_json::from_str::<serde_json::Value>(&escaped)`
   still parses and the title's VALUE round-trips unchanged (the escapes
   are valid JSON escapes — this pins "safe for JS" AND "still the same
   data").

**Verify**: `cargo test` from `src-tauri/` → all pass (full suite proves the on_page_load call-site refactor broke nothing at compile/behavior level).

### Step 3: Docs

- `docs/TESTING_STRATEGY.md` §5.1: remove the `logging.rs` entry (or
  narrow it to "subscriber init only — the rotation engine is tested,
  §0"), and if plan 004 hasn't landed, leave its other entries alone.
- §0: logging +4, lib +3 (adjust to actuals).

**Verify**: `cargo test` green; `grep -n "no decision logic worth asserting" docs/TESTING_STRATEGY.md` → 0 hits.

## Test plan

As per steps: 4 temp-dir rotation tests + 3 pure escape tests. Patterns:
settings.rs temp-dir write tests (filesystem), any table-driven pure test
in `presentation.rs` (escape cases).

## Done criteria

- [ ] `grep -c "mod tests" src-tauri/src/logging.rs` → 1
- [ ] `grep -c "escape_for_eval_splice" src-tauri/src/lib.rs` → ≥3
- [ ] `cargo test` exits 0 with 7 new tests
- [ ] clippy/fmt gates exit 0
- [ ] TESTING_STRATEGY §5.1 + §0 updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- Step 1 case 3 reveals the retention behavior differs from what any doc
  states AND fixing the test-vs-doc mismatch would require changing
  `rotate_locked` — pin behavior in the test, report the mismatch, do not
  change the engine.
- The lib.rs test module's platform gate makes the escape tests
  uncompilable in CI (it won't — CI is macos for rust) — report if so.

## Maintenance notes

- Plan 004 also edits TESTING_STRATEGY §5.1 (the lib.rs entry) — whoever
  lands second reconciles; the edits are adjacent, not conflicting.
- Reviewers: the escape function must keep exactly these three
  replacements; serde_json handles everything else. Adding a fourth is
  fine; removing one needs the tests to fail first.
- If the log path or rotation params ever become configurable, these
  tests already take them as parameters — extend, don't rewrite.
