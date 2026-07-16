# notchtap

a macOS-only background utility that shows animated, notch-anchored (or
HUD-style, on non-notch machines) push notifications, fed by a local CLI
/ HTTP endpoint.

built for two machines: a MacBook with a notch, and a Mac mini without
one. same codebase runs unmodified on both — only window placement
branches at runtime.

---

## what it does

- runs permanently as a menu-bar app (no dock icon)
- accepts notification pushes from the command line:
  `notchtap --title "title" --body "body"`
- auto-relays notifications from [cmux](https://cmux.com) (including
  claude code "agent needs input" alerts)
- renders each push as a slick, animated overlay — notch-cutout-aware on
  the MacBook, floating HUD on the Mac mini
- v1: one generic animation template, FIFO queue, TTL auto-dismiss
- v2: per-event-type animations, ESPN live football scores
- v3: outbound connectors (WhatsApp via Twilio, Telegram)

## tech stack

- **core**: Rust (Tauri) — HTTP listener, event bus, notification queue
- **UI**: React + TypeScript + CSS keyframes — rendering and animation
- **native shim**: tiny Swift CLI (`notchtap-detect`) for notch geometry
- **HTTP**: Axum (testable in-process via `tower::ServiceExt::oneshot`)
- **testing**: `cargo test` (Rust) + `vitest` (frontend)

see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for full rationale on every
decision (why Tauri over Electron, why no App Store, why this stack,
etc.).

## quick start (once scaffolded)

```bash
# install dependencies
npm install

# dev mode
npm run tauri dev

# run tests
cargo test              # from src-tauri/
npx vitest run          # from repo root

# trigger a notification manually (flags only — no positional form)
notchtap --title "hello" --body "world"
```

## project docs

| doc | purpose |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | locked decisions: scope phasing, stack, cross-device behavior, distribution |
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | phased build sequence, exit criteria for v1/v2/v3 |
| [`docs/TESTING_STRATEGY.md`](docs/TESTING_STRATEGY.md) | what gets automated vs. manual, framework choices, per-component test plan |
| [`docs/V1_TECHNICAL_SPEC.md`](docs/V1_TECHNICAL_SPEC.md) | v0 draft: concrete v1 file layout, types, api/json schemas, ready to code against |
| [`docs/V2_TECHNICAL_SPEC.md`](docs/V2_TECHNICAL_SPEC.md) | v0 draft: v2 delta — espn poller, event types, animation table, hardening fixes |
| [`docs/review/BLIND_REVIEW.md`](docs/review/BLIND_REVIEW.md) | planning-pass audit, folded into the docs above |
| [`docs/review/CHANGES_SUMMARY.md`](docs/review/CHANGES_SUMMARY.md) | changelog of the planning-pass edits |
| [`CONTEXT.md`](CONTEXT.md) | glossary / ubiquitous language (Promotion, Visible/Waiting, Paused, …) |
| [`CLAUDE.md`](CLAUDE.md) | guidance for Claude Code when working in this repo |

## scope

- **macOS only** — no Windows/Linux target, ever
- **personal use** — runs on the author's own two machines, no App Store,
  no paid Apple Developer account required
- **clean-room build** — no code, IP, or branding from any third-party
  reference app is used
