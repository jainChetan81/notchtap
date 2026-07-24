# Code Review — mac-notification-nudge (pal_codereview, google/gemini-2.5-pro, 2026-07-24)

> Recovered transcript: the original log file for this run was saved as
> a truncated stub under a wrong name ("audio-transcription"); this file
> restores the actual findings from the session output. Findings were
> re-verified by the reviewing Claude session on 2026-07-24 against
> master `e94628c`; fix disposition is recorded in
> `plans/132-external-review-fix-batch.md`.

External validation via **google/gemini-2.5-pro** confirmed the internal
finding and surfaced one new issue (in `crests.rs`). Both independently
verified below.

## 🟠 HIGH — SSRF via unvalidated crest URL (new finding, independently verified)

**`src-tauri/src/crests.rs:103-119`, sourced from `src-tauri/src/poller.rs:1306`**

`try_fetch` calls `client.get(url).send()` where `url` is `team.logo`
taken verbatim from ESPN's live JSON API response (see
`poller.rs:399-400`, `poller.rs:1306`). There's no scheme or host
validation — an attacker-influenceable URL (via a compromised/MITM'd
ESPN feed, or DNS manipulation of the ESPN domain) could point this
fetch at `127.0.0.1` or an internal-network/cloud-metadata address.
Confirmed by tracing the actual call path in `poller.rs`.

- **Fix:** parse the URL and reject non-`http(s)` schemes; restrict
  host to ESPN's known CDN domain(s) before calling `client.get`.
- **Context/caveat:** the client is `reqwest`, response body is capped
  (`read_body_capped`, 256KB) and failures are swallowed — not usable
  for RCE or unbounded exfiltration, but usable as a blind
  internal-network probe (timing/response-code side channel) if the
  upstream feed is ever compromised or spoofed.

## 🟡 MEDIUM — UTF-8 truncation corruption in Claude hook

**`hooks/notchtap-claude-hook.sh:69`**

`cut -c1-200` is byte-oriented in this script's context and can split a
multi-byte UTF-8 character, producing invalid UTF-8 in the notification
body.

- **Fix:** truncate inside `jq` instead (`tostring | .[0:200]`) — jq
  slices codepoints, not bytes.

## 🟢 LOW — `set_appearance` lock/write race (internal finding, externally validated)

**`src-tauri/src/settings.rs:887-913`**

`set_appearance` releases the lock, writes the full config to disk,
then re-acquires the lock only to mutate `.appearance` in memory —
unlike the other settings commands' single read-modify-write. Two rapid
concurrent calls can interleave and leave a stale on-disk value. Impact
low (local IPC only, cosmetic float fields, atomic write means no
corruption — just staleness).

- **Fix:** hold a single lock guard across validate → mutate →
  write-to-disk → broadcast.

## 🟢 LOW (style) — deprioritized nitpicks

- `engine.rs:47` — manual `impl Clone` for `AmbientSlot<T>` vs derive.
- `http.rs:107` — micro-optimization fusing `ok_or_else` + truncate to
  skip one allocation.

## Coverage note

Gemini's `files_examined` metadata echoed back only the 10 files
already internally reviewed — `poller.rs`, `status.rs`, `event.rs`,
`logging.rs`, `weather_poller.rs`, and the frontend components were
swept via directory context, not deep-read by name. The SSRF chain
through `poller.rs:1306` was traced by the driving session, not flagged
by Gemini as a `poller.rs` finding. (The 2026-07-24 GPT-5.2 round —
sibling log in this directory — covered all 78 files by name.)

## Overall

No critical issues. Strong concurrency model via the `Engine` facade;
good security hardening elsewhere (loopback binding, `0600`/`0700`
perms, atomic writes).
