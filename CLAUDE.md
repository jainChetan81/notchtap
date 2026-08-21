# notchtap repository guide

Canon guide, every agent + maintainer working this repo. Keep it lean: history lives in `git log`, locked decisions in `docs/ARCHITECTURE.md`, testing approach in `docs/TESTING_STRATEGY.md`, per-plan record in `plans/INDEX.md`. Domain terms: `docs/GLOSSARY.md`.

## layout & symlinks

- This is the single physical guide; `AGENTS.md` + `CONTEXT.md` are compatibility symlinks to it. Never create a second copy.
- `.claude/skills/` is the single checked-in skill tree; `.agents/skills` is a symlink to it. Never copy skills between tool-specific dirs.
- `src/` authoritative UI implementation. `prototype/*.html` sole static visual reference — mirrors shipped surfaces, never proposals/experiments.

## writing rules (comments + docs)

Comments and docs describe the CURRENT product only, present tense. Never write plan numbers, dates, review citations, or "what this used to be" — history lives in git. Keep only: invariants code can't express ("never X, it breaks Y"), cross-file couplings named explicitly, security boundaries, short API doc comments. Tool directives (biome-ignore, @ts-expect-error, SAFETY:, #[allow] justifications) stay verbatim.

## commands

Standard invocations in `package.json` + `justfile` (`npm run tauri dev`, `npx tsc --noEmit`, `npx vite build`, `cargo build`/`cargo test` from `src-tauri/`, `npx vitest run`). Non-obvious:

- Enforcing lint gate (CI + `just check-web`) is `npx biome ci .`; `npx biome check .` is local dev only — not interchangeable.
- `./notchtap --title "t" --body "b"` pushes a notification to `/notify` (default `127.0.0.1:9789`, override `--port` or `$NOTCHTAP_PORT`); `notchtap run -- <cmd>` wraps a long command and pushes a completion card.
- `just test-all` mirrors `.github/workflows/ci.yml` exactly; fresh clone runs `just setup` first (`brew install just` if missing).

No repo-wide coverage gate — the bar is "every listed example surface has a passing test". Test counts live only in `docs/TESTING_STRATEGY.md` §0. Physical-hardware behaviour (notch geometry, hud placement, animation look) stays a manual checklist by design (§5/§6).

## architecture

Tauri app: rust core plus react/ts webview ui, not electron, not pure native swift. rust core (`src-tauri/`) owns the loopback http listener (`/notify`, `/agent/events`), typed event bus, single-slot priority queue, Agent Registry, window positioning — the only process talking to the outside world. frontend renders only (overlay `index.html` → `src/App.tsx`, settings `settings.html` → `src/settings/`); per-event-type presentation stays a data change, not a new render path.

- **One runtime branch, not two builds**: same compiled app on notch macbook + notchless mac mini; runtime safe-area check picks notch vs top-center hud.
- **Swift↔rust is subprocess, not ffi**: `notchtap-detect` prints safe-area json; rust shells out and parses. Keep the decision function pure and separate from the subprocess call.
- **Display-only notifications** — no approve/deny round-trip into agent clis (`ARCHITECTURE.md` §10).

## ipc & security

The overlay (`main`) window is **receive-only**: it listens for rust-published events, invokes nothing. `src-tauri/capabilities/default.json` grants listen/unlisten and NOTHING else — **must never change**.

Settings window invokes only because `src-tauri/build.rs` opts into deny-by-default command allowlisting plus `capabilities/settings.json` plus per-handler label checks. **Never add a new `#[tauri::command]` without adding it to that `build.rs` list** — else it silently becomes callable from the overlay, breaking receive-only. Contract + command table: `ARCHITECTURE.md` §9.

Clicks don't imply invoke: a native `NSEvent` monitor (`src-tauri/src/click.rs`) observes mouseDown rust-side and pushes typed events down the same channel as hover-changed. Wanting an `invoke` for a click means you're about to break the boundary (`ARCHITECTURE.md` §11).

## rules that bite

- **`SlotState::dedup_eq`:** continuously-varying wire fields (e.g. `ttl_ms`/`remaining_ms`) must extend `dedup_eq` explicitly, never rely on derived `PartialEq` — deriving makes every tick read as content change.
- **Rust errors:** `thiserror` inside library/internal modules (structured, matchable); `anyhow` at application boundaries (HTTP maps to 400 malformed / 429 tier full / 500 unexpected).
- **Naming:** product `notchtap`, repo `mac-notification-nudge`, generic terms otherwise. No third-party branding, logos, assets, trade dress, or implied affiliation. Runtime names (Claude Code, Codex, Kimi, OpenCode) appear only where compatibility requires.
- **Placement:** pure helpers in `src/lib/`, data hooks one-per-file in `src/hooks/` (screens never touch `window.__NOTCHTAP_*`/`listen()` directly), rendering-only components, settings-window code confined to `src/settings/` importing helpers from `src/lib/`. Wait for the second use before extracting a util; keep it pure, named, tested — one test file per util file next to it.

Dev machine: mac mini (no notch), user `chetanjain`; rust toolchain installed. Notch-mode behaviour needs per-change verification on the macbook.
