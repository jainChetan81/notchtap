# Plan 050 (spike): design a read-only `GET /status` HTTP route

> **Executor instructions**: This is a DESIGN SPIKE, not a build plan.
> The deliverable is a design document plus open questions for the
> maintainer — **zero production code changes**. Follow the steps, honor
> the STOP conditions, and when done update this plan's status row in
> `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f2cbae6..HEAD -- src-tauri/src/http.rs src-tauri/src/engine.rs src-tauri/src/status.rs`
> Drift doesn't block a spike — but read the drifted regions before
> quoting them in the design doc.

## Status

- **Priority**: P3
- **Effort**: S–M (coarse — investigation + design doc, no build)
- **Risk**: LOW (docs only)
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `f2cbae6`, 2026-07-19

## Why this matters

notchtap is deliberately headless-friendly — a local HTTP endpoint, a
CLI script, hook scripts for cmux and Claude Code, a documented Uptime
Kuma webhook recipe. But every one of those integration points is
write-only: there is no way for a script, a tmux status line, or a
monitoring dashboard to ask notchtap "what's currently showing / how
deep is the queue / is a football match live right now" without opening
the GUI overlay. The exact data this would answer already exists,
already computed, already `Serialize`-derived, and already emitted to
the frontend on every state change: `Engine::emit_current_status_blocking()`
returns a `StatusState` (paused, waiting count, per-source
enabled/live/ambient summary) that the overlay's idle rail renders. This
spike investigates what exposing that same value over a new `GET
/status` route would look like, and — more importantly — what it would
cost in terms of widening the app's network attack surface, since
`/notify` today is accept-only (a malicious or buggy caller can push
notifications, but cannot read anything back).

**This is deliberately a spike, not a build plan**, per this repo's own
`improve`-skill convention for direction findings: the underlying data
and serialization already exist and are already tested, so the *code*
would be a small addition — but "should a local HTTP server that is
currently write-only start answering reads" is a security-posture
decision for the maintainer to make deliberately, not something to ship
as a drive-by addition to an unrelated plan.

## Current state (grounding — quote-verified at `f2cbae6`)

- `src-tauri/src/http.rs:133-138` — the entire current route table:

  ```rust
  pub fn router<R: tauri::Runtime>(state: AppState<R>) -> Router {
      Router::new()
          .route("/notify", post(notify_handler::<R>))
          .layer(DefaultBodyLimit::max(64 * 1024))
          .with_state(state)
  }
  ```

  Exactly one route, `POST /notify`. No `GET` route exists anywhere in
  the router.

- `src-tauri/src/http.rs:140-145` — the trust model this spike must
  respect and extend, not silently loosen:

  ```rust
  /// Binds the listener. Loopback-only is a security boundary
  /// (`ARCHITECTURE.md` §7): this is the single place a bind happens,
  /// and it is hardcoded to 127.0.0.1 — no config field can widen it.
  pub async fn bind_listener(port: u16) -> std::io::Result<tokio::net::TcpListener> {
      tokio::net::TcpListener::bind(("127.0.0.1", port)).await
  }
  ```

  Read `docs/ARCHITECTURE.md` §7 in full before writing the doc — it's
  the existing decision record for why the HTTP surface is loopback-only
  and unauthenticated, and any new route inherits that same trust
  boundary (any local process/user on the machine can already reach
  `/notify`; a new `GET /status` would be reachable by the same set of
  callers, not a wider one — but it changes what those callers can
  *learn*, not just what they can *trigger*, which is the actual new
  risk this spike must evaluate).

- `src-tauri/src/engine.rs:326-342` — the data already computed and
  ready to serialize, `emit_current_status_blocking`:

  ```rust
  pub fn emit_current_status_blocking(&self) -> StatusState {
      let live_summary = self.live.lock().unwrap().clone();
      let weather_summary = self.weather.lock().unwrap().clone();
      let state = {
          let q = self.queue.blocking_lock();
          StatusState::snapshot(&q, /* ... */)
      };
      emit_status_state(&self.app, state.clone());
      state
  }
  ```

  This already returns an owned `StatusState` (`#[derive(Serialize)]`,
  `src-tauri/src/status.rs:19-27`) — a `GET /status` handler could call
  a `read`/`read_blocking`-style variant of this (note:
  `emit_current_status_blocking` also emits a `status-state` tauri
  event as a side effect, meant for the on-page-load reload path per its
  doc-comment — an HTTP handler calling it on every request would
  re-emit that event on every poll, which is probably not desired; the
  doc must address whether a new non-emitting read path is needed
  instead of reusing this method directly).

- `AppState<R>` (`src-tauri/src/http.rs:22-38`) already carries `pub
  engine: Engine<R>` — a new handler has everything it needs already in
  scope via the existing `State<AppState<R>>` extractor, same as
  `notify_handler`.

- What must NOT leak: confirm by reading `StatusState`/`FootballStatus`/
  `NewsStatus`/`WeatherStatus`/`LiveMatchSummary`/`WeatherSummary`
  (`src-tauri/src/status.rs`) field-by-field that none of them carry
  anything from `secrets.toml` (telegram bot token, openrouter key) —
  the doc should state this explicitly with a field-by-field check, not
  just assert it.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Read-only exploration | `grep`, `Read`, `rg` | — |
| Confirm the one existing route | `grep -n "\.route(" src-tauri/src/http.rs` | exactly one `/notify` line |
| Confirm `StatusState`'s full field tree carries no secrets | `Read src-tauri/src/status.rs` in full | manual review, cited in the doc |
| Confirm nothing changed | `git status` at the end | only the new doc + `plans/README.md` row |

## Scope

**In scope** (the only files you may create/modify):
- `docs/design/read-only-status-endpoint.md` (create)
- `plans/README.md` (status row)

**Out of scope — hard rule for this spike**:
- ANY file under `src/`, `src-tauri/`, or config/build files. No
  prototype route code in the repo; illustrative snippets live inside
  the doc.
- Rewriting `docs/ARCHITECTURE.md`/`docs/V3_6_TECHNICAL_SPEC.md` — the
  doc *proposes* what amendment either would need; it doesn't make the
  edit.
- Proposing or designing authentication/API-key schemes for `/notify`
  itself — out of scope; this spike is additive (a new read route), not
  a redesign of the existing write route's trust model.

## Git workflow

- Docs-only commit `docs(design): read-only status endpoint spike` in
  repo style. Do NOT push or open a PR unless the operator instructed
  it.

## Steps

### Step 1: Read the full trust-model precedent

Read `docs/ARCHITECTURE.md` §7 in full, and `http.rs`'s `bind_listener`
doc-comment. Understand precisely what today's threat model already
accepts (any local process can POST a notification) versus what a new
`GET` route would newly expose (any local process could learn the
current queue depth, whether football/weather/news pollers are enabled,
and the current live-match/weather summary — no secrets, per the
field-by-field check above, but still information that isn't available
today without opening the GUI).

### Step 2: Write the design doc

`docs/design/read-only-status-endpoint.md`, each section with a
**recommendation and at least one rejected alternative with reason**:

1. **Route shape**: `GET /status` returning `StatusState` as JSON
   (recommend reusing the existing `Serialize` derive directly, no new
   wire type) vs. a deliberately narrower response type that omits
   fields not meant for external consumption — decide and justify.
2. **Emission side-effect**: whether the handler should reuse
   `emit_current_status_blocking` as-is (re-emitting the `status-state`
   tauri event on every HTTP poll — likely undesirable, could cause
   redundant frontend re-renders under status-line polling) or whether
   `Engine` needs a new non-emitting `read`-based accessor
   (`engine.read(|q| StatusState::snapshot(...))`-shaped, no
   `emit_status_state` call) — recommend the latter with the concrete
   method signature, and note this is the one place actual (tiny,
   additive) production code would eventually be needed: a new
   `Engine` method, not a route-layer workaround.
3. **Auth/exposure**: given the loopback-only bind is unchanged, is a
   read-only endpoint's risk meaningfully different from today's
   write-only one? Consider: information disclosure to any local
   process (most machines are single-user, but browser-based
   JavaScript from any locally-loaded page could also reach
   `127.0.0.1` — evaluate whether this is a realistic vector worth
   naming, referencing how `/notify`'s existing CORS/fetch posture, if
   any, already handles or doesn't handle this for the write side).
4. **Response caching/staleness**: should the response be point-in-time
   fresh on every request (recommend yes — the underlying read is cheap,
   per `emit_current_status_blocking`'s own doc-comment "cheap — two
   queue reads + a clone") or is there a reason to rate-limit/cache?
5. **Consumers**: what's the first realistic consumer (a tmux/shell
   status line via `curl`, a menu-bar alternative, a monitoring
   dashboard) — the doc doesn't need a real one today, but should name
   the shape of use this unlocks, mirroring the Direction category's
   evidence-grounding rule (this is a `/improve next`-sourced finding,
   not a stated user request — say so plainly).
6. **Config surface**: does this need an opt-in flag (mirroring
   `espn_live_card`'s/`weather_enabled`'s precedent — "no widened
   surface without an explicit flag") or is a read-only status route
   low-risk enough to ship unconditionally once built? Recommend one
   with reasoning.
7. **Test strategy**: `axum`/`tower::ServiceExt::oneshot` in-process
   testing, same pattern as the existing `/notify` handler tests in
   `http.rs`'s test module — name the concrete cases (empty queue,
   paused, one visible item, football/weather live).
8. **Build estimate**: S/M with the file list the build would touch
   (`http.rs`'s router + one new handler, possibly a new
   non-emitting `Engine` read method).
9. **Open questions for the maintainer** (e.g.: is this actually wanted,
   or is opening the GUI an acceptable status-check cost; should a
   future route ever return anything beyond `StatusState`, like queue
   contents — explicitly recommend against that expansion unless asked,
   since queue contents could include in-flight notification bodies,
   a materially different disclosure risk than aggregate status).

### Step 3: Sanity-check citations

Every code claim gets a `file:line` valid at the commit read (stamped at
the top of the doc).

**Verify**: `git status` → only the design doc (+ `plans/README.md`
row).

## Test plan

N/A — docs-only spike.

## Done criteria

- [ ] `docs/design/read-only-status-endpoint.md` exists, covers all 9
      sections, each with recommendation + rejected alternative
- [ ] The doc states the commit it was researched against
- [ ] The doc includes an explicit field-by-field confirmation that
      `StatusState`'s full field tree carries no secret/credential data
- [ ] The auth/exposure section explicitly compares the new route's
      risk against today's write-only `/notify` risk, not just asserting
      "loopback-only, so it's fine"
- [ ] No source-code changes (`git status` proof)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- Any `StatusState`-reachable field turns out to carry secret/credential
  data (re-read `status.rs`, `poller.rs`, `weather_poller.rs` if
  unsure) — that would make this a security finding requiring rotation
  advice, not a feature spike; report it separately per this skill's
  Hard Rule 4, don't fold it quietly into the design doc.
- A `GET` route (status or otherwise) already exists in `http.rs` by the
  time this spike runs — re-grep `\.route(` first; if one exists, the
  premise ("zero read routes today") is stale and the doc should instead
  evaluate extending what's there.

## Maintenance notes

- If approved, the build plan should reuse `StatusState`'s existing
  `Serialize` derive verbatim (no new wire type) unless this doc's Step
  2.1 recommends otherwise, and must add the new non-emitting `Engine`
  read accessor from Step 2.2 rather than reusing
  `emit_current_status_blocking` directly.
- This is one of several direction findings from the same audit pass
  (see plans 052/053, if selected) that touch the `Engine`/`status.rs`
  surface — whoever builds any of them second should re-read the
  others' decisions first, since all three touch adjacent code.
