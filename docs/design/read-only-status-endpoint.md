# Design: a read-only `GET /status` HTTP route

**Status**: design spike (plan 050) — proposal only, zero code written.
**Researched against**: commit `f2cbae6` (2026-07-19), the commit plan 050
was planned at. Every `file:line` citation below was verified by reading
that file at that commit.

## Background and premise check

notchtap's HTTP surface today is exactly one route:

- `src-tauri/src/http.rs:133-138` — `router()` builds
  `Router::new().route("/notify", post(notify_handler::<R>))` plus a
  64 KiB `DefaultBodyLimit` layer. A grep for `\.route(` over
  `src-tauri/src/http.rs` returns exactly one hit (line 135). **No `GET`
  route exists**, so this spike's premise ("zero read routes today")
  holds.

The trust boundary that route lives behind:

- `src-tauri/src/http.rs:140-145` — `bind_listener` hardcodes
  `127.0.0.1`; its doc-comment states loopback-only is a security
  boundary and no config field can widen it.
- `docs/ARCHITECTURE.md:270-281` (§7) — the decision record: default
  port `127.0.0.1:9789`, loopback-only, and the explicit scope note that
  loopback "is not an authentication boundary between local processes —
  anything running on the machine can post notifications. that's
  acceptable by design for a single-user personal tool."

The data a status route would serve already exists, fully computed and
`Serialize`-derived:

- `src-tauri/src/status.rs:19-27` — `StatusState { paused, waiting,
  football, news, weather }` with `#[derive(Serialize)]`,
  `rename_all = "camelCase"`.
- `src-tauri/src/engine.rs:326-342` — `Engine::emit_current_status_blocking()`
  returns an owned `StatusState`, with the side effect of also emitting
  the `status-state` tauri event (`engine.rs:340`). Its doc-comment
  (`engine.rs:318-325`) frames it as the on-page-load reload twin of
  `emit_current_blocking`; its only production call site is the
  webview-reload path, `src-tauri/src/lib.rs:411`.
- `src-tauri/src/http.rs:22-38` — `AppState` already carries
  `pub engine: Engine<R>`, so a new handler needs nothing beyond the
  existing `State<AppState<R>>` extractor that `notify_handler`
  (`http.rs:147-151`) already uses.

### Field-by-field secret check (STOP-condition guard)

The plan's hard STOP was: if any `StatusState`-reachable field carries
secret/credential data, stop and report as a security finding. Read in
full (`src-tauri/src/status.rs:1-270`), the complete field tree is:

| Type | Fields | Contents |
|---|---|---|
| `StatusState` (`status.rs:21-27`) | `paused: bool`, `waiting: usize`, `football`, `news`, `weather` | queue pause flag, waiting count, three per-source summaries |
| `FootballStatus` (`status.rs:31-35`) | `enabled: bool`, `live: Option<LiveMatchSummary>` | boot-config gate, live match |
| `LiveMatchSummary` (`status.rs:39-44`) | `label: String`, `minute: String` | "Home X–Y Away" score text, ESPN clock text |
| `NewsStatus` (`status.rs:64-66`) | `enabled: bool` | boot-config gate only |
| `WeatherStatus` (`status.rs:73-76`) | `enabled: bool`, `current: Option<WeatherSummary>` | boot-config gate, current weather |
| `WeatherSummary` (`status.rs:54-57`) | `temp_display: String`, `condition: String` | display-formatted "27°", WMO-code word |

Every leaf is a `bool`, `usize`, or display `String` built from public
API responses (ESPN, RSS presence, Open-Meteo). The actual secrets —
telegram `bot_token` and openrouter `api_key` — live in `secrets.toml`
and are only referenced in `notifier.rs` (`notifier.rs:132`,
`settings.rs:312-313`); no type in the `StatusState` tree is
constructed from, or can reach, either. **No secret data is reachable.
The STOP condition does not fire.**

---

## 1. Route shape

**Recommendation: `GET /status` returning `StatusState` verbatim via its
existing `Serialize` derive — no new wire type.**

```rust
// illustrative only — lives here, not in the repo
.route("/status", get(status_handler::<R>))

async fn status_handler<R: tauri::Runtime>(
    State(state): State<AppState<R>>,
) -> Json<StatusState> {
    Json(state.engine.current_status().await) // see §2
}
```

The type is already the wire contract of the `status-state` tauri event
(`status.rs:15`, `STATUS_STATE_EVENT`), already camelCase
(`status.rs:20`), already round-trip-tested for exactly these JSON
shapes (`status.rs:178-207`). Reusing it means one definition of
"status" across the overlay event channel and the HTTP read; the serde
tests already in the tree become the HTTP contract tests' reference.

**Rejected: a deliberately narrower response DTO** (e.g. strip
`weather`, expose only `paused`/`waiting`/`football`). Reason: every
field in `StatusState` is already rendered in the overlay's idle rail —
nothing in it is internal-only — and the field-by-field check above
shows nothing sensitive. A second type buys no disclosure reduction,
doubles the serde surface to keep in sync, and invites drift between
what the GUI shows and what scripts can read.

## 2. Emission side-effect

**Recommendation: add a new non-emitting `Engine` accessor and have the
handler call it — do NOT reuse `emit_current_status_blocking`.**

`emit_current_status_blocking` (`engine.rs:326-342`) ends with
`emit_status_state(&self.app, state.clone())` (`engine.rs:340`). That
emit exists for the webview-reload path (`lib.rs:411`), where a freshly
reloaded page has no state and must be re-seeded unconditionally —
its doc-comment says exactly this (`engine.rs:318-325`). An HTTP
handler calling it would re-broadcast `status-state` to the frontend on
*every poll*: a tmux status line curling once a second would cause a
needless tauri event and frontend re-render every second. That's a
route caller creating GUI churn it has no business causing.

The concrete addition (the one place this design requires real,
additive production code — an `Engine` method, not a route-layer
workaround):

```rust
// illustrative only — new method on Engine, mirroring
// emit_current_status_blocking minus the emit:
pub async fn current_status(&self) -> StatusState {
    let live_summary = self.live.lock().unwrap().clone();
    let weather_summary = self.weather.lock().unwrap().clone();
    self.read(|q| {
        StatusState::snapshot(
            &q,
            live_summary,
            self.espn_enabled,
            self.rss_enabled,
            weather_summary,
            self.weather_enabled,
        )
    })
    .await
}
```

This composes from pieces that already exist: `Engine::read`
(`engine.rs:133-137`, "Non-propagating read — no wake, no emit"), the
`live`/`weather` handles and gate flags (`engine.rs:34-38`), and
`StatusState::snapshot` (`status.rs:82-105`). Lock discipline copies
`emit_current_status_blocking`'s own rule (`engine.rs:323-325`):
read/clone/drop the `StdMutex` handles before taking the queue lock.

**Rejected: reuse `emit_current_status_blocking` as-is.** Reason above:
the per-request `status-state` re-emit is redundant frontend churn
under any polling consumer, and it muddles the event's ownership — the
heartbeat loop is documented as the sole steady-state emitter
(`status.rs:108-111`).

## 3. Auth / exposure

**Recommendation: ship with no authentication, behind the unchanged
loopback bind — but document explicitly that this converts the HTTP
surface from write-only to read-capable, and that this is a deliberate
posture change, not a freebie.**

Comparison against today's write-only `/notify` risk:

- **Same caller set.** Loopback binding (`http.rs:143-144`) means any
  process running as any local user can already reach the server.
  `ARCHITECTURE.md:277-281` accepts that as-is for the write side.
  `GET /status` is reachable by exactly that set — it widens nothing
  network-wise.
- **Different capability.** Today a local caller can *trigger* (push a
  notification) but cannot *learn* anything — `/notify` responses carry
  only `{"status": "accepted"}` or `{"status": "paused", "queued": n}`
  (`http.rs:213-220`). A status route lets that same caller learn: queue
  depth, whether the overlay is paused, which pollers are enabled, the
  live football score being tracked, and current weather. Per the
  field-by-field check, none of it is secret, but it is information not
  obtainable today without opening the GUI. On the single-user machines
  this app targets (`ARCHITECTURE.md:279-281`), the marginal exposure
  is "another of the user's own processes can read what the user's
  overlay already shows" — low.
- **Browser vector, evaluated honestly.** JavaScript on any web page can
  *send* a `GET` to `http://127.0.0.1:9789/status`, but cannot *read*
  the response cross-origin unless the server opts in via CORS headers.
  The router installs no CORS layer — a grep for
  `CorsLayer|cors|tower_http` over `src-tauri/src` returns nothing; the
  only layer is `DefaultBodyLimit` (`http.rs:136`). So the same-origin
  policy already blocks the read side of the drive-by-browser vector,
  and since the route is read-only, the send-without-read remnant is
  harmless (no state change to CSRF). Worth one line in the eventual
  docs, not a mitigation.

**Rejected: token/API-key auth on the new route.** A shared secret
would have to live in every consumer (shell status lines, dashboards),
it protects data already visible on the user's own screen, and the
plan explicitly scopes redesigning the existing trust model out. If the
single-user assumption ever changes, the fix belongs at the bind/trust
layer for all routes, not bolted onto this one.

## 4. Response caching / staleness

**Recommendation: point-in-time fresh on every request; no caching, no
rate limiting.**

`StatusState::snapshot`'s own doc-comment calls the recompute "cheap
(two queue reads + a clone)" (`status.rs:79-81`) and notes the *event
channel* — not the computation — is what needs a change guard. The
same logic covers HTTP: the read is a couple of lock acquisitions and
small clones, and `waiting`/queue depth can change between any two
requests, so a cached response is precisely wrong for the "how deep is
the queue right now" question this route exists to answer. A
once-a-second status-line poller is trivially within budget.

**Rejected: response caching or rate limiting.** Caching serves stale
queue depth to the consumers most likely to poll frequently; rate
limiting defends against load the server cannot meaningfully feel.
Either adds moving parts to protect against a non-problem.

## 5. Consumers

Honest provenance first: this is a `/improve next`-sourced direction
finding, **not a stated user request** — no one has asked for this
endpoint. What it would unlock, in order of realism:

1. **A tmux / shell status line** — `curl -s localhost:9789/status`
  rendered into a prompt or tmux segment ("⏸ 3 queued", "⚽ ARS 2–0 CHE
  67'"). This is the natural first consumer: the app's existing
  integrations are all terminal-centric (CLI, cmux hooks), and the CLI
  precedent (port resolution `NOTCHTAP_PORT` → 9789,
  `ARCHITECTURE.md:265-268`) already gives scripts the address.
2. **A lightweight menu-bar or panel widget** that wants status without
  opening the overlay window.
3. **A monitoring dashboard** alongside the documented Uptime Kuma
  webhook flow — today notchtap can be *watched into* but not *read
  from*.

**Rejected: building the endpoint speculatively without naming this
shape.** The Direction category's evidence rule applies: if none of
these shapes is compelling to the maintainer, the correct outcome of
this spike is "don't build it" (see §9).

## 6. Config surface

**Recommendation: ship unconditionally once built — no opt-in flag.**

The repo's flag precedents gate *behavior that reaches outward or costs
something*: `weather_enabled` (`config.rs:65`, default `false` per the
test at `config.rs:433`) gates an outbound polling loop;
`espn_live_card` (`config.rs:30`) gates a display behavior. A read-only
status route reaches outward to no one — it sits behind the same
loopback boundary every existing route already trusts, serves aggregate
data already on screen, and costs one cheap read per request. A flag
would default-off a feature whose entire value is being available to ad
hoc scripts (a script author can't flip a GUI setting from the status
line they're trying to build), and default-on flags that gate nothing
meaningful are config clutter.

**Rejected: an opt-in `http_status_enabled` flag mirroring
`weather_enabled`.** The precedent those flags set is "no *widened
surface* without a flag" — but this route widens no surface: same bind,
same caller set, read-only, no secrets. The flag would protect against
the local-information-disclosure question already weighed and accepted
in §3.

If the maintainer disagrees on §3, the flag is the fallback — that's
exactly why §3 is a maintainer decision, not an executor one.

## 7. Test strategy

Same in-process pattern as the existing `/notify` tests: build the real
`router()` with a mock-runtime `AppState` (`http.rs:259-282`), drive it
with `tower::ServiceExt::oneshot` (`http.rs:257`, used at e.g.
`http.rs:310-316`), assert on status code + deserialized JSON. Concrete
cases:

- **empty queue, unpaused** → 200, `paused: false`, `waiting: 0`,
  `football.live: null`, `weather.current: null`.
- **paused queue** → 200, `paused: true` (mirroring
  `paused_post_returns_202_with_queued_count`'s setup at
  `http.rs:318-332`).
- **one visible item, items waiting** → `waiting` matches
  `total_waiting()`.
- **football live / weather present** → `live.label`/`minute` and
  `current.tempDisplay`/`condition` serialize camelCase, pinning the
  wire shape against `status.rs:178-207`'s expectations. Note: the
  `live`/`weather` handles are private `Arc`s created inside
  `Engine::new` (`engine.rs:34-35`, `engine.rs:77-78`), so these two
  cases need either a small test seam on `Engine` or setup through the
  poller path — flag for the build plan.
- **no event emitted** → the handler must not emit `status-state`
  (regression test for §2; e.g. listen on the mock app and assert
  silence across N requests).
- **`POST /status` rejected** → 405, mirroring
  `get_method_on_notify_is_rejected` (`http.rs:451-462`).

**Rejected: testing through a real bound listener.** The in-process
oneshot pattern is already the house style and covers routing,
serialization, and state without ports or flakes; the one real-bind
test that exists (`listener_binds_loopback_only`, `http.rs:414-421`)
already pins the boundary this route inherits unchanged.

## 8. Build estimate

**S.** Files the build would touch:

- `src-tauri/src/http.rs` — one `.route("/status", get(...))` line in
  `router()` (`http.rs:133-138`), one ~5-line handler, plus the §7
  tests in the existing test module.
- `src-tauri/src/engine.rs` — one new ~15-line non-emitting
  `current_status()` accessor (§2), plus a unit test.

No new dependencies (`Json` is already imported at `http.rs:7`), no
config changes (§6), no frontend changes. Docs amendment on approval:
`ARCHITECTURE.md` §7's route description and `V3_6_TECHNICAL_SPEC.md`'s
endpoint contract would each need a short addition — proposed here,
edited there only on approval.

## 9. Open questions for the maintainer

1. **Is this wanted at all?** This is an audit-sourced direction
  finding, not a user request (§5). Is opening the GUI an acceptable
  status-check cost, or is the tmux-status-line shape compelling enough
  to widen the HTTP surface from write-only to read-capable?
2. **Posture sign-off on §3.** Shipping a read route with no auth is
  this doc's recommendation, but "the server now answers reads" is the
  security-posture decision this spike exists to surface, not to make.
3. **Scope discipline going forward.** Should any future route ever
  return more than `StatusState` — e.g. queue *contents*? This doc
  explicitly recommends **against** that expansion unless separately
  requested: queue contents include in-flight notification bodies
  (titles, bodies, detail pairs from arbitrary hook input), a
  materially different disclosure class than aggregate status, and one
  the field-by-field secret check above does not cover.
4. **Test seam for live/weather state** (§7): acceptable to add a
  `#[cfg(test)]`-visible setter on `Engine`, or should those cases go
  through poller-level setup?

---

## Appendix: amendment pointers (proposed, not made)

- `docs/ARCHITECTURE.md` §7 (`ARCHITECTURE.md:270-281`): extend the
  endpoint paragraph — the server answers `GET /status` with the
  `StatusState` JSON; the "not an authentication boundary" note then
  covers reads as well as writes, which is the §3 decision to record.
- `docs/V3_6_TECHNICAL_SPEC.md`: add the `/status` request/response
  contract beside the `/notify` one, reusing the `StatusState` serde
  tests (`status.rs:178-207`) as the shape pin.
