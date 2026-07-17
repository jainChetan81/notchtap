# Plan 003: Uptime Kuma → notchtap webhook recipe (docs only, no source changes)

> **Executor instructions**: Follow this plan step by step. This plan
> produces exactly one new file (a recipe doc) and touches no application
> source code, no Rust, no frontend. If anything in the "STOP conditions"
> section occurs, stop and report — do not improvise. When done, update the
> status row for this plan in `plans/README.md` — unless a reviewer
> dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat efa1bd2..HEAD -- src-tauri/src/http.rs docs/IMPLEMENTATION_PLAN.md`
> If `http.rs`'s `/notify` request contract changed since this plan was
> written (field names, required-ness, response codes), compare the
> "Current state" excerpts below against the live code before proceeding; on
> a mismatch, treat it as a STOP condition — the recipe doc must describe
> the endpoint's *actual* current contract, not this plan's snapshot of it.
> A non-empty diff by itself is not automatically a STOP: `http.rs` churns
> for reasons unrelated to `/notify` (e.g. the per-source priority/TTL
> defaulting added in commit `efa1bd2`, see "Current state" below). Only
> STOP if the diff actually changes the request shape, required fields, the
> content-type check, or the response codes described below.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction (integration recipe — already fully scoped in `docs/IMPLEMENTATION_PLAN.md` §8)
- **Planned at**: commit `efa1bd2`, 2026-07-17 (refreshed by `/improve
  review-plan` against `efa1bd2`, same day — the original `d40445e` snapshot
  had already drifted: `http.rs` gained per-source priority/TTL defaults
  between the two commits, moving several of the line numbers below. See
  "Current state" for the corrected excerpts.)

## Why this matters

`docs/IMPLEMENTATION_PLAN.md` §8 ("future integration idea — kuma alert
relay") already did the design work and concluded this is "the one idea
judged genuinely worth keeping, not yet built": Uptime Kuma (already
monitoring several services on the same machine per that doc) has a
built-in "webhook" notification provider with a custom JSON body template.
Pointed at notchtap's existing, unmodified `/notify` endpoint, a Kuma
monitor going down surfaces directly as a notchtap overlay — **zero
application code changes**, because the `/notify` endpoint already accepts
exactly the `{title, body}` shape Kuma's template can produce.

This plan's deliverable is **a short recipe document plus one manual
verification pass** — not a code plan. `docs/IMPLEMENTATION_PLAN.md` §8
already recorded the real, unresolved caveats (loopback-only bind limits
this to same-machine Kuma instances; Kuma's own custom-webhook feature has
had template-substitution bugs in some versions; whether this is worth
doing at all depends on usage patterns the doc's author couldn't resolve
in advance). This plan does not re-decide any of that — it writes down the
concrete steps so that if the maintainer wants to try it, they don't have to
re-derive the Kuma-side configuration from scratch, and records what
"verified working" looks like.

## Current state

The exact, current `/notify` contract this recipe must describe accurately
(`src-tauri/src/http.rs`):

- **Route**: `POST /notify` (`http.rs:115`), bound to `127.0.0.1:<port>`
  only — never externally reachable (`http.rs:123-125`, `bind_listener`,
  hardcoded to `"127.0.0.1"`, "no config field can widen it" per the
  function's own doc comment and `ARCHITECTURE.md` §7).
- **Required header**: `content-type: application/json` — a request
  without this exact prefix is rejected with 400 before the body is even
  parsed (`http.rs:136-140`):
  ```rust
  if !content_type.starts_with("application/json") {
      return Err(HttpError::BadRequest("content-type must be application/json"));
  }
  ```
- **Request body shape** (`NotifyRequest`, `http.rs:98-111`):
  ```rust
  struct NotifyRequest {
      title: Option<String>,
      body: Option<String>,
      priority: Option<Priority>,   // "low" | "medium" | "high", optional
      #[serde(default)]
      signal: EventSignal,           // optional, defaults to "generic"
      source: Option<RequestSource>, // only valid value today: "cmux"
  }
  ```
  `title` and `body` are `Option<String>` in the struct but are enforced
  as **required** immediately after parsing (`http.rs:145-150`):
  ```rust
  let title = req.title.ok_or(HttpError::Event(EventError::MissingField("title")))?;
  let body = req.body.ok_or(HttpError::Event(EventError::MissingField("body")))?;
  ```
  A request missing either field gets a 400. `priority` and `signal` are
  genuinely optional in all cases. `source` is optional and, as of
  commit `efa1bd2`, now selects *which* fallback priority/TTL apply
  (`http.rs:152-159`):
  ```rust
  let (origin, default_priority, ttl_secs) = match req.source {
      Some(RequestSource::Cmux) => (SourceKind::Cmux, state.cmux_priority, state.cmux_ttl_secs),
      None => (SourceKind::Manual, state.manual_default_priority, state.default_ttl),
  };
  ```
  Kuma's webhook never sets `source` (only `"cmux"` is a valid wire
  value, and that's reserved for the `notchtap` CLI's own auto-detection —
  see "Out of scope" below), so a Kuma request always takes the `None`
  branch: `Config.manual_default_priority` (default `Medium`) and
  `Config.default_ttl` (default `8` seconds), same fallback this recipe
  described before `efa1bd2` added the `cmux`-specific branch. The new
  branch doesn't change anything Kuma-facing, but a reader diffing this
  file against `http.rs` should know it's there.
- **Response codes**: `200 OK` with `{"status": "accepted"}` on a normal
  accept; `202 Accepted` with `{"status": "paused", "queued": N}` when the
  app is currently paused (still buffers the event, per `CONTEXT.md`'s
  **Paused** definition); a `4xx` on malformed/missing-field requests; the
  underlying queue can also reject with a `QueueError` (e.g. tier full) —
  this recipe's manual verification step (Step 3) only needs to observe
  "some 2xx and the overlay shows the alert", not enumerate every failure
  mode.
- **What the CLI (`notchtap` shell script) itself sends** for comparison
  (`notchtap:99-105`) — Kuma will NOT use this script, it POSTs directly,
  but this is useful for confirming the minimal valid shape:
  ```sh
  payload=$(jq -n --arg title "$title" --arg body "$body" \
    --arg priority "$priority" --arg signal "$signal" --arg source "$source" \
    '{title: $title, body: $body}
     + (if $priority != "" then {priority: $priority} else {} end)
     + (if $signal != "" then {signal: $signal} else {} end)
     + (if $source != "" then {source: $source} else {} end)')
  ```
  i.e. the minimal valid payload is exactly `{"title": "...", "body":
  "..."}` — which is what this recipe's Kuma template produces.

`docs/IMPLEMENTATION_PLAN.md` §8's exact prior findings (lines 809-851,
already read in full during recon) that this recipe doc must not
contradict or silently drop:

- Kuma's webhook body template: `{"title": "{{name}}", "body": "{{msg}}"}`.
- **Loopback-only constraint**: this only works if Kuma and notchtap run on
  the **same machine**. The doc's context: Kuma runs on the mac mini in the
  user's setup; reaching a notchtap instance on a different machine (e.g.
  the macbook) over the tailnet would require reopening the loopback-only
  decision, which is explicitly out of scope, "not proposed here."
- **Known Kuma bugs**: custom-body webhook template-variable substitution
  has had bugs in some Kuma versions (the doc cites github issues #3635 and
  #4861 on `louislam/uptime-kuma`) — "needs a manual smoke test before
  trusting it, not assumed to work first try."
- **Open, unresolved question the doc explicitly leaves unresolved**:
  whether this is worth building/configuring at all depends on how often
  the user is actually looking at the mac mini's own screen when a monitor
  fires — if it's mostly headless for him, Kuma's existing Telegram alert
  already covers the same signal and this adds nothing. **This plan does
  not resolve that question either** — it only lowers the cost of finding
  out, by writing down the concrete steps once.

## Commands you will need

This plan needs no build/test commands — it produces one new markdown file
and (optionally) one small addition to an existing doc. The only "command"
is the manual verification in Step 3, which is a real HTTP request against
a running notchtap instance, not a repo build/test command.

| Purpose | Command | Expected on success |
|---|---|---|
| Confirm current `/notify` contract before writing the doc | `sed -n '86,200p' src-tauri/src/http.rs` (from repo root) | matches the "Current state" excerpts above (covers the route registration, `NotifyRequest`, the content-type check, required-field enforcement, and the response codes — a narrower range will silently skip most of what needs checking); if not, STOP per the drift-check note |
| Manual smoke test (Step 3, requires a running notchtap + Kuma instance — cannot be run by an executor with no such instance available; document as a checklist for the operator instead if unavailable) | `curl --request POST http://127.0.0.1:9789/notify --header 'content-type: application/json' --data '{"title": "Test Monitor", "body": "Test Monitor went down"}'` | `200 OK`, and the notchtap overlay shows a card with that title/body within ~1 second |

## Scope

**In scope** (the only files you should create or modify):
- A new file: `docs/recipes/kuma-webhook.md` (create the `docs/recipes/`
  directory if it doesn't exist — confirm with `ls docs/` first). As of
  this plan's last verification, `docs/` has no existing convention this
  would duplicate: `docs/review-logs/` holds only dated retrospective
  audit/review logs (`YYYY-MM-DD-topic.md`, e.g.
  `2026-07-17-v5-settings-review.md`), not forward-looking how-to content,
  and `docs/plans/` (one file, an unrelated kimi delegation plan) and the
  empty `docs/review/` don't fit either — none of these are a "recipe"
  genre. Still run `ls docs/` yourself in case a real convention was added
  since; don't just take this note's word for it.
- Optionally, one short update to `docs/IMPLEMENTATION_PLAN.md` §8 (the
  existing kuma section) adding a single line pointing at the new recipe
  doc once written, e.g. "step-by-step recipe: `docs/recipes/kuma-webhook.md`
  (written <date>)." Do not rewrite or restructure §8 itself — it is
  already a settled, well-reasoned analysis; this plan only adds a
  pointer to newly-written how-to content, it does not re-argue the
  analysis.

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/http.rs` or any other source file — the `/notify`
  endpoint needs zero changes for this recipe to work; it already accepts
  the minimal `{title, body}` shape Kuma's webhook template produces. If
  you find yourself wanting to add a Kuma-specific field, endpoint, or
  special-case, STOP — that is scope creep beyond "write down how to
  configure the existing endpoint."
- `docs/ARCHITECTURE.md` §7 (the loopback-only decision) — do not propose
  loosening this to reach a different machine. The recipe must state
  plainly that this only works when Kuma and notchtap share a machine, per
  the existing decision.
- Any code or config that would let notchtap poll or query Kuma — the
  integration direction is strictly Kuma-pushes-to-notchtap, matching
  notchtap's own receive-only design philosophy (`CONTEXT.md`'s
  **Relay** definition — "an external tool forwarding its own
  notifications into notchtap" — describes Kuma's role here; notchtap is
  the receiver, not an actor reaching out to Kuma).
- Kuma's own installation/configuration outside of the one webhook
  notification provider entry being described — this recipe assumes an
  already-running Kuma instance and does not cover installing or
  administering Kuma itself.

## Git workflow

- Branch: `advisor/003-kuma-webhook-recipe`
- Single commit: `docs: kuma webhook recipe — point an existing monitor at /notify`
  (matches the repo's terse, colon-prefixed `docs:` commit style — see
  `git log --oneline` entries like `docs: v5 news landed — testing counts,
  plan §4.6, agents state`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Confirm the `/notify` contract hasn't drifted

Run the drift-check command from the header of this plan. Read the live
`src-tauri/src/http.rs` lines 86-200 (the response-code branching this
plan describes runs through line 199, past the old 56-180 range) and
confirm the request shape,
required fields, and response codes match this plan's "Current state"
section. If they match, proceed. If they don't, this is a STOP condition —
report the discrepancy rather than writing a recipe doc that describes a
contract the code no longer has.

**Verify**: your own read of `http.rs` matches the excerpts above (no
command output to check — this is a manual read-and-compare step).

### Step 2: Write `docs/recipes/kuma-webhook.md`

Check `ls docs/` first for an existing convention (per Scope above). Then
write the file with this content (adjust only if Step 1 found a drifted
contract — in that case, STOP instead of silently adjusting):

```markdown
# recipe: uptime kuma monitor alerts → notchtap overlay

routes an uptime kuma monitor's up/down alert through notchtap's existing
`/notify` endpoint, so a monitor going down shows up as a notch/hud overlay
card — no new notchtap code, no new service, one kuma notification config
entry.

status: **not yet manually verified end-to-end** (`IMPLEMENTATION_PLAN.md`
§8) — kuma's custom-webhook template substitution has known bugs in some
versions (github issues #3635, #4861 on `louislam/uptime-kuma`); smoke-test
before relying on this for anything you actually care about noticing.

## constraint: same machine only

notchtap's `/notify` endpoint binds `127.0.0.1` only, by design
(`ARCHITECTURE.md` §7) — this is not configurable. **kuma must run on the
same machine as the notchtap instance you want alerts to reach.** if kuma
runs on a different machine (e.g. reaching a macbook's notchtap from a mac
mini's kuma over the tailnet), this recipe does not apply — that would
require reopening the loopback-only decision, which is a real scope change,
not a config tweak.

## setup

1. in kuma, go to **Settings → Notifications → Setup Notification**.
2. notification type: **Webhook**.
3. **Webhook URL**: `http://127.0.0.1:<port>/notify` — `<port>` matches
   notchtap's configured `port` (default `9789`; check the settings window's
   General section, "Listener port", if it's been changed from the
   default).
4. **Content Type**: `application/json` — required; notchtap's `/notify`
   rejects any other content-type with a 400.
5. **Request Body**: choose the "Custom Body" / template option (exact
   label depends on your kuma version) and set it to:
   ```json
   {"title": "{{name}}", "body": "{{msg}}"}
   ```
   `{{name}}` and `{{msg}}` are kuma's own template variables for the
   monitor's name and the alert message — kuma substitutes them before
   sending.
6. attach this notification to whichever monitor(s) you want relayed, save.

this sends the minimal valid `/notify` payload — no `priority`, `signal`,
or `source` field, so the event falls back to notchtap's configured
`manual_default_priority` (default `Medium`) and `default_ttl` (default `8`
seconds), same as any other unadorned manual push. if you want kuma alerts
to promote at a different priority or stay visible longer, there is no
kuma-side way to set that per-request today (the webhook template can't
express notchtap's `priority`/`signal` fields without kuma exposing more
variables than `{{name}}`/`{{msg}}`) — the only lever available is
notchtap's own manual-priority/TTL settings, which apply to *all*
unlabeled manual pushes, not just kuma's.

## manual verification (do this before trusting it)

1. confirm notchtap is running and reachable:
   ```sh
   curl --request POST http://127.0.0.1:9789/notify \
     --header 'content-type: application/json' \
     --data '{"title": "Test Monitor", "body": "Test Monitor went down"}'
   ```
   expect a `200` (or `202` if the app happens to be paused) and the
   overlay showing a card titled "Test Monitor" within about a second. if
   this doesn't work, the kuma-side config below has nothing to build on —
   fix this first.
2. in kuma, use the notification provider's own "Test" button (if your
   kuma version has one) to fire a real webhook through kuma's own
   templating, not the curl command above. confirm the same overlay card
   appears with kuma's actual monitor name/message substituted in — this is
   the step that specifically exercises kuma's known template-substitution
   bugs (#3635, #4861). if the title/body come through as the literal
   strings `{{name}}`/`{{msg}}` instead of substituted values, that's the
   bug — check your kuma version against the linked issues.
3. optionally, actually take a monitored service down briefly and confirm
   a real down-alert reaches the overlay end-to-end.

## known limits (not fixed by this recipe, by design)

- **same machine only** (see above) — this is `ARCHITECTURE.md` §7's
  decision, not an oversight.
- **no per-alert priority/rotation control** — see the setup section above.
- **loopback-only is not an auth boundary** — any process on the same
  machine can already post to `/notify` (`ARCHITECTURE.md` §7's own
  documented scope note); this recipe doesn't change or worsen that, it's
  an existing, accepted property of the endpoint.
- **whether this is worth configuring at all** is genuinely unresolved
  (`IMPLEMENTATION_PLAN.md` §8): if you're rarely looking at this machine's
  screen when a monitor fires, kuma's own telegram/other alert channels
  already cover the same signal and this recipe adds nothing beyond a
  redundant notification path.
```

**Verify**: the file exists at `docs/recipes/kuma-webhook.md` (or the
equivalent path if Step 2 found and matched an existing doc-location
convention instead), and `cat docs/recipes/kuma-webhook.md | head -5` shows
the expected title line.

### Step 3: Manual smoke test (operator action, not the executor's to complete alone)

If you (the executor) have access to a running notchtap instance and an
Uptime Kuma instance on the same machine, perform the "manual verification"
steps written into the recipe doc itself, and update the doc's "status"
line from "not yet manually verified end-to-end" to a dated confirmation,
e.g. "verified working YYYY-MM-DD: kuma vX.Y.Z, test-button webhook
correctly substituted `{{name}}`/`{{msg}}`."

If you do **not** have access to a running Kuma instance (likely, in an
isolated executor environment), do not fabricate a verification result.
Leave the doc's status line as "not yet manually verified end-to-end" and
say so plainly in your final report — this is expected, not a failure of
this plan. The curl-only half of Step 1's verification (confirming
`/notify` itself works) is the part you can and should actually run if a
notchtap instance is reachable in your environment; the kuma-specific half
requires a real Kuma install this plan does not provision.

### Step 4 (optional): Add one pointer line in `IMPLEMENTATION_PLAN.md` §8

If you completed Steps 1-2, optionally add one line at the end of §8's
existing text (after "no timeline, no owner, not blocking any phase above —
recorded here so the idea isn't lost, same treatment as §2.4's posture
module.") — do not edit anything before that sentence:

```
step-by-step recipe written: `docs/recipes/kuma-webhook.md`.
```

**Verify**: `grep -n "kuma-webhook.md" docs/IMPLEMENTATION_PLAN.md` shows
the new line, and `git diff docs/IMPLEMENTATION_PLAN.md` shows only this
one-line addition, nothing else changed in that file.

## Test plan

This plan has no automated test — it's a documentation deliverable. The
closest thing to a test is Step 3's manual verification, which is
explicitly optional/best-effort depending on the executor's environment
(see Step 3's own text). There is no vitest/cargo test coverage to add;
do not invent one (e.g. do not add a test that merely checks the markdown
file exists — that provides no real signal and isn't this repo's
convention for documentation changes).

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `docs/recipes/kuma-webhook.md` (or the equivalent matched-convention
      path) exists and contains the setup steps, the same-machine
      constraint, the manual verification steps, and the known-limits
      section
- [ ] The recipe's described `/notify` request shape
      (`{"title": ..., "body": ...}`, `content-type: application/json`)
      matches the live `src-tauri/src/http.rs` contract as of Step 1's
      confirmation
- [ ] No source code file (`.rs`, `.ts`, `.tsx`) is modified
      (`git status --short` shows only new/modified files under `docs/`)
- [ ] If Step 4 was done: `git diff docs/IMPLEMENTATION_PLAN.md` shows
      exactly one added line, nothing else
- [ ] `plans/README.md` status row for `003` updated to `DONE` (or
      `DONE — kuma-side verification not run, see doc's status line` if
      Step 3's manual Kuma test could not be performed)

## STOP conditions

Stop and report back (do not improvise) if:

- Step 1 finds the live `/notify` contract has drifted from this plan's
  "Current state" section (different required fields, different response
  codes, a changed content-type check, etc.) — rewrite the recipe to match
  the *live* contract only after confirming with a human that the drift is
  intentional and not itself a regression worth its own bug report.
- You find an existing `docs/` convention for short how-to/recipe content
  that this plan's proposed `docs/recipes/` path would duplicate or
  conflict with — use the existing convention instead and note the path
  change in your final report.
- Any step tempts you to modify `src-tauri/src/http.rs`, `config.rs`, or
  any other source file "to make the kuma integration nicer" (e.g. adding
  a `source: "kuma"` variant, or a dedicated `/notify/kuma` route) — this
  is explicitly out of scope; the entire point of this idea (per
  `IMPLEMENTATION_PLAN.md` §8) is that the existing endpoint already
  suffices. Report the temptation as a *separate* future finding instead of
  acting on it here.

## Maintenance notes

- This recipe describes kuma's webhook template using kuma's own
  `{{name}}`/`{{msg}}` variables — if a future kuma version renames or
  changes these template variables, the recipe doc will silently go stale.
  No code depends on this doc, so nothing will fail loudly; a periodic
  "does this recipe doc still match kuma's current UI" check is a manual
  maintenance task, not an automated one.
- If the `/notify` request contract ever changes (a new required field, a
  renamed field, a different content-type requirement), this recipe doc
  must be updated in the same change — grep for `docs/recipes/kuma-
  webhook.md` from any future `http.rs` change touching `NotifyRequest`.
- The doc's own "not yet manually verified" / "verified working" status
  line is the single source of truth for whether Step 3 has actually been
  done — keep it accurate; don't let it silently go stale to "verified"
  after a kuma upgrade that might have reintroduced the substitution bugs
  the recipe warns about.
</content>
