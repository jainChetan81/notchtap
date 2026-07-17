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
