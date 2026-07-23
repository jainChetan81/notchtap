# mac-notification-nudge — architecture & decisions

macos only. no windows/linux target, ever. independent, clean-room
build — not a fork or clone of any specific third-party app; no
external branding, code, or assets are used.

---

## 0. phased scope

| | v1 | v2 | v3 |
|---|---|---|---|
| core engine + notification queue | yes | — | — |
| cli push (manual + cmux-relayed) | yes | + push to other sources | — |
| animation | one generic template | per-event-type variety | — |
| live football scores (espn public api) | — | yes | — |
| posture module (airpods motion, optional) | — | optional | — |
| outbound connectors (telegram first) | — | — | yes |
| notch overlay / mac mini hud | yes (both machines from day one) | — | — |

v1 is deliberately thin: engine + queue + one animation + cli push,
wired to cmux so claude code (and anything else cmux watches) flows
straight in. everything else stacks on top without touching the core.

---

## 1. what we're building (v1)

a background utility that:

- runs a notification queue engine, permanently, as a menu-bar/notch app
- accepts pushes from the command line — either invoked directly, or
  relayed automatically through cmux's notification hook (§7)
- renders each push as a notch-anchored overlay on the macbook, and an
  equivalent floating hud on the mac mini (no notch) — same build, both
  machines
- shows one animation template for v1 (see §4) — variety comes in v2

---

## 2. system architecture

```
┌───────────────────────────────────────────────────────────┐
│                      v1 input sources                       │
│   direct cli invocation      │   cmux notification command   │
│   (manual / any script)      │   (auto-relays cmux's own      │
│                               │    desktop notifications,      │
│                               │    incl. claude code / copilot │
│                               │    "agent needs input")        │
└───────────────┬───────────────────────────┬─────────────────┘
                │                           │
                ▼                           ▼
        ┌───────────────────────────────────────────┐
        │           core engine (rust)                │
        │  - event bus (typed, v1 has one "generic"   │
        │    type; v2 adds score/goal/posture)        │
        │  - notification queue (fifo, max n           │
        │    concurrent, ttl per item)                  │
        │  - dispatch router                            │
        └───────┬─────────────────────────────────────┘
                │
                ▼
    ┌───────────────────────────────┐
    │      presentation ui             │
    │  (react/ts window)               │
    │  - notch mode (macbook)          │
    │  - hud mode (mac mini)           │
    │  - animation engine (1 template  │
    │    in v1, table-driven in v2)    │
    └───────────────────────────────┘
```

v3 adds a connectors layer at event *acceptance* (telegram first) —
deliberately not drawn here, it's out of scope until v3.

the same core (rust) and ui (react/ts webview) run unmodified on both
machines. only one module differs: **window placement** — notch-aware
on the macbook, plain top-center hud on the mac mini. detect at runtime
via `NSScreen.main?.safeAreaInsets.top > 0` (native call, thin swift
shim either way) and switch presentation mode automatically.

---

## 3. notification queue design

each event enqueues as:

```
{ id, type, priority, ttl, payload }
```

- **concurrency**: cap visible items (e.g. 3), render as a stack; excess
  wait in queue.
- **lifecycle per item**: `enter → hold → exit`.
- **pause** (from the tray, §6): pause disables *promotion* only. pushes
  are still accepted into the waiting queue — the http response tells
  the caller so (`202` + `{"status": "paused", "queued": <n>}`, vs the
  normal `200`), and `max_queued`/`429` still applies — and
  already-visible items finish their natural ttl and exit. resume
  re-enables promotion immediately; nothing buffered is dropped. pause
  state is in-memory only — the app always launches unpaused.
  **(amended 2026-07-17, v5, §17)**: the *toggle* stays session-only,
  but a persisted `start_paused` config flag can make the app launch
  already paused — the master kill switch. same paused semantics,
  only the launch state changes.
- v1 is info-only — auto-dismiss after ttl, no approve/deny action wired
  to anything. an interactive "this blocks until you respond" model is
  a v2+ concern if true remote-approve is ever wanted (see §7 note on
  cmux's limits).

**default values (v1)**:

| parameter | default | notes |
|---|---|---|
| `ttl` | `8` seconds | time from enter-complete to exit-start |
| `max_concurrent` | `3` | visible stack items; excess wait in queue |
| `max_queued` | `50` | hard cap on waiting items; new pushes return `429` when exceeded |
| `enter_duration` | `300` ms | animation in |
| `exit_duration` | `300` ms | animation out |
| `queue_overflow` | reject with `429` | prevents unbounded memory growth if the ui is stuck |

**superseded 2026-07-17 (v3.6, locked via grilling session, see
`IMPLEMENTATION_PLAN.md` §3.6 and `V3_6_TECHNICAL_SPEC.md`)**: the
whole "cap-3 visible stack" model above is retired, not just tuned.
`max_concurrent` is gone entirely — exactly one item is ever visible
(the **Slot**, see `CONTEXT.md`) — and promotion is priority-ordered
(`low | medium | high`) rather than pure fifo, though a priority
arrival still never interrupts the currently-visible item mid-display.
`ttl` is renamed **rotation**, and gains a `Recurring` kind that
requeues instead of dropping. `max_queued` becomes
`max_queued_per_tier`, applied independently per priority tier (a
`Low` burst can't starve `High`'s own waiting room). this table is
kept as the v1 historical record; `V3_6_TECHNICAL_SPEC.md` §3/§4 is
the current design.

---

## 4. animation system

v1: **one** generic template — enter/hold/exit via **css keyframes**, no per-type branching.
gets the pipe working end to end before investing in variety.

v2: swap the single template for a config table (event type →
animation), e.g. goal = confetti + bounce, posture-alert = shake,
generic/cmux = simple slide. **css keyframes** (locked — framer motion
was the alternative, evaluated and declined 2026-07-16 in favour of
zero new dependencies, see §16) keeps this a config change, not a new
code path.

**reversed 2026-07-17**: the css-keyframes lock above is reopened by
the user — the ui is migrating wholesale to **framer motion + lucide
icons** (migration in flight in the working tree as of this note, not
yet specced in docs). the data-not-code principle survives the stack
change: per-event-type variety stays a table, it just moves from the
stylesheet into the component layer. the migration gets its own
spec/plan entry when it stabilizes; §16's addendum records the
decision trail.

**reduce-motion fallback (decided 2026-07-18, plan 023).** the two
signature moments — the goal confetti burst + overshoot + ring
(`.rail-card.pulse-goal`) and the red-card strobe (`.rail-card.pulse-red`)
— are **suppressed entirely under `prefers-reduced-motion: reduce`**:
the fallback is *deliberately nothing*, not a static substitute. the
card, its priority accent, stamp, and copy still render and still
announce via the `aria-live` region; only the celebratory animation is
withheld. this is implemented purely in the stylesheet
(`@media (prefers-reduced-motion: reduce) { … animation: none }` in
`src/styles.css`, mirrored in `src/settings/preview-overlay.css`), which
is why the goal celebration is authored as CSS (`::after`/`::before`
pseudo-elements) rather than a JS-driven lottie player — a css rule can
be turned off by the media query, an autoplaying player cannot. the
`motion` components elsewhere are covered separately by
`<MotionConfig reducedMotion="user">` in `App.tsx`; these two plain-CSS
pulses need (and have) their own override.

---

## 5. cross-device behaviour

| | macbook (has notch) | mac mini (no notch) |
|---|---|---|
| window anchor | pinned over notch cutout, using `NSScreen.auxiliaryTopLeftArea`/`auxiliaryTopRightArea` | top-center floating hud, always visible (no cutout to hide behind) |
| detection | `safeAreaInsets.top > 0` at runtime | same check, fails → hud mode |
| everything else | identical — same queue, same animation, same cli input | identical |

this native screen-geometry call is the one piece of appkit that can't
be avoided regardless of stack choice — budget for a small
swift/objective-c shim here even if the rest is rust + web.

**integration pattern**: the swift code is compiled as a tiny standalone
cli tool (`notchtap-detect`) that prints json to stdout and exits.
the rust core calls it via `std::process::command` and parses the output.
this avoids ffi complexity entirely, keeps the swift boundary isolated
and testable, and lets the rust core stay a plain tauri/rust binary.

---

## 6. always-on background behaviour

standard pattern, v1 day one:

- `LSUIElement = true` in the bundle's info.plist → no dock icon,
  menu-bar presence only
- register as a login item via `SMAppService.mainApp.register()`
  (current macos api; requires **macos 13+**)
- menu-bar tray icon (tauri's tray api) with two always-present items:
  **pause** (semantics in §3 — label toggles to "resume" while paused)
  and **quit**, plus — only when `espn_enabled = true` — **pause
  football scores** (v2): stops the espn poller from issuing new
  network fetches (takes effect at the next poll tick), independent of
  the promotion pause; resuming re-baselines silently (no burst of
  stale score alerts). like pause, it's in-memory only — polling
  always starts active on launch. the original "exactly two items"
  decision was reopened and approved 2026-07-16 to admit this one
  conditional third item; the bar for further tray items stays high.
  **(reopened again 2026-07-17, v5, §17)**: a fourth item,
  **settings…**, opens the settings window — the tray itself stays
  minimal; anything richer than a toggle belongs in that window, not
  in more tray items. the "no settings ui beyond that — the config
  file (§10) is the settings surface" line that used to close this
  bullet is superseded by §17: the config file remains the *storage*,
  the settings window becomes the *editing surface*. tauri's default
  icon is fine until the deferred real-icon
  item (`IMPLEMENTATION_PLAN.md` §5)
- **always-on-top** (`setAlwaysOnTop` / `NSWindowLevel.floating`) — v1
day one. a notification overlay buried under other windows is useless.
- **transparent overlay window** — the main window is undecorated,
transparent, shadowless, non-resizable, and never takes focus. on macos
a transparent tauri webview requires `macOSPrivateApi: true` in
`tauri.conf.json`; that private-api use is the accepted cost of the
overlay look (added 2026-07-16, post-implementation review — it was in
the code but undocumented). app-store distribution is already ruled out
by §9, so the private-api restriction has no practical bite.

**minimum macos version**: v1 targets **macos 13 (ventura)** and later.
`SMAppService.mainApp.register()` is unavailable on macos 12 (monterey)
and earlier, which would require the legacy `SMLoginItemSetEnabled`
approach. if either target machine runs macos 12 or older, adjust the
login-item registration method before building.

**posture module (future, not v2)**: a real, shipping approach for this
exists using `CMHeadphoneMotionManager` (apple's public coremotion
api) to read airpods motion data, ~60 samples/sec, fully on-device,
inside app sandbox. if picked up later it's a clean addition: a new
event source feeding the same v1 queue, no rework.

### 6.1 hover primitive (plan 087)

the overlay window sits at `NSStatusWindowLevel`, flush over the real
menu bar (not just a notch cutout's dead zone) — `apply_overlay_native_config`
(`src-tauri/src/lib.rs`) calls `window.set_ignore_cursor_events(true)`
unconditionally there to stop it swallowing clicks meant for other
apps' menu-bar tray icons (the 2026-07-17 bug). plan 086's spike
(`docs/design/hover-cursor-tracking.md`) found, empirically, that a
`tauri-nspanel` tracking area's mouseEntered/mouseMoved/mouseExited
still fire normally under `ignoresMouseEvents = true` — click dispatch
and tracking-area notifications are gated by independent AppKit
mechanisms. plan 087 built on that: a tracking area on the same
`OverlayPanel`, a pure rust rect-derivation function
(`src-tauri/src/hover.rs`), and a `hover-changed` event to the webview —
with zero change to `set_ignore_cursor_events` or
`capabilities/default.json`. see the spike doc for the rationale and
the rejected alternative (a frontend-reported-bounds invoke command).

---

## 7. cli push — v1's actual notification source

**direct**: any script or terminal command calls the local cli/socket
directly. this is the baseline — always available, zero dependencies.

**cli contract (locked)**: flags only, one form for humans and scripts
alike — `notchtap --title <t> --body <b> [--subtitle <s>] [--detail Label=Value]... [--port <p>]`.
no positional form. as of plan 035, `--subtitle` is a first-class
*optional* wire field — it is **no longer folded** into the body — and
the `/notify` schema also accepts optional `details: [{label, value}]`
pairs (each repeatable `--detail Label=Value` on the cli, split on the
first `=`). both are display-only, capped/truncated server-side for the
fixed overlay window, and never influence priority/rotation. existing
callers are unaffected: `subtitle` was always optional and `details`
defaults to absent, so an old `{title, body}` payload behaves
byte-identically. port resolution: `--port` flag → `$NOTCHTAP_PORT` env var →
`9789`. the cli is a shell script (`jq` + `curl`, see the technical
spec §12) and never reads `config.toml` — if the server's port is ever
changed in config, set `$NOTCHTAP_PORT` to match on that machine.

**default port**: `127.0.0.1:9789`. the `/notify` endpoint binds to
loopback only, no external exposure. this port is unassigned in the iana
registry and unlikely to collide with common local services. if the port
is in use at startup, the rust core should exit with a clear error rather
than silently falling back to an arbitrary port — the user can override
via the config file.

one scope note on that boundary: loopback-only prevents *network*
exposure, but it is not an authentication boundary between local
processes — anything running on the machine can post notifications.
that's acceptable by design for a single-user personal tool; revisit
only if that assumption ever changes.

**cmux relay:**

cmux (the terminal running claude code) already has the integration
point for this, documented at cmux.com/docs/notifications:

- **settings > app > notification command** — a shell command cmux
  runs on *every* notification it fires (including its own claude code
  / copilot cli / opencode "agent needs input" alerts), with
  `CMUX_NOTIFICATION_TITLE`, `CMUX_NOTIFICATION_SUBTITLE`,
  `CMUX_NOTIFICATION_BODY` as env vars.
- point that one setting at the engine's cli entrypoint: `notchtap`
  `--title "$CMUX_NOTIFICATION_TITLE" --subtitle "$CMUX_NOTIFICATION_SUBTITLE"`
  `--body "$CMUX_NOTIFICATION_BODY"`. cmux already does the work of
  hooking into claude code (and copilot cli, and opencode) — no
  `PreToolUse`/`PermissionRequest` hook is written at all, just
  consuming cmux's existing output. this is also why it
  generalizes for v2 ("push to different stuff"): anything cmux already
  notifies on flows through this one setting, no per-tool integration
  work required.

the `notchtap` cli self-identifies a cmux-relayed push when the
`CMUX_NOTIFICATION_BODY` environment variable is present, adding
`source: "cmux"` on the wire. cmux priority and rotation seconds are
configured independently in settings.

**precise about the limit, so v1 scope stays honest**: cmux's
notification command is a **heads-up relay**, not an approval gate. it
fires *after* cmux decides to show a notification — it doesn't hand
back a way to answer into claude code's permission prompt from this
app's ui. that "click approve here, claude code proceeds" loop is a
separate, harder problem (it needs claude code's own
`PreToolUse`/`PermissionRequest` hook to actually block and wait for a
decision — see code.claude.com/docs/en/hooks if wanted later). for v1,
this isn't needed — the goal is just knowing claude needs input, via
one cli command. that's fully solved by the notification-command relay
alone.

**optional richer hook sources (plan 035)**: two committed hook scripts
in `hooks/` give the overlay real structure beyond cmux's three plain
strings. `hooks/notchtap-cmux-hook.sh` is a cmux *notification hook*
(the stdin-JSON form, distinct from the notification-command above): it
echoes cmux's json back **unchanged** first — so cmux's own
banner/history/sound behaviour is untouched — then relays
`title`/`body`/`subtitle` plus the workspace `cwd` as a `Project`
detail. `hooks/notchtap-claude-hook.sh` is a claude code hook that maps
`Notification`, `PermissionRequest` (both real, documented events;
`tool_name`/`tool_input`/`cwd` become Tool / Command-or-File / Project
detail cells), `Stop`, and `PostToolUse`(`Task`). both are
**observational only**: they post to the cli in the background and exit
0 without writing any decision to stdout — and per the hooks contract,
staying silent means "no decision", so claude code's real permission
prompt still shows. this is the same heads-up boundary as the relay
above: the respond-back loop (clicking approve here to unblock the
agent) stays out of scope and unchanged. the operator wires these in
`~/.claude/settings.json` and cmux's `notifications.hooks`; each script
does nothing and exits 0 when the `notchtap` cli is not on PATH, so it
can never block a session.

**v3**: outbound connectors sit here as additional sinks observing
accepted events — telegram first (bot api: free, instant, no approval
process). the earlier "whatsapp via twilio recommended" preference was
reopened and reversed 2026-07-16: twilio's sandbox needs a 72h re-join
and meta's template rules block freeform alerts — wrong fit for an
always-on personal notifier. whatsapp is "maybe later", re-evaluated
only if telegram proves insufficient. decisions in
`IMPLEMENTATION_PLAN.md` §3; contract was `archive/V3_TECHNICAL_SPEC.md`
(v3 shipped; removed at repo close-out 2026-07-23,
see `git log -- docs/archive/`).

---

## 8. tech stack recommendation

**constraint that changes the usual calculus**: both target machines
are macos. cross-platform reach — electron's and tauri's headline
justification — isn't actually needed here.

**recommendation: tauri (rust core + react/ts ui), with a small native
swift shim for notch geometry + window-level click-through/always-on-top
flags.**

reasoning:

- **footprint/perf** — tauri sits far closer to native (tens of mb,
  low idle cpu) than electron (hundreds of mb baseline, a full chromium +
  node runtime per app). matters because this runs 24/7 in the
  background on a mac mini presumably doing other things too.
- **developer experience (dx)/ecosystem fit** — the queue/animation/ui layer is the strongest
  surface (react, ts, css). rust handles polling/ipc/cli dispatch —
  bounded, real practice in a language already being learned.
- **pure native swift** wins on every technical axis except current
  fluency and iteration speed — right call for a long-term polished
  product, not for something wanted working soon.

net: tauri ships fast in a familiar stack, stays light enough to run
forever in the background, gives real rust practice. the one
unavoidable native surface (notch bounds + window flags) is small and
isolated.

---

## 9. distribution / install

the apple developer program is $99/yr. for personal use — the user's
own two machines, nobody else installing it — it's likely not needed
at all:

- the ios 7-day reinstall pain some prior personal projects hit is
  specific to **ios on-device provisioning profiles** issued to a free
  apple id ("personal team"). apple caps those at 7 days for sideloaded
  iphone/ipad apps — an ios sideloading limitation, not a macos one.
- **macos doesn't have that limit.** a mac app built locally (via
  xcode or tauri's build) and run on the machine that built it, never
  downloaded through a browser, isn't subject to ios-style expiry at
  all. a free apple id can still sign the app (ad-hoc/personal-team
  signing); gatekeeper's notarization check specifically targets files
  carrying the "downloaded from the internet" quarantine flag
  (`com.apple.quarantine`), which a locally built binary never gets.
- **moving between the macbook and mac mini**: build from source on
  each machine (git clone + build locally), or copy the built `.app`
  via a method that doesn't set the quarantine flag (local network
  share, usb, `scp`). if it ever does get flagged, one command clears
  it: `xattr -cr YourApp.app`. no recurring cost, no recurring
  reinstall.

**when the $99/yr would actually be needed:**
- distributing the built app to other people (so gatekeeper doesn't
  warn *them*)
- publishing on the mac app store
- a small number of advanced entitlements/services that specifically
  require a paid team

none of those apply to "runs on my macbook and my mac mini, built by
me." skip the fee unless this is ever handed to someone else.

**app store is the wrong channel regardless of cost** — sandboxing
blocks the persistent overlay + (any future) accessibility-based
automation this app needs, and review tends to reject apps that mimic
system chrome or automate other applications.

---

## 10. configuration & settings

v1 needs a minimal config file. suggested location:
`~/.config/notchtap/config.toml` (or json, whatever is easier with the
chosen rust deserializer).

**v1 fields**:

| field | default | description |
|---|---|---|
| `port` | `9789` | local http listener port |
| `default_ttl` | `8` | seconds per notification |
| `max_concurrent` | `3` | visible stack items |
| `max_queued` | `50` | waiting items before rejecting |
| `detect_path` | `/usr/local/bin/notchtap-detect` | absolute path to the notch-detection helper — gui/login-item-launched apps get a minimal `PATH`, so the core never does a `PATH` lookup (added 2026-07-16, consensus review) |

v2 adds (locked 2026-07-16, §16): `espn_enabled` (default `true`),
`espn_leagues` (default `["eng.1", "uefa.champions", "esp.1"]`),
`espn_poll_secs` (default `30`). posture module remains future, not
v2. api keys / bot tokens (telegram, etc.) live in a separate secrets
file (`secrets.toml`, see `archive/V3_TECHNICAL_SPEC.md` §4 — v3
shipped; removed at repo close-out 2026-07-23,
see `git log -- docs/archive/` for the removed text — not env vars;
login items don't inherit shell env) — never in the committed config.

v3.6 (locked 2026-07-17, see `V3_6_TECHNICAL_SPEC.md` §4.6) removes
`max_concurrent` outright (no longer meaningful — see §3's addendum
above) and renames `max_queued` to `max_queued_per_tier` (same default,
`50`, now applied independently per priority tier rather than as one
shared cap).

plan 083 adds `~/.config/notchtap/crests/` — a sibling of
`config.toml`/`secrets.toml` under the same directory — as the repo's
first binary-asset cache: club crest PNGs, fetched at runtime from
ESPN's scoreboard-provided logo URLs and never committed to git.
Lifecycle: fetched once per team per process lifetime on a cache miss,
persists across restarts, no eviction (bounded by the watched leagues'
team counts). Served to the overlay webview via tauri's asset protocol,
scoped to this directory only.

the rust core reads this file once at startup. changes require a restart
in v1; a file-watcher or settings ui is a v2+ convenience. **(resolved
2026-07-17, v5, §17)**: the settings ui is that convenience — a
settings window whose save path validates, writes this file
atomically, and relaunches the app. the read-once-at-boot rule is
unchanged and now permanent: there is no file-watcher and no
hot-reload; restart *is* the reload mechanism. **(2026-07-18, plan
013)**: boot now runs the loaded config through the settings window's
`validate()` and logs each violation as a warning, continuing with the
file's values rather than exiting — malformed TOML still fails fast in
`Config::load`.

---

## 11. logging & observability

- **rust core**: use `tracing` (already pulled in by tauri/axum). write
to a rotating log file at `~/Library/Logs/notchtap/notchtap.log`
(`src-tauri/src/logging.rs`). rotate at 10 mb, keep 3
backups. log level `info` in release, `debug` in dev.
- **frontend errors**: superseded 2026-07-17: never built; the overlay
is receive-only (§14/§17), so frontend errors are devtools-only by
design — no tauri command carries them back to the log file, and none
should be added.
- **macos console**: optionally bridge `tracing` events to `os_log` via
a small adapter, but file logs are the primary source of truth.

this is a background app — when something breaks, the user needs a log
to read. set this up in v1, not as an afterthought.

---

## 12. multi-display edge case

the notch exists only on the built-in macbook display. if the user has
an external monitor and the menu bar is on the external display, the
notification should still appear on the screen that has the notch (the
built-in one) in notch mode, and on the primary screen in hud mode.

v1 behavior: use tauri's default screen (the one containing the menu
bar). this is acceptable for v1. v2 may query `NSScreen.screens` via
the swift shim to find the notch-bearing display explicitly.

---

## 13. deduplication

v1 has **no deduplication** — if cmux fires the same "agent needs input"
notification twice in rapid succession, or a script loops with the same
message, the queue will contain duplicates. this is acceptable for a
personal tool with trusted, local sources.

if duplicate spam becomes a problem in practice, v2 can add a
`(title, body)` hash deduplication window (e.g., 5 seconds): identical
content within the window is silently dropped. this is tracked but not
implemented until the pain is real.

---

## 14. ipc security model

the frontend is **untrusted code running in a webview** — even though
it's first-party. the rust core treats it as a display-only consumer:

- frontend **receives** events via tauri `emit` / `listen`
- frontend **does not invoke** commands back into rust in v1
- the tauri capabilities file should reflect this: listen-only event
  permissions (`core:event:allow-listen`/`allow-unlisten`, not the
  `core:event:default` set, which would also grant emit), no
  filesystem, no shell, no network from the frontend
- the webview csp restricts `connect-src` to tauri's ipc endpoints
  only — the frontend cannot `fetch()` the local `/notify` endpoint
  (or anything else). a `csp: null` config would leave that network
  path open even with locked-down capabilities (added 2026-07-16,
  post-implementation review)

**amended 2026-07-17 (v5, §17)**: everything above now describes the
*overlay* window (`main`), where it stays permanent — that window
never gains an invoke command. the v5 settings window (`settings`) is
a second, separately-scoped webview with exactly four invoke commands,
gated per-window through the capability acl (which for app-defined
commands requires the explicit `build.rs` opt-in — see
`V5_TECHNICAL_SPEC.md` §2; without it tauri allows app commands to
every window, which would silently void this section). one-way data
flow into the overlay remains the rule that keeps v3-style untrusted
content safe.

**amended 2026-07-18**: the settings window's invoke-command count grew
past the four quoted above — v5.1 (`efa1bd2`) added `send_test_notification`
and `set_appearance` (six), and plan 020 (`9774930`) added
`get_default_config` — seven invoke commands total as of that date. the
mechanism described above (the `build.rs` opt-in gating a per-window
acl) is unchanged; only the count grew. `src-tauri/build.rs` and
`V5_TECHNICAL_SPEC.md` §2 remain the authoritative list.

**amended 2026-07-22**: as of 2026-07-22 the count is eleven (history
×2, connector-health, log-lines added); `src-tauri/build.rs` is the
authority.

this boundary matters if the app ever processes untrusted content (e.g.,
whatsapp messages from unknown senders in v3). establishing the
one-way data flow in v1 means v3 doesn't accidentally open a hole.

---

## 15. status

all decisions above are locked — scope, stack, distribution model,
cross-device behaviour. see `IMPLEMENTATION_PLAN.md` in this folder for
the phased build sequence and exit criteria.

---

## 16. v2 decisions (locked 2026-07-16)

- **leagues**: premier league (`eng.1`), champions league
  (`uefa.champions`), la liga (`esp.1`) — the default `espn_leagues`
  config list (§10); changing leagues is a config edit, never code.
- **trigger scope**: every scoreboard delta espn reports — score
  changes (goals), match-state transitions (kickoff, half-time,
  full-time), and card/situation events where the payload actually
  carries them. chosen over "goals only" with eyes open about noise;
  if it proves too chatty, narrowing is a filter/config change, not a
  redesign.
- **animation library**: css keyframes, no framer motion (resolves
  §4's deferred evaluation) — zero new dependencies; the "config
  table" is the stylesheet keyed by event type. **reversed 2026-07-17
  by the user**: the ui is migrating to framer motion + lucide icons
  (see §4's addendum). the zero-new-dependencies rationale is
  consciously traded for the richer ui; this line is kept as the
  historical record, not the current decision.
- **v2 absorbs three hardening fixes** from the 2026-07-16
  implementation consensus review (frontend wall-clock deadline
  recheck against sleep/timer-throttle staleness; `app_handle.exit(1)`
  instead of `process::exit` in the server task; a runtime-thread
  guard before the tray's `blocking_lock`). notch-precise positioning
  stays deferred (`IMPLEMENTATION_PLAN.md` §5) — it needs the macbook
  physically present.
- the cmux relay needed no v2 work at all: it was live-verified on the
  mac mini on 2026-07-16 (a real claude code "needs input" alert
  surfaced through the overlay). only the macbook's cmux setting
  remains to configure.

code-level detail for all of the above was `archive/V2_TECHNICAL_SPEC.md`
(v2 shipped; was a v0 draft, same rules as the v1 spec; removed at
repo close-out 2026-07-23, see `git log -- docs/archive/`).

---

## 17. v5 decisions (locked 2026-07-17) — settings window (control panel)

reopens two locked lines with eyes open: §6's "no settings ui — the
config file is the settings surface" and §14's receive-only frontend
(for **one new window only**; the overlay's receive-only rule is
unchanged and permanent). code-level contract in
`V5_TECHNICAL_SPEC.md`; build sequence in `IMPLEMENTATION_PLAN.md`
§4.5.

- **entry point**: a fourth tray item, **settings…**, opening a second
  webview window (label `settings`) — a normal decorated, closable,
  activating window (tailscale-style), nothing like the overlay panel.
  the tray stays minimal; the window exists because the tray can't
  hold key entry or (future) animation previews.
- **two windows, two trust levels**: the overlay (`main`) keeps its
  listen-only capability file byte-for-byte. the settings window gets
  its own capability with exactly the invoke commands it needs
  (`get_config`, `get_secret_status`, `save_config_and_relaunch`,
  `set_secret`). the tauri v2 gotcha that makes this a decision
  rather than a formality: app-defined commands are allowed to *all*
  windows by default — per-window gating requires the explicit
  `tauri_build::AppManifest::commands` opt-in in `build.rs` plus
  autogenerated `allow-*` permissions granted only to the `settings`
  capability, with a label check inside each handler as
  defense-in-depth (spec §2).
- **save & relaunch**: config is still read exactly once at boot.
  saving validates rust-side, writes `config.toml` atomically
  (same-dir temp file + rename — a half-written file is a bricked
  boot given §10's fail-fast rule), then relaunches the app. **no
  hot-reload plumbing, ever** — restart is the reload mechanism; no
  file-watcher.
- **secrets stay in `secrets.toml` (0600), plaintext**: same pattern
  and loader lineage as the telegram token (§10, v3). "hash the key"
  was raised and rejected — hashing is one-way; an outbound api key
  must be sent as-is, so hashing would destroy it. macos keychain was
  evaluated and declined: encryption-at-rest buys little on a
  single-user machine and costs a dependency plus prompts. the honest
  security model here is file permissions.
- **keys are write-only across ipc**: the settings window can *set* a
  secret and read a masked status ("set (…a1b2)"); a full secret
  value never crosses ipc outbound. an **openrouter api key** field
  lands now — storage ahead of the ai features it will serve (the key
  sits unused until the first such feature; adding the field commits
  to nothing else).
- **master kill switch**: a persisted `start_paused` config flag —
  the app launches with promotion paused (tray reads "resume").
  amends §3's "always launches unpaused"; the tray toggle itself
  stays session-only. reuses paused semantics wholesale, no new
  queue states.
- **panel scope (v5)**: `start_paused`, espn on/off + leagues + poll
  interval, `default_ttl` / `port` / `max_queued_per_tier`, openrouter
  key, telegram enable + token/chat-id. `detect_path` stays
  file-only. **extended 2026-07-17** (same day, after the
  `v5-news-backend` merge landed rss config): news on/off + feeds +
  poll interval + ttl + max-per-poll (`rss_*` fields) join the panel —
  "i decide when to poll news" was the panel's founding ask.

---

## 18. espn live-match card (locked 2026-07-19, plan 039)

the queue's Topic-supersession / `RotationSpec::Recurring` machinery
gets its first producer: the espn poller. governing design:
`docs/design/scoreboard-topic-card.md` (plan 031 spike, approved).

- **opt-in, default off**: new config flag `espn_live_card` (default
  `false`). off remains byte-for-byte today's behavior — a burst of
  one-shot, topicless cards per match. on flips one live match into a
  single updating card.
- **topic identity**: `espn:{league}:{match_id}` (espn's own event id).
  every event for one match shares it, so kickoff/goal/card/half-time
  supersede each other in the single Slot instead of queueing as
  separate items.
- **rotation**: `Recurring { display_secs: espn_ttl_secs }` while in
  play (reuses the existing ttl — no new dwell knob); the full-time
  event is emitted `OneShot` on the *same* Topic, so supersession flips
  the visible card's rotation in place and it retires via the ordinary
  one-shot path — no bespoke teardown.
- **connector semantics unchanged**: every delta still fans out to
  every enabled connector even though the overlay shows one
  consolidated card (`Engine::accept` clones before enqueue, and the
  offer loop runs regardless of merge-vs-promote).
- **multi-match deferred**: scope is single-match correctness. multiple
  concurrent live matches each get their own Topic and share the tier
  via rotation-order/FIFO exactly as today's multi-match burst already
  does — no special arbitration.
- no `engine.rs`/`queue.rs` change was needed: `Engine::accept` is
  Topic/Rotation-agnostic by construction; all supersession and
  `Recurring`-requeue logic already lived in `queue.rs`, unit- and
  proptest-covered. this plan is that machinery's first caller, not a
  change to it.

## 19. weather source (locked 2026-07-19, plan 040 Part B)

a fifth `SourceKind` (`Weather`) and a new poller
(`src-tauri/src/weather_poller.rs`) against Open-Meteo — keyless, no
auth, no secrets handling, same `net.rs` client posture as the other
pollers. opt-in, default off (`weather_enabled = false`), same rule as
rss: ambient sources must not default on top of the app's primary
agent-notification purpose.

- **location is raw `weather_lat`/`weather_lon` config numbers** — no
  geocoding, no city-name lookup, no second API dependency.
- **ambient vs card is the football split, reused.** current conditions
  are *not* an `Event`: the poller hands an already-display-formatted
  `WeatherSummary` ("27°" + "Cloudy") to `engine.update_weather`, which
  folds it into the idle rail's `StatusState` exactly like the
  live-match summary (compare-then-store, wake only on change).
  threshold alerts are ordinary `accept()`-routed one-shot cards with
  `origin: SourceKind::Weather`. the two mechanisms never conflate
  "current conditions" with "a card."
- **units are display-only.** the poller requests the operator's unit
  (`weather_units`, default Celsius) directly from Open-Meteo via its
  `temperature_unit` query param — no client-side conversion for
  display. alert thresholds (`weather_temp_hot_c` = 36.0,
  `weather_temp_cold_c` = 14.0) are always stored and compared in
  Celsius regardless.
- **alerts are rain-incoming + temperature threshold only** — Open-Meteo
  has no severe-weather-warnings feed, so no third category exists.
  rain lookahead is hourly-resolution data: the "30-minute lookahead"
  reads the hourly entry closest to poll-time+30min, rounded to the
  nearest hour.
- **alert re-fire is edge-triggered, not level-triggered**: an alert
  fires once on crossing into alert territory, stays silent while the
  condition holds, and re-arms only after it clears — the poller carries
  per-alert "already fired" state across polls, the same shape
  `poller.rs`'s `Snapshot` uses for kickoff/half-time.
- **defaults**: `weather_poll_secs` = 900, rain = 30-min lookahead @
  60% probability, `weather_priority` = Medium (bracketed by espn High
  and rss Low), rotation-order slot right after Manual:
  `[Football, Manual, Weather, Cmux, News]`.
- **no new ipc**: `get_config`/`get_default_config` serialize the whole
  `Config`; the overlay stays receive-only. settings window gains a
  Weather section and a per-source test-notification button (the
  existing `send_test_notification` command, no new `#[tauri::command]`).
