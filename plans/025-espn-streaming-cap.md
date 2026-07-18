# Plan 025: Stream-cap the ESPN scoreboard fetch and share the pollers' HTTP posture

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a58f115..HEAD -- src-tauri/src/poller.rs src-tauri/src/rss_poller.rs src-tauri/src/lib.rs src-tauri/src/net.rs docs/TESTING_STRATEGY.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M (small M — one new module, two call-site swaps, 3–4 tests)
- **Risk**: LOW
- **Depends on**: none
- **Category**: security
- **Planned at**: commit `a58f115`, 2026-07-18. **Review-plan pass
  2026-07-18 (advisor, against `d926977` + filing)**: every code
  excerpt re-verified byte-identical against live code (`fetch_league`,
  the rss streaming block, both client builders, the lib.rs module
  ordering, the `rss_poller.rs:439-441` tech-debt comment). Known
  benign drift: plan 022 landed after this plan was written — the
  drift check WILL show `docs/TESTING_STRATEGY.md` movement and §0 now
  reads **257 + 3 doc-tests**, not the 251 quoted below; that is the
  expected 022 drift, NOT a STOP (`poller.rs`/`rss_poller.rs`/`lib.rs`
  are unchanged since `a58f115`). Step 4's recount language governs
  the counts.

## Why this matters

Both pollers cap remote response bodies at 1 MiB, but only the RSS
poller enforces the cap safely. `rss_poller.rs::fetch_feed` streams the
body chunk-by-chunk and bails the moment the running total exceeds the
cap — its own comment explains that a chunked response with no
`Content-Length` would otherwise balloon memory far past the cap before
any check runs. The ESPN poller's `fetch_league` still has the pre-fix
shape: it checks `Content-Length` (which a misbehaving or compromised
endpoint can simply omit or understate), then buffers the **entire**
body with `.bytes()`, and only then checks the length. In a 24/7
resident process, that is an unbounded-memory hazard on the score path.

The two pollers also duplicate their HTTP client posture byte-for-byte
(UA, `redirect::Policy::limited(3)`, 10 s timeout) — they have already
diverged once (this very cap). This plan fixes the ESPN gap by
extracting one shared, wiremock-tested helper pair that both pollers
use, so the fetch posture cannot silently drift apart again.

## Current state

- `src-tauri/src/poller.rs` — ESPN scoreboard poller.
  - `MAX_SCOREBOARD_BYTES` at line 462 (`1024 * 1024`).
  - The vulnerable fetch (`poller.rs:500-514`):

    ```rust
    async fn fetch_league(client: &reqwest::Client, league: &str) -> anyhow::Result<String> {
        let url = format!("https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard");
        let response = client.get(&url).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_SCOREBOARD_BYTES as u64)
        {
            anyhow::bail!("scoreboard response exceeds 1 MiB");
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_SCOREBOARD_BYTES {
            anyhow::bail!("scoreboard response exceeds 1 MiB");
        }
        Ok(String::from_utf8(bytes.to_vec())?)
    }
    ```
  - The client construction (`poller.rs:563-567`, inside
    `spawn_espn_poller`) — byte-identical to the RSS one below.
  - A section comment at `poller.rs:495-498` says the fetch loop is
    "deliberately thin and untested (v2 spec §3)" — this plan makes the
    capped-read portion tested via the shared helper; amend that
    comment accordingly (see Step 4).
- `src-tauri/src/rss_poller.rs` — RSS poller, the exemplar.
  - `MAX_FEED_BYTES` at line 19 (`1024 * 1024`).
  - The safe streaming read (`rss_poller.rs:414-431`):

    ```rust
    if response
        .content_length()
        .is_some_and(|length| length > MAX_FEED_BYTES as u64)
    {
        anyhow::bail!("response body exceeds 1 MiB");
    }

    // Stream and bail as soon as the running total exceeds the cap, instead
    // of buffering the whole body first — a chunked response with no
    // Content-Length would otherwise let a misbehaving feed balloon memory
    // far past 1 MiB before the size check ever runs.
    let mut body: Vec<u8> = Vec::with_capacity(64 * 1024);
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_FEED_BYTES {
            anyhow::bail!("response body exceeds 1 MiB");
        }
        body.extend_from_slice(&chunk);
    }
    ```
  - The client construction (`rss_poller.rs:453-457`, inside
    `spawn_rss_poller`):

    ```rust
    let client = match reqwest::Client::builder()
        .user_agent("notchtap/0.1 (+https://github.com/jainChetan81/notchtap)")
        .redirect(reqwest::redirect::Policy::limited(3))
        .timeout(Duration::from_secs(10))
        .build()
    ```
  - Existing wiremock test that pins the error-message contract:
    `oversized_content_length_rejected` (`rss_poller.rs:1072-1089`)
    asserts `error.to_string().contains("1 MiB")`. **Any shared helper
    must keep "1 MiB" in its error string for a 1 MiB cap.**
- `src-tauri/src/lib.rs:1-15` — module declarations (`mod config;` …
  `mod settings;`); a new module must be declared here.
- `src-tauri/Cargo.toml:42` — `wiremock = "0.6"` is already a
  dev-dependency; `anyhow` and `reqwest` are already regular deps.

Conventions: library-internal fallible helpers here use `anyhow` at the
fetch boundary (both pollers already do); tests live in
`#[cfg(test)] mod tests` in the same file; wiremock test structure to
copy is `rss_poller.rs:981-1090` (`MockServer::start()`, `Mock::given`,
`ResponseTemplate`). Test counts live in `docs/TESTING_STRATEGY.md` §0
and only there. Naming: never reference third-party app names — use
`notchtap` / generic terms.

## Commands you will need

| Purpose | Command | Expected on success |
|---------|---------|---------------------|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass (257 + 3 doc-tests at current HEAD — 022 landed since planning; recount against §0 — plus this plan's new tests) |
| One module | `cd src-tauri && cargo test --locked net::` | new helper tests pass |
| Lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Format | `cd src-tauri && cargo fmt --check` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/net.rs` (create)
- `src-tauri/src/lib.rs` (one `mod net;` line only)
- `src-tauri/src/poller.rs`
- `src-tauri/src/rss_poller.rs`
- `docs/TESTING_STRATEGY.md` (§0 counts)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):
- The pollers' spawn signatures / the `#[allow(clippy::too_many_arguments)]`
  param bundles — a recorded tech-debt item, deliberately not absorbed
  here (comment at `rss_poller.rs:439-440`).
- `fetch_feed`'s 304/validator logic and `feed_rs` parsing — only the
  content-length check + streaming loop move into the helper.
- `diff_scoreboard` / `diff_feed` and everything below the "here is a
  response body" line — the pure tested surfaces.
- Backoff logic (`poller.rs` `BACKOFF_BASE` region).

## Git workflow

- Branch: `advisor/025-espn-streaming-cap`.
- Commit style (from `git log`): lowercase `area: imperative summary`,
  e.g. `pollers: share capped streaming fetch + client posture`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Create `src-tauri/src/net.rs` with the two shared helpers

New module with exactly two `pub(crate)` items plus tests:

```rust
//! Shared HTTP posture for the outbound pollers (plan 025): one client
//! builder and one capped body reader, so the espn and rss fetch paths
//! cannot drift apart again (they did once — the streaming cap landed
//! on rss only).

use std::time::Duration;

pub(crate) fn build_poll_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("notchtap/0.1 (+https://github.com/jainChetan81/notchtap)")
        .redirect(reqwest::redirect::Policy::limited(3))
        .timeout(Duration::from_secs(10))
        .build()
}

/// Read a response body, failing fast once `cap` bytes are exceeded —
/// checked against Content-Length up front AND enforced while
/// streaming, because a chunked response with no Content-Length would
/// otherwise buffer unbounded before any post-hoc check runs.
pub(crate) async fn read_body_capped(
    mut response: reqwest::Response,
    cap: usize,
) -> anyhow::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > cap as u64)
    {
        anyhow::bail!("response body exceeds {} MiB", cap / (1024 * 1024));
    }
    let mut body: Vec<u8> = Vec::with_capacity(64 * 1024);
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > cap {
            anyhow::bail!("response body exceeds {} MiB", cap / (1024 * 1024));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
```

The error string must render as `response body exceeds 1 MiB` for a
1 MiB cap — the existing rss test asserts `.contains("1 MiB")`. Note:
with a sub-MiB test cap (Step 2 uses `1024`) the same format renders
`response body exceeds 0 MiB`. That is acceptable in test fixtures —
do NOT "fix" the message format to make small caps read nicely; only
the 1 MiB rendering is contractual, and reformatting risks breaking
the rss test's `.contains("1 MiB")` assertion.

Add `mod net;` to the module list at the top of `src-tauri/src/lib.rs`
(alphabetical position: after `mod logging;` / `mod login_item;`,
before `mod notifier;` — match the existing ordering style).

**Verify**: `cd src-tauri && cargo build 2>&1 | tail -3` → compiles
(expect temporary `dead_code` warnings until Steps 2–3 wire the
callers; if clippy `-D warnings` is run at this point it will fail on
those — that is expected until Step 3).

### Step 2: Add wiremock tests for `read_body_capped` in `net.rs`

`#[cfg(test)] mod tests` in `net.rs`, modeled on
`rss_poller.rs:981-1090` (same imports:
`wiremock::matchers::{method, path}`, `wiremock::{Mock, MockServer, ResponseTemplate}`).
Use a small cap (e.g. `1024`) so fixtures stay tiny, plus one 1 MiB
message-format case:

1. `body_under_cap_returned_whole` — 200 with a body smaller than the
   cap → `Ok` with the exact bytes.
2. `oversized_content_length_rejected_before_read` — body larger than
   the cap (wiremock sets Content-Length automatically) → `Err`
   containing `"exceeds"`.
3. `streaming_bail_stops_before_buffering_whole_body` — the regression
   this plan exists for. Ideal fixture: an over-cap body whose
   Content-Length header is absent or understated, so only the
   streaming loop can catch it. Check whether wiremock 0.6 can produce
   that (e.g. a raw/chunked body variant of `ResponseTemplate`). If it
   cannot — wiremock normally sets an accurate Content-Length itself —
   fall back to a plain over-cap body and note in a test comment that
   the content-length fast path fires first in this fixture; do NOT
   weaken or restructure the helper just to make the ideal fixture
   testable.
4. `error_message_names_mib_for_mib_cap` — with `cap = 1024 * 1024`
   and an oversized body, assert `error.to_string().contains("1 MiB")`
   (pins the message contract the rss test depends on).

**Verify**: `cd src-tauri && cargo test --locked net::` → 4 tests pass.

### Step 3: Switch both pollers onto the helpers

1. `poller.rs::fetch_league` becomes:

   ```rust
   async fn fetch_league(client: &reqwest::Client, league: &str) -> anyhow::Result<String> {
       let url = format!("https://site.api.espn.com/apis/site/v2/sports/soccer/{league}/scoreboard");
       let response = client.get(&url).send().await?.error_for_status()?;
       let bytes = crate::net::read_body_capped(response, MAX_SCOREBOARD_BYTES).await?;
       Ok(String::from_utf8(bytes)?)
   }
   ```

2. In `poller.rs::spawn_espn_poller` (~line 563) and
   `rss_poller.rs::spawn_rss_poller` (~line 453), replace the inline
   `reqwest::Client::builder()...build()` expression with
   `crate::net::build_poll_client()` — keep the existing
   `match`/error-log-and-return shape around it.
3. In `rss_poller.rs::fetch_feed`, replace the content-length check +
   streaming loop (lines 414–431) with
   `let body = crate::net::read_body_capped(response, MAX_FEED_BYTES).await?;`
   — everything before (304 handling, status check, validator reads)
   and after (`feed_rs::parser::parse(&body[..])`, validator persist)
   stays byte-identical. Note `fetch_feed` currently declares
   `let mut response` for the chunk loop; the `mut` moves into the
   helper, so drop it here if the compiler warns.

**Verify**: `cd src-tauri && cargo test --locked` → full suite green,
including the untouched `rss_poller` wiremock tests
(`oversized_content_length_rejected`, `not_modified_returns_none_and_preserves_state`,
`validators_not_persisted_on_parse_failure`, `validators_persisted_on_success`,
`conditional_headers_sent_when_state_has_validators`).

### Step 4: Reconcile comments, docs, counts

- Amend `poller.rs`'s section comment (lines 495–498): the fetch loop
  is still thin, but the capped body read is now the tested
  `net::read_body_capped` — reword so the comment stays true.
- `docs/TESTING_STRATEGY.md` §0: add a `net` row with its test count
  and bump the rust total accordingly (recount from the actual
  `cargo test` output, don't arithmetic blindly).

**Verify**: `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check` → exit 0.

## Test plan

- New tests: the 4 `net::tests` cases in Step 2, modeled on
  `rss_poller.rs:981` (`mod fetch_feed_tests`).
- Regression safety net: the five existing `fetch_feed` wiremock tests
  must pass unmodified — they prove the rss refactor preserved
  behavior. No poller test may be edited by this plan.
- Verification: `cd src-tauri && cargo test --locked` → all pass; count
  = old count + 4.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `cd src-tauri && cargo test --locked` exits 0 with 4 new `net::` tests
- [ ] `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `cd src-tauri && cargo fmt --check` exits 0
- [ ] `grep -n "response.bytes()" src-tauri/src/poller.rs` → no matches
- [ ] `grep -n "Client::builder" src-tauri/src/poller.rs src-tauri/src/rss_poller.rs` → no matches (both use `net::build_poll_client`)
- [ ] `grep -c "read_body_capped" src-tauri/src/poller.rs src-tauri/src/rss_poller.rs` → 1 each
- [ ] No existing test in `rss_poller.rs` was modified (`git diff --stat` shows only in-scope files; `git diff src-tauri/src/rss_poller.rs` shows no hunk inside `mod tests`)
- [ ] `docs/TESTING_STRATEGY.md` §0 matches the actual counts
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `fetch_league` or the rss streaming block no longer match the
  "Current state" excerpts.
- Any existing `rss_poller` wiremock test fails after Step 3 — that
  means the helper's behavior diverged from the original; report the
  failing assertion rather than editing the test.
- wiremock 0.6 cannot express a usable oversized-body fixture at all
  (all of Step 2's fallbacks exhausted) — report which variants you
  tried.
- You find yourself wanting to change spawn signatures, backoff, or
  parser logic — out of scope.

## Maintenance notes

- Future fetch-posture changes (UA string, timeout, redirect policy,
  cap semantics) now have exactly one home: `net.rs`. A reviewer seeing
  a `reqwest::Client::builder()` reappear in a poller should push it
  back into `net.rs`.
- If a third remote source lands (e.g. a future enrichment call), start
  from `build_poll_client()` — but note its 10 s timeout and UA were
  chosen for polling; an interactive path may want its own builder.
- Deferred deliberately: bundling the pollers' 8-arg spawn signatures
  into a params struct (recorded tech-debt at `rss_poller.rs:439-440`);
  wiring a full `fetch_league` wiremock test (its URL is hardcoded to
  the real host — parameterizing the base URL for tests was judged not
  worth the plumbing while `read_body_capped` covers the risky part).
- **Plan 037 (engine propagation, filed 2026-07-18) hard-depends on
  this plan landing first** — its dependency gate checks this plan's
  `plans/README.md` row reads DONE. 037 later migrates the pollers'
  enqueue/emit tails and spawn signatures onto an `Engine` handle; it
  does not touch `net.rs` or the fetch paths this plan creates, so no
  coordination beyond ordering is needed.
