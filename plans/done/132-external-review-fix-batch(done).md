# Plan 132: external-review fix batch (Gemini 2.5 Pro + GPT-5.2 rounds, 2026-07-24)

> Filed 2026-07-24 from the operator's two external review rounds
> (`docs/review-logs/2026-07-24-notchtap-codereview-gemini2.5pro.md`,
> `docs/review-logs/2026-07-24-notchtap-codereview-gpt5.2.md`). Every
> finding below was independently re-verified against master `e94628c`
> by the reviewing session before filing. Declined findings are listed
> at the bottom — do NOT implement them.

## Scope: five small fixes, three Rust files + one shell line

### 1. `src-tauri/src/crests.rs` — crest URL allowlist (SSRF hardening)

`try_fetch` currently fetches whatever URL the ESPN feed hands it
(`client.get(url)` at ~line 116); the URL originates from `team.logo`
in ESPN's JSON (`poller.rs:399` builds the id→logo map,
`poller.rs:1306` schedules the fetch). Add a **pure, exported**
predicate in `crests.rs`:

```rust
/// A crest URL is fetchable only if it is https and points at ESPN's
/// own CDN — the URL arrives from a network feed, so it is untrusted
/// input, not a trusted fetch target (same posture as `path_for`'s
/// team-id sanitization).
pub fn crest_url_allowed(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "https" {
        return false;
    }
    match parsed.host_str() {
        Some(host) => host == "espncdn.com" || host.ends_with(".espncdn.com"),
        None => false,
    }
}
```

**Enforcement point: the map-building/scheduling side in `poller.rs`,
NOT inside `try_fetch`.** Filter non-conforming URLs out where
`team.logo` enters the id→logo map (~line 399 area) so a disallowed
URL never reaches `fetch_and_store` (log at `debug`, include the
rejected host, never the full URL at warn level). Reason this must
not go inside `try_fetch`: the existing wiremock tests
(`successful_fetch_writes_the_file_and_becomes_a_cache_hit`, the 404
and oversized-body tests) fetch from `http://127.0.0.1:<port>` mock
servers and MUST keep passing unmodified — they test fetch/cache
mechanics, not URL policy.

Also in this file: move the `std::fs::create_dir_all(&self.dir)` call
out of `try_fetch` (per-fetch) into `CrestCache::new` (once at
construction). Keep the `?`-compatible behavior: `new` currently can't
fail, so do it best-effort there (`let _ =` with a comment) AND keep
the call in `try_fetch` — no, simpler: keep `new` infallible, move
nothing structural — just add the `create_dir_all` in `new` as
best-effort and leave `try_fetch`'s as-is (it is the correctness
backstop if the dir is deleted mid-run). This sub-item is style-only;
if it grows any complexity, skip it and note that in the report.

**Tests (crests.rs `#[cfg(test)]`):** pure-fn cases —
`https://a.espncdn.com/i/teamlogos/soccer/500/360.png` → true;
`https://espncdn.com/x.png` → true; `http://a.espncdn.com/x.png` →
false (scheme); `https://evil.com/x.png` → false;
`https://espncdn.com.evil.com/x.png` → false (suffix-spoof);
`https://127.0.0.1/x.png` → false; `not a url` → false.
**Tests (poller.rs):** the existing logo-map test area (~line 1465+,
`patch_crests` neighborhood) gets one case: a team whose `logo` is
`http://…` or a non-ESPN host does not enter the map / never schedules
a fetch.

### 2. `src-tauri/src/settings.rs` — poison-tolerant Config lock (6 sites)

Replace `state.inner().lock().unwrap()` with
`state.inner().lock().unwrap_or_else(|e| e.into_inner())` at exactly
these sites (verified line numbers at `e94628c`): 678 (`get_config`),
747 (`save_config_and_relaunch`), 777 (`send_test_notification`), 830
(`search_news_now`), 904 + 908 (`set_appearance`). This is the
pattern the same file already uses for the secrets lock at line ~503 —
match it exactly. No behavior change on the un-poisoned path.

### 3. `src-tauri/src/settings.rs` — `set_appearance` single-lock read-modify-write

Current shape (lines ~903-911): clone under lock → write disk →
re-acquire lock → mutate memory. Two rapid calls can interleave into a
stale disk write. Restructure to ONE guard held across the whole
sequence, preserving the disk-write-first failure semantics (a failed
write must leave managed state untouched):

```rust
let dir = notchtap_config_dir()?;
let mut managed = state.inner().lock().unwrap_or_else(|e| e.into_inner());
let mut config = managed.clone();
config.appearance = appearance.clone();
write_config_atomic(&dir, &config).map_err(|e| format!("could not write config.toml: {e}"))?;
managed.appearance = appearance;
drop(managed);
broadcast_appearance_change(&app, &config);
Ok(())
```

(`set_appearance` is a sync command — holding the std mutex across the
atomic file write is acceptable and is the point of the fix.)

### 4. `src-tauri/src/queue.rs` — de-panic two `expect`s

- Line ~374 (`rotate_out_if_elapsed`):
  `item.promoted_at.expect("visible items have promoted_at")` → a
  graceful `let … else` that logs a `tracing::warn!` (one line, e.g.
  "visible item missing promoted_at — rotation check skipped") and
  returns. A future promotion-path bug must degrade to a stuck-card
  log, not a panicked rotation task and a silently frozen overlay.
  Mirror `current_slot_state`'s existing graceful posture for the same
  invariant. If `tracing` isn't already imported/used in queue.rs,
  check first and follow whatever the file does elsewhere (it may use
  fully-qualified `tracing::warn!`).
- Line ~320-322 (topic re-tier move):
  `.remove(pos).expect("position just found")` → `if let Some(mut
  existing) = self.waiting[tier_idx].remove(pos) { apply_fresh_content(…);
  push_back; }` keeping the `return true` contract identical. No
  logging needed (currently unreachable; this just removes the panic
  site at no cost).

**Tests:** no new tests required for 322 (unreachable). For 374, add
one unit test only if there is a clean way to construct a visible item
with `promoted_at = None` through existing test helpers; if the type's
invariants make that contortionist, skip the test and say so in the
report — the change is defensive, not behavioral.

### 5. `hooks/notchtap-claude-hook.sh:69` — UTF-8-safe truncation

`… | jq -r '(.tool_input // {}) | tostring' | cut -c1-200` →
`… | jq -r '(.tool_input // {}) | tostring | .[0:200]'` (jq slices by
codepoint, `cut` can split a multi-byte character). One line; no other
hook changes.

## Ledger

Update `docs/TESTING_STRATEGY.md` §0 with the new rust test count
(base at `e94628c`: 542 rust + 3 doc-tests / 380 frontend; frontend
count unchanged by this plan). Count from the actual `cargo test`
output, don't arithmetic it.

## Declined findings — do NOT implement

- `ipc.ts` runtime validation of invoke results (accepted asymmetry)
- `history.rs` `read_recent` full-file read (bounded by 5MB rotation)
- SearchNowRow/SecretRow + section fetch-shape dedup (documented
  per-section independence)
- `copyConfig` structuredClone future-proofing
- Appearance debounce
- `engine.rs` manual Clone, `http.rs` allocation micro-opt

## Constraints

- `capabilities/default.json` byte-identical; no new commands, so the
  settings command triple (15) is untouched — verify no drift anyway.
- No new dependencies (`reqwest::Url` is already available via
  reqwest; do NOT add the `url` crate to Cargo.toml).
- Naming rules per CLAUDE.md (no third-party branding).

## Verification ladder

From `src-tauri/`: `cargo test`, `cargo clippy -- -D warnings`,
`cargo fmt --check`. From repo root: `npx tsc --noEmit`,
`npx vitest run`, `npx biome ci .`, `npx vite build`.
`git diff src-tauri/capabilities/default.json` must be empty.
(cargo needs `PATH="$HOME/.cargo/bin:$PATH"` on this machine.)

## STOP conditions

- The wiremock crest tests can only pass by weakening the URL policy
  (means the enforcement point drifted into `try_fetch` — re-read
  item 1).
- `set_appearance` single-lock shape conflicts with something the
  broadcast path expects (report, don't redesign the broadcast).
