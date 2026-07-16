# notchtap — ubiquitous language

glossary only. no implementation details — those live in
`docs/V1_TECHNICAL_SPEC.md`. decisions live in `docs/ARCHITECTURE.md`
and `docs/adr/`.

## terms

- **Event** — one incoming push (title + body, plus type/priority/ttl
  assigned by the engine). the unit that flows through the system.
- **Notification** — an Event being (or waiting to be) displayed.
  every Notification is an Event; "Notification" is the word for the
  display-side view of it.
- **Visible** — the set of Notifications currently rendered on screen.
  capped (`max_concurrent`). ordered as a stack.
- **Waiting** — Notifications accepted but not yet shown, in FIFO
  order. capped (`max_queued`); pushes beyond the cap are rejected.
- **Promotion** — the moment a Waiting Notification moves into
  Visible. the engine's decision alone; the frontend never promotes.
- **TTL** — how long a Notification stays Visible, measured from
  Promotion (not from arrival). time spent Waiting never burns TTL.
- **Paused** — engine state in which Promotion is disabled. pushes are
  still accepted and buffered into Waiting (caller is told the app is
  paused); already-Visible Notifications finish their natural TTL and
  exit. Resuming re-enables Promotion; nothing is dropped.
- **Polling Pause** — a Poller-level state (per source, from the tray)
  in which the Poller stops checking its external service; no Events
  are produced and changes during the pause are never seen. distinct
  from Paused: Paused buffers and drops nothing, a Polling Pause
  observes nothing. resuming re-baselines silently, like a first
  sighting.
- **Presentation Mode** — how the window anchors: **Notch** (over the
  macbook's notch cutout) or **HUD** (floating top-center, on
  notchless machines). decided at runtime, never at build time.
- **notchtap** — the product: the always-on engine + overlay app, and
  the name of the CLI that pushes to it.
- **notchtap-detect** — the standalone swift helper that reports
  screen safe-area geometry so the engine can pick a Presentation
  Mode.
- **Relay** — an external tool (cmux in v2) forwarding its own
  notifications into notchtap. a Relay is heads-up only: it can never
  answer back into the tool that raised the alert.
- **Connector** — an outbound sink (telegram in v3) that receives
  every accepted Event and forwards it off the machine, best-effort.
  a Connector observes acceptance, not Promotion: the queue's display
  rules (cap, TTL, Paused) never apply to it, and its failures never
  affect the pusher's response.
- **Notifier** — the outbound half of notchtap as a whole: the seam
  through which accepted Events leave the machine. Connectors are its
  members; the overlay is not one. a seam, not a code interface —
  earlier drafts said "the Notifier trait," but no trait exists (and
  none is needed until a second Connector does).
- **Poller** — an internal event source that repeatedly checks an
  external service (espn in v2) and turns observed *changes* into
  Events. a Poller emits deltas only: the first sighting of a match is
  silent, and repetition of an unchanged fact never produces an Event.
- **Score Update** — an Event produced when a watched match's score
  changes (a goal).
- **Match State** — an Event produced when a watched match's phase
  changes: kickoff, half-time, full-time (and cards, where reported).
