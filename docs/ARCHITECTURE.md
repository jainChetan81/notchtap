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
| whatsapp + other connectors | — | — | yes |
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

v3 adds a notifiers layer alongside the ui (whatsapp/twilio, telegram,
etc.) — deliberately not drawn here, it's out of scope until v3.

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

---

## 4. animation system

v1: **one** generic template — enter/hold/exit via **css keyframes**, no per-type branching.
gets the pipe working end to end before investing in variety.

v2: swap the single template for a config table (event type →
animation), e.g. goal = confetti + bounce, posture-alert = shake,
generic/cmux = simple slide. **css keyframes** (or framer motion, evaluated
at v2 time) keeps this a config change, not a new code path, when that
point is reached.

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
- menu-bar tray icon (tauri's tray api) with exactly two items:
  **pause** (semantics in §3 — label toggles to "resume" while paused)
  and **quit**. no settings ui — the config file (§10) is the settings
  surface. tauri's default icon is fine until the deferred real-icon
  item (`IMPLEMENTATION_PLAN.md` §4)
- **always-on-top** (`setAlwaysOnTop` / `NSWindowLevel.floating`) — v1
day one. a notification overlay buried under other windows is useless.

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

---

## 7. cli push — v1's actual notification source

**direct**: any script or terminal command calls the local cli/socket
directly. this is the baseline — always available, zero dependencies.

**cli contract (locked)**: flags only, one form for humans and scripts
alike — `notchtap --title <t> --body <b> [--subtitle <s>] [--port <p>]`.
no positional form. a non-empty `--subtitle` is folded by the cli into
the posted body as `"<subtitle> — <body>"` — the `/notify` wire schema
stays exactly `{title, body}`; the server never learns "subtitle"
exists. port resolution: `--port` flag → `$NOTCHTAP_PORT` env var →
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

**v3**: whatsapp (twilio recommended — official-adjacent, one rest
call, no baileys ban-risk) and other connectors (telegram, etc.) sit
here as additional notifiers, same interface, no core rework.

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

v2 may add: espn league list, cmux integration on/off, posture
module on/off. api keys (twilio, etc.) live in a separate env var or
secret file — never in the committed config.

the rust core reads this file once at startup. changes require a restart
in v1; a file-watcher or settings ui is a v2+ convenience.

---

## 11. logging & observability

- **rust core**: use `tracing` (already pulled in by tauri/axum). write
to a rotating log file at `~/.local/share/notchtap/logs/notchtap.log`
(or `~/Library/Logs/notchtap/` on macos). rotate at 10 mb, keep 3
backups. log level `info` in release, `debug` in dev.
- **frontend errors**: any react error boundary catch or animation
failure logs back to the same file via a tauri command — the frontend
never writes to disk directly.
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
- the tauri capabilities file should reflect this: one event permission,
  no filesystem, no shell, no network from the frontend

this boundary matters if the app ever processes untrusted content (e.g.,
whatsapp messages from unknown senders in v3). establishing the
one-way data flow in v1 means v3 doesn't accidentally open a hole.

---

## 15. status

all decisions above are locked — scope, stack, distribution model,
cross-device behaviour. see `IMPLEMENTATION_PLAN.md` in this folder for
the phased build sequence and exit criteria.
