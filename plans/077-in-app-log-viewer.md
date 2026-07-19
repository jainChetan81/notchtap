# Plan 077: Add a read-only "recent log lines" panel to the Settings window

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src-tauri/src/logging.rs src-tauri/src/settings.rs src-tauri/build.rs src-tauri/capabilities/settings.json src/settings/SettingsApp.tsx`
> If any of these changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW — read-only, settings-window-gated by the existing
  deny-by-default ACL
- **Depends on**: none (soft: consider landing after plan 070, if
  selected, so there's more worth reading in the log — not a hard
  dependency)
- **Category**: direction (grounded feature suggestion — see
  `plans/README.md`'s Direction section)
- **Planned at**: commit `f6c2f46`, 2026-07-20

## Why this matters

`docs/ARCHITECTURE.md` §11 states this explicitly: "this is a background
app — when something breaks, the user needs a log to read. set this up
in v1, not as an afterthought." The write side is fully built and tested
(`src-tauri/src/logging.rs` — rotation at 10MB, 3 backups, plan 014's
test suite) — but no read side exists anywhere. Confirmed via a grep of
`SettingsApp.tsx`'s 8 sidebar sections: no Logs/Activity/Diagnostics
section exists. Today's only way to read the log is leaving the app
entirely (Console.app or a terminal `tail ~/Library/Logs/notchtap/notchtap.log`)
and knowing the path by heart.

`ARCHITECTURE.md:474-477` explicitly rules out the *opposite* direction
(frontend errors being sent back into the log file — deliberately
excluded, receive-only-overlay law) but says nothing about reading the
log file's contents *into* the Settings UI — this is a different
direction the architecture doc doesn't foreclose, and its own §11
promise is exactly the grounding for building it.

A read-only "last N lines" panel in the Settings window (which already
exists and already has 8 sections) closes the gap between what §11
promises and what's actually reachable without a terminal. It also
becomes a natural home for plan 076's connector-health line, if that's
selected, and for anything plan 070's new logging (if selected) makes
worth surfacing.

## Current state

- `src-tauri/src/logging.rs:9-33` — `init_logging`/`log_dir`, the
  existing write-side (already fully excerpted in prior plans — the
  relevant fact here is `log_dir()`'s exact path construction):

  ```rust
  fn log_dir() -> anyhow::Result<PathBuf> {
      let home =
          dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
      let dir = home.join("Library").join("Logs").join("notchtap");
      fs::create_dir_all(&dir)?;
      Ok(dir)
  }
  ```

  The active log file is `{log_dir}/notchtap.log`; rotated backups are
  numbered (read `SizeRotatingAppender`'s implementation, right below
  this excerpt in the same file, for the exact backup-naming scheme
  before deciding whether Step 1 needs to read only the active file or
  also offer backups — recommended: active file only, keep this plan
  small, backups are already accessible via Console.app/terminal for the
  rare case someone needs history beyond the current file).

- `src-tauri/src/settings.rs:311-345` — `SecretStatus`'s masking
  precedent (`fn secret_status`, already excerpted in plan 076) is the
  closest analogue for "don't leak sensitive material through a
  settings-window read command" — relevant here because a log line could
  in principle carry something sensitive (the codebase already treats
  this class of risk carefully, per `notifier.rs`'s existing token-
  redaction work from plan 006). This plan's `get_recent_log_lines`
  should NOT do content-based redaction (that's a much bigger, fuzzier
  problem) — instead, rely on the fact that `notifier.rs`'s token
  redaction (`e.without_url()`, plan 006) already prevents secrets from
  reaching the log file in the first place. Note this explicitly in your
  implementation as the reason no additional redaction layer is being
  added here.

- The 4-file pattern for adding a new settings-window-only invoke
  command (identical to plan 076's Step 2 — read that plan's "Current
  state" section for the full list if you're executing this plan
  independently of 076): `settings.rs` (the command function),
  `build.rs` (`AppManifest::commands` allowlist), `lib.rs`
  (`generate_handler!` list), `capabilities/settings.json`
  (`permissions` array).

- `src/settings/SettingsApp.tsx` — read its existing sidebar-section
  structure (`grep -n "nav-item" src/settings/SettingsApp.tsx`, 8
  sections confirmed at planning time) to decide: new "Diagnostics"
  section, or fold into an existing one (e.g. a collapsed panel at the
  bottom of an existing section)? Recommended: a new minimal section —
  it's a distinct concern from the existing 8 (rotation/priority,
  appearance, connectors, etc.) and matches this repo's pattern of one
  section per concern.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Frontend lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/logging.rs` — a pure function reading the last N lines
  of the active log file (no rotation-side changes)
- `src-tauri/src/settings.rs` — the new `get_recent_log_lines` command
- `src-tauri/build.rs`, `src-tauri/src/lib.rs`,
  `src-tauri/capabilities/settings.json` — command registration (see the
  4-file pattern above)
- `src/settings/SettingsApp.tsx` — a new read-only panel/section

**Out of scope**:
- Reading rotated backup files (`.1`, `.2`, `.3`) — active file only, per
  the "Current state" recommendation above; keep this plan small.
- Live-tailing / auto-refresh — a manual "refresh" button or re-fetch on
  section-open is sufficient; a streaming/websocket-style live tail is a
  meaningfully bigger feature this plan doesn't need.
- Any content-based secret redaction on top of what `notifier.rs`'s
  existing token redaction already provides — see the "Current state"
  note above for why this is deliberately out of scope, not an oversight.
- `src-tauri/src/logging.rs`'s rotation/write logic — untouched.

## Steps

### Step 1: Add a pure "read last N lines" function to `logging.rs`

Add a function (e.g. `pub fn read_recent_lines(n: usize) -> anyhow::Result<Vec<String>>`)
that opens `{log_dir()}/notchtap.log`, reads it, and returns the last `n`
lines. Keep it simple — a full-file read plus a tail-slice is fine for a
10MB-capped file (the rotation cap already bounds worst-case file size);
don't over-engineer a streaming/seek-based tail reader for this size.
Handle the "file doesn't exist yet" case (a fresh install before
anything's been logged) by returning an empty `Vec`, not an error.

**Verify**: `cd src-tauri && cargo build` → exit 0.

### Step 2: Add the `get_recent_log_lines` invoke command

Follow the exact 4-file pattern (see "Current state"):
1. `settings.rs`: `pub async fn get_recent_log_lines(window: tauri::WebviewWindow) -> Result<Vec<String>, String>` — `ensure_settings_window(&window)?` first, then call `logging::read_recent_lines(200)` (or a similarly modest fixed count — no need to make this configurable), map the error to `String` matching every other command's error-handling style.
2. `build.rs`: add `"get_recent_log_lines"` to the `AppManifest::commands(&[...])` list.
3. `lib.rs`: add `settings::get_recent_log_lines,` to `generate_handler![...]`.
4. `capabilities/settings.json`: add `"allow-get-recent-log-lines"` to `permissions`.

**Verify**: `cd src-tauri && cargo build` → exit 0; `grep -c "get_recent_log_lines" src-tauri/build.rs src-tauri/src/lib.rs src-tauri/capabilities/settings.json src-tauri/src/settings.rs` → each returns at least 1.

### Step 3: Add the Settings-window panel

Add a new sidebar section (or fold into an existing one — your call per
"Current state") rendering the log lines in a monospace, read-only,
scrollable `<pre>`/`<textarea readOnly>` block, with a manual refresh
button calling `invoke("get_recent_log_lines")`. Fetch on section-open,
not on every app load (matches the "advisory fetch, isolated from the
critical panel load" pattern `CLAUDE.md` already describes for
`SecretStatus`).

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 4: Tests

- Rust: 2-3 tests for `read_recent_lines` in `logging.rs`'s `mod tests`
  (there's already a `mod tests` with a temp-dir pattern from plan 014 —
  reuse that exact fixture-construction approach): empty-file-returns-
  empty-vec, fewer-lines-than-n-returns-all, more-lines-than-n-returns-
  only-the-last-n.
- Frontend: 1 test in `SettingsApp.test.tsx` confirming the panel renders
  fetched lines from a mocked `invoke` response — mirror whatever
  existing test covers a similar fetch-on-mount/fetch-on-open pattern in
  this file.

**Verify**: `cd src-tauri && cargo test --locked logging::` → new tests pass; `npx vitest run` → new test passes.

### Step 5: Full suite + lint, both sides

**Verify**:
- `cd src-tauri && cargo test --locked` → all pass
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0
- `npx tsc --noEmit` → exit 0
- `npx vitest run` → all pass
- `npx biome ci .` → exit 0 (or matches pre-existing failure baseline exactly)
- `npx vite build` → exit 0

## Test plan

- Rust: 3 new tests in `logging.rs`'s `mod tests` — empty file, fewer
  lines than requested, more lines than requested (tail-slice
  correctness) — using the existing temp-dir fixture pattern from plan
  014's rotation tests in the same file.
- Frontend: 1 new test in `SettingsApp.test.tsx` — panel renders mocked
  log lines.
- Verification: `cargo test --locked logging::` and `npx vitest run` →
  all pass, including new cases.

## Done criteria

- [ ] `cargo test --locked` exits 0, including new `logging.rs` tests
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx vitest run` exits 0, including the new `SettingsApp` test
- [ ] `npx biome ci .` exits 0 (or matches pre-existing baseline)
- [ ] `npx vite build` exits 0
- [ ] `get_recent_log_lines` is registered in all 4 required places
- [ ] `capabilities/default.json` (overlay window) is byte-identical to before this plan (`git diff` empty) — this command must NEVER reach the overlay window
- [ ] `plans/README.md` status row for 077 updated

## STOP conditions

- The log file can grow large enough that a full-file read becomes a
  real perf concern before you've verified the 10MB rotation cap actually
  bounds it in practice — re-check `SizeRotatingAppender`'s rotation
  trigger before assuming "10MB max" holds exactly; if it doesn't, a
  tail-optimized read (seek-from-end) may be needed instead of the
  simple full-read approach — if so, that's still within this plan's S
  effort estimate, just implement the seek-based version instead of
  raising it as a blocker.
- Any of the 4 command-registration files don't match the pattern
  described (drift, e.g. an 8th or 9th command already added by another
  plan like 076) — re-read the live files and adjust, but don't skip any
  of the 4.

## Maintenance notes

- If plan 076 (Telegram connector health) also lands, consider whether
  its health line belongs in this same new Diagnostics section rather
  than the Connectors section — not required, just noted as a plausible
  follow-up UI consolidation, not something this plan should force.
- If log volume ever grows enough that reading rotated backups becomes
  genuinely useful (not just "nice to have"), that's a natural, small
  follow-up extension of `read_recent_lines` rather than a redesign.
