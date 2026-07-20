# Plan 014: Test the log-rotation engine and the eval-splice escaping (two untested load-bearing pure surfaces)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat af9be44..HEAD -- src-tauri/src/logging.rs src-tauri/src/lib.rs docs/TESTING_STRATEGY.md`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none (coordinates with plan 004's TESTING_STRATEGY edits — see Maintenance notes)
- **Category**: tests
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18; **review-plan pass 2026-07-18 at `af9be44`** — `logging.rs` excerpts re-verified byte-identical; `lib.rs` excerpts refreshed (the splice moved to ~429-433 AND grew a second identical site at ~451-453, both now covered by Step 2); §0 count targets pinned (249→256); plan-004 conditional resolved (004 landed at `0749235` — this plan lands second)

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
   replacements (U+2028, U+2029, `<`) protecting `webview.eval(...)`
   splices. The comment itself documents the `</script>` breakout a
   missing replacement would reopen. Untested, and not on §5.1's
   exemption list. Per the repo's §4.4 discipline the fix is: extract
   the pure function, test it, leave the eval calls thin. **The pattern
   has already been hand-duplicated once**: v5.1's appearance hot-apply
   added a second, identical three-replacement splice for
   `window.__NOTCHTAP_APPEARANCE__` (lib.rs:451-453) alongside the
   original slot-state one (lib.rs:429-433). Two live copies of a
   security shield is exactly how they drift apart — both call sites
   route through the extracted function in this plan.

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
use them without visibility changes. **Gotcha**: `new()` seeds `size`
from the existing file's length (`logging.rs:68`,
`let size = file.metadata()?.len();`) — every test MUST use a fresh,
empty temp dir or the threshold arithmetic silently shifts.

`src-tauri/src/lib.rs` — the splice shield, now at TWO call sites inside
`on_page_load`. Site 1 (slot-state, ~lib.rs:429-433; its `</script>`
comment block sits at 423-428):

```rust
let safe_json = state_json
    .replace('\u{2028}', "\\u2028")
    .replace('\u{2029}', "\\u2029")
    .replace('<', "\\u003c");
let _ = webview.eval(format!("window.__NOTCHTAP_SLOT_STATE__ = {safe_json};"));
```

Site 2 (appearance, ~lib.rs:450-455 — same three replacements chained
onto the serialize):

```rust
let payload_json = serde_json::to_string(&payload)
    .unwrap_or_else(|_| "null".into())
    .replace('\u{2028}', "\\u2028")
    .replace('\u{2029}', "\\u2029")
    .replace('<', "\\u003c");
let _ =
    webview.eval(format!("window.__NOTCHTAP_APPEARANCE__ = {payload_json};"));
```

(Site 1's `state_json` is `serde_json::to_string(&current_state)` of a
`SlotState` whose title/body carry arbitrary external text; site 2's
payload is config-derived appearance data — lower-risk, but it must use
the same shield so the two never drift apart.)

Temp-dir test pattern to copy — verified exemplar: `settings.rs:1195-1196`
has `fn temp_dir() -> PathBuf { std::env::temp_dir().join(format!("notchtap-settings-test-{}", Uuid::new_v4())) }`;
`uuid` (v4 feature) is already a main dependency (`Cargo.toml:26`).
Copy that helper shape into `logging.rs`'s test module (rename the
prefix to `notchtap-logtest-`). Counts live ONLY in
`docs/TESTING_STRATEGY.md` §0.

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
- The emit/eval mechanism itself or the `slot-state` event (plan 009,
  DONE — this plan touches only the escaping, not what gets emitted).

## Git workflow

- Current branch; commits:
  1. `logging: test the size-rotation engine (threshold, cascade, reset)`
  2. `lib: extract + test escape_for_eval_splice (u+2028/u+2029/</script> shield)`
- Do NOT push.

## Steps

### Step 1: Rotation tests

Add `#[cfg(test)] mod tests` to `logging.rs`. Use a unique temp dir per
test — copy the settings.rs helper shape (see Current state):
`std::env::temp_dir().join(format!("notchtap-logtest-{}", Uuid::new_v4()))`
(`use uuid::Uuid;` inside the test module). A fresh dir per test is
mandatory, not hygiene: `new()` seeds `size` from any pre-existing file.
Drive the appender through `io::Write` with small sizes (e.g.
`max_size = 100`, `max_files = 3`). Cases:

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

### Step 2: Extract and test the splice escape (BOTH call sites)

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

Call it from BOTH `on_page_load` sites shown in Current state —
site 1 becomes `let safe_json = escape_for_eval_splice(&state_json);`
and site 2 becomes `let payload_json = escape_for_eval_splice(&serde_json::to_string(&payload).unwrap_or_else(|_| "null".into()));`
(behavior-identical at each: same three replacements, same order).
Move site 1's inline `</script>` comment (lib.rs:423-428) onto the
function; site 2's "Double-shield …" comment refers to the global+emit
pairing, not the escaping — leave it in place. Tests
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
  §0"). Plan 004 has already landed, so its §5.1 edits are simply the
  text you see — no reconciliation conditional applies. Optionally add
  one clause to §5.1's `lib.rs` entry noting the eval-splice escaping
  is now extracted + tested (the untested-by-design page-load
  orchestration itself stays accurately listed).
- §0: rust total 249→256; module breakdown gets a new `logging 4`
  entry and `lib (hotkey) 9`→`lib 12`.

**Verify**: `cargo test` green; `grep -n "no decision logic worth asserting" docs/TESTING_STRATEGY.md` → 0 hits.

## Test plan

As per steps: 4 temp-dir rotation tests + 3 pure escape tests. Patterns:
settings.rs temp-dir write tests (filesystem), any table-driven pure test
in `presentation.rs` (escape cases).

## Done criteria

- [ ] `grep -c "mod tests" src-tauri/src/logging.rs` → 1
- [ ] `grep -c "escape_for_eval_splice" src-tauri/src/lib.rs` → ≥5
      (fn def 1 + doc-comment mention + 2 call sites + test references;
      the load-bearing part: exactly 2 call sites and ZERO remaining
      inline `.replace('<', "\\\\u003c")` chains outside the fn)
- [ ] `cargo test` exits 0 with 256 tests (249 baseline + 7 new)
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

- Plan 004 has landed (`0749235`); this plan's §5.1 edits apply on top
  of its text directly — no second-lander reconciliation remains.
- Reviewers: the escape function must keep exactly these three
  replacements; serde_json handles everything else. Adding a fourth is
  fine; removing one needs the tests to fail first. Any FUTURE eval
  splice must route through `escape_for_eval_splice` — reject a third
  hand-rolled copy on sight (that is how site 2 happened).
- If the log path or rotation params ever become configurable, these
  tests already take them as parameters — extend, don't rewrite.
