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
- a **single visible slot**: at most one notification on screen at a
  time, permanently rotating — not a stacked queue
- accepts pushes from five sources: the `notchtap` cli, cmux's
  notification relay (including claude code "agent needs input"
  alerts), an ESPN live-football poller, an rss news poller, and an
  Open-Meteo weather poller (ambient idle-rail chip plus
  rain/temperature threshold alerts)
- each source has a configurable Priority (`Low`/`Medium`/`High`);
  within a tier, a configurable Rotation Order breaks ties ahead of
  plain arrival order
- news items render as status-rail cards and are overlay-only — never
  relayed outbound
- outbound: accepted events (except news) are relayed to Telegram
- a settings window (opened from the tray) edits config and secrets;
  saving relaunches the app — there's no hot-reload
- renders as a slick, animated overlay — notch-cutout-aware on the
  MacBook, floating HUD on the Mac mini

### global hotkeys

these are OS-level global grabs — they work even when notchtap isn't
focused:

| shortcut | action |
|---|---|
| ⌃⇧N | toggle expand on the visible notification |
| ⌃⇧O | open the story/link for the visible item (news only) |
| ⌃⇧X | dismiss the visible item now |
| ⌃⇧P | toggle pause (stop/resume promotion) |

## tech stack

- **core**: Rust (Tauri) — HTTP listener, event bus, notification queue
- **UI**: React + TypeScript + CSS keyframes — rendering and animation
- **native shim**: tiny Swift CLI (`notchtap-detect`) for notch geometry
- **HTTP**: Axum (testable in-process via `tower::ServiceExt::oneshot`)
- **testing**: `cargo test` (Rust) + `vitest` (frontend)

see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for full rationale on every
decision (why Tauri over Electron, why no App Store, why this stack,
etc.).

## quick start

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

or use the [`justfile`](justfile): `just setup` installs web deps on a
fresh clone, and `just test-all` runs every check CI runs (fmt, clippy,
tests, audits, tsc, vitest, vite build, cli + swift checks) in one
command — `brew install just` first.

## setup

- rust toolchain via [`rustup`](https://rustup.rs) — required for
  `cargo build`/`cargo test`
- build the notch-detection helper and symlink it where the app expects
  it (or point `detect_path` in config at it instead):
  ```bash
  swift build -c release   # from notchtap-detect/
  ln -s "$(pwd)/.build/release/notchtap-detect" /usr/local/bin/notchtap-detect
  ```
- `brew install jq` — the `notchtap` cli script needs `jq` and `curl`
- optionally symlink the `notchtap` script somewhere on your `PATH`
- first run: if `~/.config/notchtap/config.toml` is absent, the app
  runs with all defaults and never creates the file — only a
  settings-window save creates it
- logs: `~/Library/Logs/notchtap/notchtap.log` (10 MB × 3 rotation)

## project docs

| doc | purpose |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | locked decisions: scope phasing, stack, cross-device behavior, distribution |
| [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) | phased build sequence, exit criteria for v1–v5 |
| [`docs/TESTING_STRATEGY.md`](docs/TESTING_STRATEGY.md) | what gets automated vs. manual, framework choices, per-component test plan |
| [`docs/V3_6_TECHNICAL_SPEC.md`](docs/V3_6_TECHNICAL_SPEC.md) | v0 draft: permanent rotating overlay — single-slot queue, priority tiers, global hotkeys |
| [`docs/V5_TECHNICAL_SPEC.md`](docs/V5_TECHNICAL_SPEC.md) | v0 draft: settings window |
| [`docs/archive/V1_TECHNICAL_SPEC.md`](docs/archive/V1_TECHNICAL_SPEC.md) | archived — v1 shipped; superseded by the specs above |
| [`docs/archive/V2_TECHNICAL_SPEC.md`](docs/archive/V2_TECHNICAL_SPEC.md) | archived — v2 shipped; superseded by the specs above |
| [`docs/archive/V3_TECHNICAL_SPEC.md`](docs/archive/V3_TECHNICAL_SPEC.md) | archived — v3 shipped; superseded by the specs above |
| [`docs/archive/BLIND_REVIEW.md`](docs/archive/BLIND_REVIEW.md) | planning-pass audit, folded into the docs above |
| [`docs/archive/CHANGES_SUMMARY.md`](docs/archive/CHANGES_SUMMARY.md) | changelog of the planning-pass edits |
| [`CONTEXT.md`](CONTEXT.md) | glossary / ubiquitous language (Promotion, Visible/Waiting, Paused, …) |
| [`CLAUDE.md`](CLAUDE.md) | guidance for Claude Code when working in this repo |

## scope

- **macOS only** — no Windows/Linux target, ever
- **personal use** — runs on the author's own two machines, no App Store,
  no paid Apple Developer account required
- **clean-room build** — no code, IP, or branding from any third-party
  reference app is used
