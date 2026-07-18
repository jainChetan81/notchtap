# Plan 021: Settings save polish — preserve feed metadata across URL edits, reject duplicates, pre-flight the port

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat 73c03ef..HEAD -- src/settings/SettingsApp.tsx src-tauri/src/settings.rs`
> On any change, compare excerpts below; mismatch = STOP. Plans 020
> (get_default_config) and 015 (heartbeat) already landed in these files;
> the excerpts below were refreshed against `73c03ef` to account for them,
> so a clean drift result is expected from that baseline.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW–MED (the port pre-flight changes save-path behavior)
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed
  to `b43a7ca` 2026-07-18 (excerpts re-verified unchanged);
  **review-plan pass 2026-07-18**: corrected stale line refs (saveConfig
  is at ~1114 not 835, the save command at 587-602 not 438-452, validate
  at 35), fixed Step 3's snippet which read `state.inner().port` — the
  managed state is `StdMutex<Config>` (v5.1), so the pre-flight must use
  the function's existing `booted` local; **reconcile pass 2026-07-18** at
  `73c03ef` (after plans 020 + 015 landed): excerpt CONTENT re-verified
  byte-identical (saveConfig's rebuild and `save_config_and_relaunch` were
  untouched by 020/015 — 020 only restructured the load effect, 015 added
  no wake to the relaunch path), only line refs moved — saveConfig now
  ~1092 (was 1114), `submittedConfig` 1095-1101, `lines()` ~168, the save
  command 595-610, the save-path mock ~187, settings `#[test]` count 39.

## Why this matters

Three sharp edges in the settings save flow:

1. **Editing a feed URL silently discards its metadata.** The form
   rebuilds `rss_feeds` by exact-string-matching each textarea line
   against the old entries; fix a typo or add a trailing slash and that
   feed's `source`/`category` reset to null — after relaunch its masthead
   reads the generic fallback with nothing explaining why.
2. **Duplicate feed lines are accepted** — neither the form nor
   `validate()` dedupes, so a pasted duplicate yields two identical
   `[[rss_feeds]]` entries polled twice per tick (the SeenStore hides the
   duplicate *notifications*, but the network work doubles, silently).
3. **Saving a colliding port bricks the app.** `validate` checks range
   only; save writes and relaunches; the relaunched process fails to bind
   and `exit(1)`s with no UI — the settings window re-creates exactly the
   bricked-boot failure it exists to prevent, recoverable only by
   hand-editing `~/.config/notchtap/config.toml`. A best-effort throwaway
   bind before writing catches the overwhelmingly common case.

## Current state

`src/settings/SettingsApp.tsx:1095-1101` (inside `saveConfig`,
~line 1092):

```tsx
const submittedConfig: Config = {
  ...config,
  espn_leagues: lines(espnLeaguesText),
  rss_feeds: lines(rssFeedsText).map((url) =>
    config.rss_feeds.find((feed) => feed.url === url) ?? { url, source: null, category: null },
  ),
};
```

`lines()` (same file, ~line 168) splits/trims/filters the textarea.
Note `config` here is the BOOTED config from `get_config` — so metadata
matching is against pre-edit entries only. There is no UI to edit
source/category; they come from the config file (hand-edit) or the
defaults; the form must simply not *destroy* them.

`src-tauri/src/settings.rs:35+` — `validate()` returns
`Result<(), Vec<String>>` with per-field messages; the rss_feeds rule
(full URL parse + scheme + host, `reqwest::Url::parse`, ~line 97) lives
in the same function. Port validation today (~lines 38-43): range only
(`port < 1024` → error; u16 caps the top).

The save command — `settings.rs:595-610`, quoted in full because Step 3
edits it (note the managed state is `StdMutex<Config>` and a `booted`
clone already exists at line 602):

```rust
pub fn save_config_and_relaunch(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, StdMutex<Config>>,
    config: Config,
) -> Result<(), Vec<String>> {
    ensure_settings_window(&window).map_err(|e| vec![e])?;
    let booted = state.inner().lock().unwrap().clone();
    let config = pin_uneditable_fields(config, &booted);
    validate(&config)?;
    let dir = notchtap_config_dir().map_err(|e| vec![e])?;
    write_config_atomic(&dir, &config)
        .map_err(|e| vec![format!("could not write config.toml: {e}")])?;
    tracing::info!("config saved from settings window — relaunching");
    app.restart();
}
```

Conventions: settings rust tests are exhaustive per-rule (39 as of this
refresh, same file); frontend tests via mockIPC (`SettingsApp.test.tsx`
— the `save_config_and_relaunch` payload-intercepting mock is at
~line 187). Counts in `docs/TESTING_STRATEGY.md` §0 only.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust suite | `cargo test settings::` then `cargo test` (from `src-tauri/`) | all pass |
| Frontend | `npx vitest run && npx tsc --noEmit` | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` | exit 0 |

## Scope

**In scope**:
- `src/settings/SettingsApp.tsx` (saveConfig's rss_feeds rebuild) +
  `SettingsApp.test.tsx`
- `src-tauri/src/settings.rs` (duplicate-feed rule in `validate`; port
  pre-flight in `save_config_and_relaunch`) + its tests
- `docs/TESTING_STRATEGY.md` §0
- `plans/README.md` (status row)

**Out of scope**:
- Adding source/category editing UI (bigger feature; the fix here is
  non-destruction).
- Boot-time port handling (`lib.rs`'s exit-on-bind-failure stays — the
  documented fail-fast decision; this plan only stops the settings
  window from *creating* that state).
- `validate`'s other rules; the atomic-write mechanics.

## Git workflow

- Current branch; commits:
  1. `settings: preserve feed metadata across url edits, reject duplicate feeds`
  2. `settings: best-effort port bind pre-flight before save-and-relaunch`
- Do NOT push.

## Steps

### Step 1: Metadata-preserving rebuild (frontend)

In `saveConfig`, match by *normalized* URL instead of exact string.
Normalization for matching only (the saved value stays what the user
typed): trim + strip a single trailing `/` + lowercase scheme/host is
overkill to hand-roll — use the URL parser, falling back to the raw
string for unparseable lines:

```tsx
function feedKey(url: string): string {
  try {
    const u = new URL(url);
    u.hash = "";
    return u.href.replace(/\/$/, "");
  } catch {
    return url.trim();
  }
}
```

then:

```tsx
rss_feeds: lines(rssFeedsText).map((url) =>
  config.rss_feeds.find((feed) => feedKey(feed.url) === feedKey(url))
    ?? { url, source: null, category: null },
),
```

This preserves metadata across trailing-slash/hash-only edits. A
*substantive* URL change (different path/host) still resets metadata —
correct, it IS a different feed. Note the limitation in a one-line
comment.

Frontend tests (model on the existing save-path tests in
`SettingsApp.test.tsx` — read how they fill the textarea and intercept
the `save_config_and_relaunch` mock's payload):
- editing a feed URL by appending `/` keeps its `source` in the submitted
  payload;
- replacing it with a different host submits null metadata.

**Verify**: `npx vitest run` → all pass incl. 2 new.

### Step 2: Duplicate rejection (rust, validate)

In `validate()`'s rss_feeds section, add: collect `feedKey`-equivalent
normalized URLs (rust side: `reqwest::Url::parse` → cleared fragment →
trailing-slash-trimmed string; fall back to the trimmed raw on parse
failure — parse failures already get their own error) and push an error
per duplicate: `format!("duplicate rss feed: {url}")`. Frontend dedup is
NOT added — server-side rejection with the message list is the repo's
existing UX for every other rule.

Rust tests (same file's test module, model on the neighboring rss rules'
tests): exact duplicate rejected; trailing-slash-variant duplicate
rejected; two genuinely different feeds accepted.

**Verify**: `cargo test settings::` → all pass incl. 3 new.

### Step 3: Port pre-flight (rust, save path)

In `save_config_and_relaunch`, after `validate(&config)?` and before
`write_config_atomic`, when the submitted port differs from the booted
one. Use the `booted` local that ALREADY EXISTS in the function (line
602's `let booted = state.inner().lock().unwrap().clone();`) — do NOT
write `state.inner().port`; the managed state is `StdMutex<Config>`, so
that doesn't compile:

```rust
if config.port != booted.port {
    // Best-effort pre-flight (plan 021): the relaunched app exits(1) on a
    // taken port with no UI — catch the common collision before writing.
    // A race remains possible (port taken between check and relaunch);
    // this narrows the window, it doesn't close it — accepted.
    if let Err(e) = std::net::TcpListener::bind(("127.0.0.1", config.port)) {
        return Err(vec![format!(
            "port {} is not bindable right now ({e}) — pick another or free it first",
            config.port
        )]);
    }
}
```

(The listener drops immediately, freeing the port for the relaunch. The
`!=` guard matters: the app itself holds the booted port, so binding it
would false-positive.) Add rust tests: (a) saving with the port UNCHANGED
never trips the pre-flight even while something holds that port — bind a
listener on an ephemeral port, make it both booted and submitted value;
(b) a submitted port held by a live listener returns the error vec;
(c) a free submitted port passes validation up to the write (the
existing command tests show how far the save path is drivable without a
real `app.restart()` — read them; if the pre-flight can't be reached
without the AppHandle, extract it as
`fn preflight_port(new: u16, booted: u16) -> Result<(), String>` and
test THAT, calling it from the command — prefer this extraction, it
matches the repo's decision/boundary split).

**Verify**: `cargo test settings::` → all pass incl. new; `cargo clippy --all-targets -- -D warnings && cargo fmt --check` → exit 0.

### Step 4: Counts

`docs/TESTING_STRATEGY.md` §0 (the two count rows in the "done" table):
in the rust row, bump the `settings` sub-count by the number of new
`#[test]` fns actually added (Steps 2–3) AND the row's leading total by
the same delta; in the frontend row, bump `settings form` by the number
of new `it()` blocks (Step 1 — likely 2) AND that row's leading total by
the same delta. Sub-counts must keep summing to each total, and §0 is
the only place counts live. Re-read the rows at execution time —
concurrent plans move them (at this refresh, master `73c03ef`: rust
`243 tests — settings 39, …`; frontend `64 tests — … settings form 11 …`;
and plans 013/023 are merging in parallel, so re-read §0 right before
editing and reconcile against whatever total is there then).

**Verify**: `cargo test` (full) + `npx vitest run` green.

## Test plan

As per steps: 2 frontend payload tests, ~3 duplicate-rule tests,
~3 pre-flight tests (via the extracted `preflight_port`). Exemplars: the
existing rss-URL validation tests (settings.rs) and the save-path mockIPC
tests (SettingsApp.test.tsx).

## Done criteria

- [ ] `grep -c "feedKey" src/settings/SettingsApp.tsx` → ≥2
- [ ] `grep -c "duplicate rss feed" src-tauri/src/settings.rs` → ≥1
- [ ] `grep -c "preflight_port\|TcpListener::bind" src-tauri/src/settings.rs` → ≥1
- [ ] `cargo test`, clippy, fmt, `npx vitest run`, `npx tsc --noEmit` all exit 0
- [ ] §0 updated
- [ ] `plans/README.md` status row updated

## STOP conditions

- The saveConfig excerpt doesn't match (plan 002 from the earlier session
  may have moved it) — reconcile by reading; STOP only if the rebuild
  logic itself changed.
- The pre-flight can't distinguish "the app's own listener" from a
  foreign process on the booted port in tests — the `!=` guard should
  make this moot; if not, report.
- `validate` gaining a network-ish side effect (the bind) offends a
  recorded purity rule you find in the specs — it shouldn't (the bind is
  in the command, not `validate`); keep it that way, and STOP if the
  extraction pressure pushes it into `validate`.

## Maintenance notes

- The pre-flight is best-effort by design (TOCTOU window remains) — do
  not "fix" the race with port-holding handoff complexity; the boot-time
  fail-fast is still the backstop.
- If feed source/category editing UI ever lands, the normalized-match
  rebuild becomes obsolete (the form would own metadata) — delete
  `feedKey` then.
- Reviewers: check the duplicate-rule message reads well in the settings
  window's error list (it renders the strings verbatim).
