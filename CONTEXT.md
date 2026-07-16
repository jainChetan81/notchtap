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
