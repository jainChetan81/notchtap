# Plan 012: Harden ⌃⇧O open-story — reap the child, open the parsed URL, test the scheme gate

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d40445e..HEAD -- src-tauri/src/lib.rs`
> On any change, compare the excerpt below; mismatch = STOP. NOTE: plan
> 001 in this directory ("wire skip and open-settings hotkeys") also edits
> lib.rs's shortcut area — if it landed first, the surrounding code may
> have moved; the function itself should be intact.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug / security / tests
- **Planned at**: commit `d40445e`, 2026-07-17

## Why this matters

`open_current_story` (the ⌃⇧O handler) is the repo's one path where
external data (an RSS feed's link) reaches a privileged API
(`/usr/bin/open`). Three gaps:

1. **Zombie per press**: `Command::new("open").arg(&url).spawn()` drops
   the `Child` without `wait()` — each exited child stays a zombie in the
   process table for the life of a 24/7 app.
2. **Validated ≠ executed**: the scheme check runs on the *parsed* URL,
   but the *original raw string* is what gets passed to `open`. The WHATWG
   parser strips embedded tab/CR/LF and trims whitespace before
   validating, so the accepted value and the executed argv can differ.
   Passing the parsed serialization (and `open -u`, which forces URL
   interpretation) collapses that gap to zero.
3. **The gate has zero tests** while its three sibling handlers have
   five. The classic regression — "simplify" to
   `url.starts_with("http")`, which admits `httpx://` — would ship
   silently. (The settings suite already fixed this exact prefix-vs-parse
   bug class for feed URLs.)

## Current state

`src-tauri/src/lib.rs:649-674`:

```rust
#[cfg(target_os = "macos")]
fn open_current_story<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    queue: &Arc<Mutex<SingleSlotQueue>>,
) {
    let url = {
        let q = queue.blocking_lock();
        let Some(url) = q.current_link() else {
            tracing::debug!("open story ignored: no visible article link");
            return;
        };
        url.to_string()
    };

    let is_http = reqwest::Url::parse(&url)
        .map(|parsed| parsed.scheme() == "http" || parsed.scheme() == "https")
        .unwrap_or(false);
    if !is_http {
        tracing::debug!(%url, "open story ignored: link is not a valid http(s) url");
        return;
    }

    if let Err(error) = std::process::Command::new("open").arg(&url).spawn() {
        tracing::debug!(%error, %url, "open story command could not be spawned");
    }
}
```

Conventions to match: the repo's §4.4 discipline — "separate the decision
from the boundary": pure decision functions get unit tests; the
subprocess call stays the thin untested boundary. The sibling handlers'
tests live at the bottom of `lib.rs` (`#[cfg(all(test, target_os =
"macos"))] mod tests`, ~line 676) using `tauri::test::mock_app()`-style
setups — model the no-op test on the existing `dismiss_current`-style
tests there. Counts live ONLY in `docs/TESTING_STRATEGY.md` §0.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Lib tests | `cargo test --lib` (from `src-tauri/`) | all pass |
| Full suite | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/lib.rs` (`open_current_story` + new pure fn + tests)
- `docs/TESTING_STRATEGY.md` §0 (lib count)
- `plans/README.md` (status row)

**Out of scope**:
- The other three hotkey handlers, shortcut registration, `current_link`
  in queue.rs.
- Adding an opener crate/plugin — the no-new-dependency `open` approach
  is the recorded v5.1 decision; this plan refines, not replaces, it.

## Git workflow

- Current branch; commit style: `lib: open-story — normalized url via open -u, reap child, extract tested scheme gate`.
- Do NOT push.

## Steps

### Step 1: Extract and test the pure gate

Add near `open_current_story`:

```rust
/// Returns the normalized (parsed re-serialized) URL iff the link is a
/// well-formed http(s) URL — the ONLY thing ⌃⇧O will hand to `open`.
/// Full parse, never a prefix check: `starts_with("http")` admits
/// `httpx://` (the same trap the settings feed validation already fixed).
fn openable_http_url(raw: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(raw).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(parsed.to_string()),
        _ => None,
    }
}
```

Tests (table-driven, in the existing lib.rs test module — these are pure,
no mock app needed):
- `https://example.com/a` → `Some(...)` equal to the parsed serialization
- `http://example.com` → Some
- `httpx://example.com` → None (the prefix trap)
- `file:///etc/hosts` → None
- `javascript:alert(1)` → None
- `notaurl` → None
- `"  https://example.com  "` and `"https://exa\tmple.com/pa\nth"`
  (embedded tab/newline) → whatever `Url::parse` yields, asserted
  explicitly: the important property is the RETURNED string equals
  `parsed.to_string()` (normalized), never the raw input. Write the
  assertion as `result == Some(reqwest::Url::parse(raw).unwrap().to_string())`
  for the accepting cases.

**Verify**: `cargo test --lib` from `src-tauri/` → new tests pass.

### Step 2: Use it, force URL interpretation, reap the child

Rewrite the tail of `open_current_story`:

```rust
    let Some(normalized) = openable_http_url(&url) else {
        tracing::debug!(%url, "open story ignored: link is not a valid http(s) url");
        return;
    };

    // -u forces URL interpretation (never a file-path fallback), and the
    // argument is the parser's own serialization — what was validated is
    // exactly what executes. The child is reaped off-thread: a dropped,
    // un-waited Child is a zombie until this 24/7 process exits.
    match std::process::Command::new("open").arg("-u").arg(&normalized).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => {
            tracing::debug!(%error, %normalized, "open story command could not be spawned");
        }
    }
```

**Verify**: `cargo test` → all pass. `cargo clippy --all-targets -- -D warnings && cargo fmt --check` → exit 0.

### Step 3: No-op path test + counts

If the existing test module doesn't already cover "⌃⇧O with no visible
link is a no-op", add one modeled on the existing no-op-style handler
tests (drive the mock queue empty, call `open_current_story`, assert no
panic and queue state unchanged — the spawn is unreachable without a
link, so the test never launches anything). Update
`docs/TESTING_STRATEGY.md` §0 lib count.

**Verify**: `cargo test` → all pass.

### Step 4: One manual smoke (operator, dev machine, `rss_enabled = true`)

Wait for a headline, press ⌃⇧O → article opens in the browser; press ⌃⇧O
on the idle pill → nothing. Then `ps aux | grep -c '[o]pen'` after a few
presses → no accumulating defunct entries (`ps aux | grep defunct` →
empty).

**Verify**: operator confirmation (or explicitly reported as pending — the unit gates above are the automated proof).

## Test plan

- 7+ table-driven cases for `openable_http_url` (see Step 1) — pattern:
  any existing table-driven test in `lib.rs`/`presentation.rs`.
- 1 no-op handler test (Step 3) — pattern: the existing
  `dismiss`/`toggle` no-op tests in `lib.rs`'s test module.
- The `open` spawn itself stays untested by design (§4.4 boundary rule).

## Done criteria

- [ ] `grep -c "openable_http_url" src-tauri/src/lib.rs` → ≥3 (fn + call + tests)
- [ ] `grep -c '"-u"' src-tauri/src/lib.rs` → 1
- [ ] `grep -c "child.wait()" src-tauri/src/lib.rs` → 1
- [ ] `cargo test` exits 0 with the new tests
- [ ] clippy/fmt gates exit 0
- [ ] `docs/TESTING_STRATEGY.md` §0 updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- The `open_current_story` excerpt doesn't match (plan 001 from the
  earlier session may have restructured the shortcut area — reconcile by
  reading, and STOP only if the function's logic itself changed).
- `open -u` is rejected by the macOS version on the dev machine (verify
  with `open -u https://example.com` manually if unsure — it's supported
  on all modern macOS); if it fails, report rather than dropping `-u`.

## Maintenance notes

- Any future handler that passes external data to a subprocess should
  copy this shape: pure tested gate returning the normalized value, thin
  spawn, off-thread reap.
- If the earlier session's plan 001 (skip hotkey) adds more handlers,
  they don't launch processes — no interaction beyond textual proximity.
