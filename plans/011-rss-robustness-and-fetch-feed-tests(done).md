# Plan 011: RSS robustness — characterize `fetch_feed`, bound the entity decoder, stream the size cap

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> Steps are ordered tests-first on purpose — do not reorder. If anything
> in "STOP conditions" occurs, stop and report. When done, update this
> plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat b43a7ca..HEAD -- src-tauri/src/rss_poller.rs`
> On any change, compare excerpts below; mismatch = STOP.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none (010 touches a different file's client; no conflict)
- **Category**: bug / tests
- **Planned at**: commit `d40445e`, 2026-07-17; drift baseline refreshed to `b43a7ca` 2026-07-18 (excerpts re-verified unchanged)

## Why this matters

Three related weaknesses in the RSS path, which parses untrusted
third-party XML 24/7:

1. **`fetch_feed` has four real decisions and zero tests.** It decides:
   304 → skip; non-200 → bail; read etag/last-modified but persist them
   ONLY after a successful parse (the in-code comment names the bug this
   ordering prevents: "storing them on a failure path would make the next
   poll 304 and silently never retry" — regress it and a feed blacks out
   forever, silently); and a two-stage 1 MiB cap. `wiremock` is already a
   dev-dependency (used by `notifier.rs` tests) — the tests are cheap.
2. **`decode_entities` is O(n²) on hostile input.** For every `&`, it
   rescans the remainder of the string looking for `;`. A ~1 MB summary
   of mostly `&` with no `;` costs on the order of 10¹¹ comparisons — a
   compute stall on a tokio worker of an always-on app, repeated every
   poll while the entry stays in the feed. It also runs on the FULL
   entry text before the output truncation to 120/240 chars.
3. **The 1 MiB cap only applies after full buffering** when the server
   omits Content-Length (chunked responses): `response.bytes().await?`
   reads everything into memory first. A misbehaving feed can push far
   more than 1 MiB of transient allocation before rejection.

Order matters: the characterization tests (Step 1) land BEFORE the
`fetch_feed` rewrite (Step 3) so the validator-ordering behavior is pinned
by tests while the function is being edited.

## Current state

`src-tauri/src/rss_poller.rs:359-409` — `fetch_feed`:

```rust
async fn fetch_feed(
    client: &reqwest::Client,
    url: &str,
    state: &mut FeedState,
) -> anyhow::Result<Option<feed_rs::model::Feed>> {
    let mut request = client.get(url);
    if let Some(etag) = &state.etag {
        request = request.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = &state.last_modified {
        request = request.header(IF_MODIFIED_SINCE, last_modified);
    }

    let response = request.send().await?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }
    if response.status() != reqwest::StatusCode::OK {
        anyhow::bail!("unexpected http status {}", response.status());
    }

    // read validators now, but only persist them after a successful parse: …
    let etag = response.headers().get(reqwest::header::ETAG)…;
    let last_modified = response.headers().get(reqwest::header::LAST_MODIFIED)…;

    if response
        .content_length()
        .is_some_and(|length| length > MAX_FEED_BYTES as u64)
    {
        anyhow::bail!("response body exceeds 1 MiB");
    }
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_FEED_BYTES {
        anyhow::bail!("response body exceeds 1 MiB");
    }

    let feed = feed_rs::parser::parse(&bytes[..])?;
    state.etag = etag;
    state.last_modified = last_modified;
    Ok(Some(feed))
}
```

`src-tauri/src/rss_poller.rs:164-186` — `decode_entities`:

```rust
fn decode_entities(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '&' {
            if let Some(relative_end) = chars[index + 1..].iter().position(|ch| *ch == ';') {
                let end = index + 1 + relative_end;
                let entity: String = chars[index + 1..end].iter().collect();
                if let Some(decoded) = decode_entity(&entity) {
                    out.push(decoded);
                    index = end + 1;
                    continue;
                }
            }
        }
        out.push(chars[index]);
        index += 1;
    }

    out
}
```

`sanitize` (lines ~188+) calls `decode_entities(&strip_html_tags(text))`
on the full field, then collapses whitespace and truncates to `max_chars`
(`TITLE_MAX_CHARS = 120`, `BODY_MAX_CHARS = 240`).
`MAX_FEED_BYTES: usize = 1024 * 1024` (line 20).

Test conventions in this file: `#[cfg(test)] mod tests` at the bottom,
21 existing tests, pure functions tested against fixtures including
"real-shaped ndtv captures". For async wiremock tests, model on
`src-tauri/src/notifier.rs`'s `send_path` module (wiremock `MockServer`,
`#[tokio::test]`). Counts live ONLY in `docs/TESTING_STRATEGY.md` §0.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Module tests | `cargo test rss_poller::` (from `src-tauri/`) | all pass |
| Full suite | `cargo test` (from `src-tauri/`) | all pass |
| Gates | `cargo clippy --all-targets -- -D warnings && cargo fmt --check` (from `src-tauri/`) | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/rss_poller.rs` (fetch_feed, decode_entities, sanitize,
  tests)
- `docs/TESTING_STRATEGY.md` §0 count + §4.12's "thin fetch" boundary
  sentence (it currently claims fetch mechanics are out-of-test-scope;
  after Step 1 that's no longer true — update the sentence)
- `plans/README.md` (status row)

**Out of scope**:
- `poller.rs` (ESPN — plan 010), `SeenStore`, `diff_feed`, category
  derivation, the spawn loop.
- Changing `MAX_FEED_BYTES`, TITLE/BODY limits, or any config field.
- feed-rs version.

## Git workflow

- Current branch; two commits preferred:
  1. `rss_poller: wiremock characterization for fetch_feed (304, validator ordering, size cap)`
  2. `rss_poller: bound entity-decoder lookahead, stream the size cap, pre-truncate sanitize input`
- Do NOT push.

## Steps

### Step 1 (tests first): wiremock characterization of `fetch_feed`

Add an async test sub-module (e.g. `mod fetch_feed_tests`) using wiremock.
`fetch_feed` takes `&reqwest::Client`, `&str` url, `&mut FeedState` — all
directly constructible. Cases:

1. `not_modified_returns_none_and_preserves_state` — mock returns 304;
   assert `Ok(None)` and `state.etag`/`last_modified` unchanged.
2. `validators_not_persisted_on_parse_failure` — mock returns 200 with
   ETag/Last-Modified headers and an UNPARSEABLE body (`b"not xml"`);
   assert `Err(..)` and state validators still `None`. **This is the
   bug-guard test** — the permanent-304-blackout regression.
3. `validators_persisted_on_success` — 200 + headers + a minimal valid
   RSS body (reuse/borrow the smallest existing fixture in this file's
   tests; a hand-written 5-line `<rss><channel><item>…` string is fine);
   assert `Ok(Some(_))` and both validators stored.
4. `oversized_content_length_rejected` — 200 with
   `Content-Length: 2000000` (wiremock `insert_header`) and small body →
   assert Err mentioning "1 MiB". If wiremock normalizes Content-Length
   to the actual body size (it may), instead serve an actual >1 MiB body
   and assert the post-read rejection — either form pins the cap.
5. `conditional_headers_sent_when_state_has_validators` — set
   `state.etag`/`last_modified`, mock `.and(header(IF_NONE_MATCH, ...))`
   matcher returning 304; assert `Ok(None)` (proves the request carried
   the validators).

**Verify**: `cargo test rss_poller::` → all pass (these characterize CURRENT behavior — they must pass before any rewrite).

### Step 2: Bound the entity decoder + pre-truncate sanitize input

Two changes in one commit:

(a) In `decode_entities`, cap the `;` lookahead. The longest legal entity
this decoder handles (read `decode_entity` just below it) is a numeric
form like `#x10FFFF` — 9 chars is enough; use a named const:

```rust
const MAX_ENTITY_LEN: usize = 10;
// inside the loop:
let window_end = (index + 1 + MAX_ENTITY_LEN).min(chars.len());
if let Some(relative_end) = chars[index + 1..window_end].iter().position(|ch| *ch == ';') {
```

(rest unchanged — an `&` with no `;` within 10 chars is emitted literally,
same as an unknown entity today).

(b) In `sanitize`, truncate the input before the expensive passes: the
output is capped at `max_chars` anyway, and HTML stripping + entity
decoding can only shrink text, so operating on a bounded prefix is
behavior-identical for all real titles/summaries. Take a generous prefix
(e.g. `max_chars * 8` chars — entities are ≤10 input chars per output
char, tags add slack) with a comment explaining the bound:

```rust
fn sanitize(text: &str, max_chars: usize) -> String {
    // Output is truncated to max_chars below; stripping/decoding never
    // lengthens text, so a bounded prefix is behavior-identical and keeps
    // hostile multi-hundred-KB fields from costing full-length passes.
    let bounded: String = text.chars().take(max_chars * 8).collect();
    let decoded = decode_entities(&strip_html_tags(&bounded));
    …
```

Add unit tests (plain `#[test]`, pure functions):
- `ampersand_flood_without_semicolons_is_linear` — a 100 KB string of
  `'&'` runs through `sanitize(_, 240)` and returns quickly (assert on
  the output content — e.g. starts with `&&&` and has ≤240 chars — not on
  wall time; the bounded lookahead + prefix make it structurally linear).
- `entities_still_decode_at_boundaries` — `"&amp;"`, `"&#x1F600;"`-class
  cases if `decode_entity` supports them (read it first), plus a real
  boundary pair (not a mislabeled one — see below), and an
  existing-behavior case copied from the current tests to prove no
  regression.

  **On the boundary case, work out the arithmetic, don't eyeball it.**
  With `window_end = (index + 1 + MAX_ENTITY_LEN).min(chars.len())` and a
  search over `chars[index + 1..window_end]`, the slice holds exactly
  `MAX_ENTITY_LEN` characters — i.e. the entity text (between `&` and
  `;`, exclusive of both) can be **up to 10 chars long and still be
  found**; 11+ chars pushes the `;` outside the window. A naive "put `;`
  10 chars after `&`" case actually lands *inside* the window (found),
  the opposite of what "not decoded" implies — so don't write that case
  literally as described above; if you do, it will contradict the very
  code in Step 2(a) and you'll waste a cycle chasing a phantom bug.
  Also note: an unrecognized entity name is emitted literally regardless
  of whether it was found within the window or truncated away, so a case
  using gibberish text doesn't exercise the cutoff at all — you need a
  name `decode_entity` actually recognizes for the boundary to matter.
  Numeric entities tolerate leading zeros, so pad one to hit an exact
  length: `"&#x0001F600;"` (entity text `#x0001F600`, 10 chars) must
  still decode to `'\u{1F600}'` (the grinning-face emoji), while
  `"&#x00001F600;"` (entity text `#x00001F600`, 11 chars) must NOT decode
  and must emit the literal `&#x00001F600;` text unchanged — that pair is
  the one that actually fails if `MAX_ENTITY_LEN`'s bound regresses.

**Verify**: `cargo test rss_poller::` → all pass, including every pre-existing sanitize/fixture test (behavior-identical for real input is the requirement).

### Step 3: Stream the size cap

Rewrite the body read in `fetch_feed` to accumulate chunks and bail as
soon as the running total exceeds `MAX_FEED_BYTES`:

```rust
let mut response = response; // chunk() needs mut
let mut body: Vec<u8> = Vec::with_capacity(64 * 1024);
while let Some(chunk) = response.chunk().await? {
    if body.len() + chunk.len() > MAX_FEED_BYTES {
        anyhow::bail!("response body exceeds 1 MiB");
    }
    body.extend_from_slice(&chunk);
}
let feed = feed_rs::parser::parse(&body[..])?;
```

Keep the pre-read `content_length()` fast-reject. Step 1's tests must
still pass unchanged (same observable contract).

**Verify**: `cargo test rss_poller::` → all pass, especially test 4 (oversized) and test 3 (validators-on-success). Then full gates: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` → clean.

### Step 4: Docs

`docs/TESTING_STRATEGY.md` §0 (line ~19) is a single line: `225 tests —
settings 38, queue 47, http 26, notifier 23, rss_poller 21, poller 19,
event 17, config 17, presentation 11, lib (hotkey) 6`. Bump BOTH the
`rss_poller N` sub-count by the number of tests actually added in Steps 1
and 2, AND the leading total (`225`) by the same delta — the sub-counts
must keep summing to the total, since this line is the only place these
counts live (per this repo's `CLAUDE.md`).

In §4.12 (~line 530), replace the "conditional-GET … http mechanics
reuse §4.7's 'fetch loop stays thin' boundary" clause (it's the second
half of the "untested by design" bullet, after the shader/reduced-motion
clause — leave that first half alone) with one stating `fetch_feed`'s
decision surface (304 / validator ordering / size cap) is now
wiremock-tested, and only the spawn loop remains thin-by-design.

**Verify**: `cargo test` still green.

## Test plan

Summarized from steps: 5 wiremock `fetch_feed` cases (module
`fetch_feed_tests`, modeled on `notifier.rs`'s `send_path`), 2+ pure
sanitize/decoder cases. All pre-existing rss_poller tests (21) must pass
untouched — if one needs editing, that's a STOP condition (behavior
changed where it shouldn't).

## Done criteria

- [ ] `cargo test` exits 0; new tests present
      (`cargo test rss_poller:: 2>&1 | grep -oE '[0-9]+ passed' | head -1` →
      the number is ≥ 28. Do NOT use `grep -c "test result"` for this — that
      counts summary-line occurrences, one per compiled test binary, which
      is 2 today regardless of how many tests exist inside rss_poller, and
      will never read as a test count.)
- [ ] `grep -c "MAX_ENTITY_LEN" src-tauri/src/rss_poller.rs` → ≥2
- [ ] `grep -c "chunk()" src-tauri/src/rss_poller.rs` → ≥1
- [ ] `grep -c "response.bytes().await" src-tauri/src/rss_poller.rs` → 0
- [ ] clippy/fmt gates exit 0
- [ ] `docs/TESTING_STRATEGY.md` §0 + §4.12 updated — §0's `rss_poller N`
      sub-count and its leading total both moved by the same delta
- [ ] `plans/README.md` status row updated

## STOP conditions

- Any pre-existing rss_poller test fails after Step 2 or 3 — the changes
  were supposed to be behavior-identical for valid input; report the
  diff, do not adjust the old test.
- `decode_entity` supports an entity form longer than 10 chars (read it
  before Step 2) — adjust `MAX_ENTITY_LEN` to its real max +1 and note
  it; if its max is unbounded (it shouldn't be), STOP.
- wiremock cannot express a Content-Length larger than the served body
  AND serving a real >1 MiB body makes the test slow (>5 s) — report,
  keep the other 4 cases.

## Maintenance notes

- Reviewers: the validator-ordering test (case 2) is the load-bearing
  one — any future `fetch_feed` refactor must keep it green.
- If a future change raises `MAX_FEED_BYTES`, the streaming loop keeps
  memory bounded automatically; the `Vec::with_capacity` hint can stay.
- The `sanitize` prefix bound assumes output caps stay ≤ a few hundred
  chars; if a "full article text" feature ever lands, revisit the `* 8`
  factor.
