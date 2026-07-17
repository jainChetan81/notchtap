# notchtap — v3 technical spec (v0 draft)

> Archived 2026-07-17 — v3 shipped and is done. Moved here from `docs/`
> alongside the v1/v2 specs once v3.6/v5 superseded them as the active
> working drafts. Not updated inline below to preserve the record as-written
> — file paths inside this document still read `docs/...`, meaning the
> repo-root `docs/` folder, not this `docs/archive/` subfolder.

operationalizes `IMPLEMENTATION_PLAN.md` §3 into code-level specifics.
like the v1/v2 specs, this is a working draft — adjust freely as
implementation surfaces friction; if a change is a *decision* change,
it goes to `ARCHITECTURE.md` / `IMPLEMENTATION_PLAN.md` §3 instead.
decisions here were locked in a grilling session 2026-07-16.

---

## 0. scope

one new capability: accepted events fan out to outbound connectors
(telegram in v3), so notifications reach the user away from the mac.

- **in**: the notifier seam, one telegram connector, secrets file,
  per-event-type message templates, retry/drop rules.
- **not in v3**: whatsapp/twilio (demoted — re-evaluate later),
  per-connector event filtering, presence/away gating, replying back
  into any tool (still locked out per `ARCHITECTURE.md` §7), settings
  ui of any kind.

---

## 1. the seam

```
/notify → validate → queue.enqueue ──ok──► overlay path (unchanged:
        │                    │             queue → promotion → webview)
        │                    └──ok──► for each connector: try_send(event)
        └──reject (400/413/429): nothing fans out
```

- fan-out happens in the http handler (and any future internal event
  source) **after** `enqueue` returns `Ok`. promotion is irrelevant to
  connectors; paused overlay still fans out.
- the Notifier seam is **outbound-only** — the overlay is not a
  member of it. (a seam, not a rust trait — `ConnectorHandle` is the
  code-level shape until a second connector earns an abstraction;
  see `CONTEXT.md`.) the queue's result alone decides the http status.
- new module `src-tauri/src/notifier.rs` owns everything below.

## 2. types

```rust
/// spawned once per enabled connector at startup.
pub struct ConnectorHandle {
    name: &'static str,               // "telegram"
    tx: tokio::sync::mpsc::Sender<Event>, // bounded, CHANNEL_CAP = 64
}

impl ConnectorHandle {
    /// called at acceptance. never blocks: try_send; on a full
    /// channel, drop the event and tracing::warn!.
    pub fn offer(&self, event: &Event);
}

pub enum RetryDecision {
    RetryAfter(Duration),   // transient: network error / 5xx / timeout
    ResendPlain,            // 400: formatting rejected — strip to plain text
    Drop,                   // second failure, or after ResendPlain fails
}

/// pure — unit-tested. attempt is 0-based. `retry_after` is the delay
/// a RetryAfter decision carries; the worker sleeps exactly the carried
/// value (signature gained the param in the 2026-07-16 review — the
/// draft's version returned a const the worker didn't actually use).
pub fn on_send_failure(attempt: u32, kind: FailureKind, retry_after: Duration) -> RetryDecision;
```

worker loop (thin, untested): `rx.recv()` → format → send with 10s
reqwest timeout → on failure consult `on_send_failure` → at most one
retry (~5s later) or one plain-text resend → drop and warn.

## 3. telegram connector

- endpoint: `POST https://api.telegram.org/bot{token}/sendMessage`
  with json `{ "chat_id": ..., "text": ..., "parse_mode": "HTML" }`
  (`parse_mode` omitted entirely for a plain-text fallback resend).
- **html mode, not markdownv2**: only `<`, `>`, `&` need escaping
  (`fn escape_html`), vs markdownv2's 18 characters. generic events
  carry arbitrary agent/cli text — smallest escaping surface wins.
- per-event-type templates (`fn format_message(event) -> String`,
  pure, unit-tested — data-not-code, mirroring the frontend's
  animation table):

  | event type    | template                          |
  |---------------|-----------------------------------|
  | `score_update`| `⚽ <b>{title}</b>\n{body}`       |
  | `match_state` | `🕐 <b>{title}</b>\n{body}`       |
  | `generic`     | `🤖 <b>{title}</b>\n{body}`       |
  | unknown       | `<b>{title}</b>\n{body}` (generic rule, same as css) |

  title/body are html-escaped before substitution.

## 4. config & secrets

`config.toml` gains (non-secret):

```toml
[connectors.telegram]
enabled = false        # default off — v3 is opt-in per machine
```

new file `~/.config/notchtap/secrets.toml` (never committed, never
pasted, not in `config.toml` so config stays paste-safe):

```toml
[telegram]
bot_token = "..."
chat_id = "..."
```

load rules, checked at startup:
- file missing, unreadable, or perms not `0600` → warn + connector
  disabled; app runs overlay-only. same-build-everywhere philosophy:
  a machine without secrets just has no outbound.
- `enabled = true` with valid secrets → spawn the worker.
- env vars are **not** consulted — login-item launches don't inherit
  shell env (`SMAppService`), so an env-var path would silently break.

## 5. testing crosswalk (`TESTING_STRATEGY.md` §4.9)

tdd'd first (pure): `format_message` per type + a nasty-characters
escaping case (`<b>`, `&`, underscores, backticks in body);
`on_send_failure` all arms; `offer` drop-on-full.

wiremock (integration): worker send path — 200 ok; 400 → exactly one
plain-text resend (assert `parse_mode` absent); 5xx → exactly one
retry then drop. no live telegram call in any test or ci run, ever.

http fan-out (in `http.rs` tests): accepted push lands in a test
connector channel; 429-rejected push does not; paused (`202`) push
**does** (acceptance succeeded).

manual (`IMPLEMENTATION_PLAN.md` §6): one real telegram message end to
end on the mac mini; app healthy with secrets absent.

## 6. open questions — resolved during implementation (2026-07-16)

- **fan-out call sites**: `offer` is called from both acceptance
  sites — `http.rs` (after `enqueue` ok) and the espn poller via
  `poller::enqueue_and_fan_out` (its testable enqueue helper). the
  draft deferred this "until a second call site exists," but the
  poller already was one — caught in the v3 review when score events
  couldn't reach telegram. not hoisted into `event::dispatch`
  (dispatch validates, it doesn't own acceptance).
- **`FailureKind` mapping**, pinned with the wiremock cases: any
  reqwest transport error (timeout / connect / dns) → `Transient`;
  5xx → `Transient`; 400 → `BadRequest`; every other status →
  `Fatal` (401/403/404 are config-level — retrying can't help).
