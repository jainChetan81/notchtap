# Blind Review — mac-notification-nudge

> Historical record — file paths below reflect the layout at review time
> (docs at repo root). The planning docs now live under `docs/`
> (`../ARCHITECTURE.md`, `../IMPLEMENTATION_PLAN.md`,
> `../TESTING_STRATEGY.md`); `CLAUDE.md` and `README.md` are still at
> the repo root. Not updated inline below to preserve the record as-written.

> Role: Observer / Reviewer only. This is feedback for the executor to act on.  
> Review date: 2025-07-16  
> Docs reviewed: `ARCHITECTURE.md`, `CLAUDE.md`, `IMPLEMENTATION_PLAN.md`, `TESTING_STRATEGY.md`  
> **Status: ALL ITEMS ADDRESSED** — see individual sections below for what was changed.  
> **Follow-up (2026-07-16):** a second, independent pass checked these "addressed" claims against actual file contents. Item 3.7 below was marked done but the content didn't exist in the cited location — now corrected. See `CHANGES_SUMMARY.md`'s addendum for that pass and three other fixes (a corrupted section in `ARCHITECTURE.md`, an unresolved contradiction in `IMPLEMENTATION_PLAN.md` §4, and gaps in `TESTING_STRATEGY.md`) that this document's "ALL ITEMS ADDRESSED" header didn't catch.

---

## 1. Executive Summary

The docs are **well above average** for a pre-scaffold personal project. Architecture is locked with clear reasoning, phased scope is disciplined (v1 is genuinely thin), testing strategy has honest boundaries, and the cmux integration is the right call (consume existing hooks, don't build new ones). The project has a high chance of shipping v1 cleanly if the gaps below are closed.

That said, there are **~15 specific items** that should be resolved or documented *before* `npm create tauri-app@latest` runs, or they will surface as friction during implementation.

---

## 2. What's Strong (Don't Change)

| Area | Why It Works |
|---|---|
| **Phased scope** | v1 is actually thin — engine + queue + one animation + cli. Not a "thin v1" that secretly includes 6 features. |
| **Tauri over Electron** | Correct call for a 24/7 background app. The reasoning in `ARCHITECTURE.md` §8 is sound. |
| **Testing pyramid** | Honest about what's automatable vs. manual. The `tower::ServiceExt::oneshot` justification for axum is smart. |
| **cmux relay approach** | Using cmux's existing notification-command hook instead of writing a Claude Code `PreToolUse` hook is the right boundary. The explicit limit note (`ARCHITECTURE.md` §7) is excellent — it prevents scope creep. |
| **Distribution model** | No App Store, no $99 fee for personal use, correct understanding of macOS vs. iOS provisioning. |
| **Cross-device as runtime branch** | One build, notch vs. HUD decided at runtime. Avoids the dual-target trap. |
| **Pure function isolation for notch/hud** | `presentation_mode(safe_area_top_inset: f64) -> Mode` is exactly the right split. |
| **No coverage percentage gate** | Per-component test requirements are better than a repo-wide number. |

---

## 3. Issues, Gaps & Questions — ALL RESOLVED

### 🔴 Must Resolve Before Scaffold (v1 Blockers) — ALL DONE

#### 3.1 CLI / Binary Name — ✅ `notchtap`
- **Action:** Picked `notchtap`. Updated all references across all docs.
- **Files changed:** `ARCHITECTURE.md` §7, `IMPLEMENTATION_PLAN.md` §1.4, §1.5, §2.2, `CLAUDE.md` §1.1, §naming.

#### 3.2 Default Port for `/notify` — ✅ `9789`
- **Action:** Picked `127.0.0.1:9789`. Documented in `ARCHITECTURE.md` §7, `IMPLEMENTATION_PLAN.md` §1.2, `CLAUDE.md` §1.1.
- **Rationale:** Unassigned in IANA, unlikely to collide.

#### 3.3 Swift ↔ Rust Integration Pattern — ✅ `notchtap-detect` CLI
- **Action:** Specified: tiny standalone Swift CLI tool (`notchtap-detect`) that prints JSON to stdout. Rust core calls it via `std::process::Command`. No FFI.
- **File changed:** `ARCHITECTURE.md` §5.

#### 3.4 TTL / Timing Defaults — ✅ Documented
- **Action:** Added default values table in `ARCHITECTURE.md` §3: TTL=8s, max_concurrent=3, max_queued=50, enter/exit=300ms, overflow=HTTP 429.

#### 3.5 macOS Minimum Version — ✅ macOS 13+
- **Action:** Added to `ARCHITECTURE.md` §6: v1 targets macOS 13 (Ventura). `SMAppService` requires it; macOS 12 would need `SMLoginItemSetEnabled`.

---

### 🟡 Should Resolve Before or During v1 — ALL DONE

#### 3.6 Configuration / Settings Story — ✅ `~/.config/notchtap/config.toml`
- **Action:** New `ARCHITECTURE.md` §10. v1 fields: port, default_ttl, max_concurrent, max_queued. API keys in separate env/secret file.

#### 3.7 ESPN API Risk — ✅ Documented (citation corrected 2026-07-16)
- **Action:** Added note in `IMPLEMENTATION_PLAN.md` §2.1 (not `ARCHITECTURE.md` §7 — that citation was wrong and the content didn't actually exist anywhere until this correction pass): ESPN API is undocumented public, best-effort. Poller must fail gracefully (no crash, log a warning, skip the poll cycle).

#### 3.8 Tauri IPC Security — ✅ Documented
- **Action:** New `ARCHITECTURE.md` §14 and new `CLAUDE.md` section. Frontend is receive-only, no invoke commands. Capabilities file locked to minimum.

#### 3.9 Queue Overflow Behavior — ✅ `429` on max_queued exceeded
- **Action:** Added to `ARCHITECTURE.md` §3. Hard cap of 50 queued items, new pushes return HTTP 429.

#### 3.10 Deduplication — ✅ Acknowledged, deferred
- **Action:** New `ARCHITECTURE.md` §13. v1 has no dedup. Tracked for v2 if it becomes a real problem.

#### 3.11 AirPods Posture Module — ✅ Out of v2 scope
- **Action:** Updated `ARCHITECTURE.md` §6 and `IMPLEMENTATION_PLAN.md` §2.4: "future, not in v2 scope." Tracked idea, no committed timeline.

#### 3.12 Animation Engine — ✅ Locked to CSS keyframes
- **Action:** Updated `ARCHITECTURE.md` §4: v1 uses CSS keyframes. v2 may evaluate Framer Motion if needed. `IMPLEMENTATION_PLAN.md` §1.3 already specified CSS.

#### 3.13 Missing README.md — ✅ Created
- **Action:** Created `README.md` with project overview, stack, quick start, doc index.

---

### 🟢 Nice to Have / Polish — ALL DONE

#### 3.14 Logging Strategy — ✅ `tracing` + rotating file log
- **Action:** New `ARCHITECTURE.md` §11. `tracing` to `~/.local/share/notchtap/logs/`, 10MB rotation, 3 backups. Frontend errors log back via Tauri command.

#### 3.15 Always-On-Top — ✅ Moved to v1 requirements
- **Action:** Removed from `IMPLEMENTATION_PLAN.md` §4 (deferred). Added to `ARCHITECTURE.md` §6 as v1 day-one requirement. Click-through stays deferred.

#### 3.16 Justfile — ✅ Added to CLAUDE.md
- **Action:** Added note in `CLAUDE.md` §commands: recommend `justfile` with `dev`, `test-rust`, `test-web`, `test-all`, `build`, `push` recipes.

#### 3.17 Multi-Display Edge Case — ✅ Documented
- **Action:** New `ARCHITECTURE.md` §12. v1 uses Tauri default screen. v2 may query `NSScreen.screens` explicitly.

#### 3.18 Rust Error Handling — ✅ `thiserror` + `anyhow` split
- **Action:** New `CLAUDE.md` section. `thiserror` for library modules (matchable in tests), `anyhow` at application boundary (HTTP handlers, main.rs).

---

## 4. Typos & Corrections — ALL FIXED

| File | Issue | Fix |
|---|---|---|
| `ARCHITECTURE.md` | "generalises" → "generalizes" | ✅ Fixed |
| `ARCHITECTURE.md` | "~100s of mb" → "hundreds of mb" | ✅ Fixed |
| `ARCHITECTURE.md` | "dx" → "developer experience (dx)" | ✅ Fixed |
| `IMPLEMENTATION_PLAN.md` | "macos" → "macOS" | ✅ Fixed |

---

## 5. New Sections Added to Architecture

| Section | Content |
|---|---|
| §10 | Configuration & settings (`~/.config/notchtap/config.toml`) |
| §11 | Logging & observability (`tracing`, rotating file logs) |
| §12 | Multi-display edge case |
| §13 | Deduplication (v1: none, tracked for v2) |
| §14 | IPC security model (frontend receive-only, locked-down capabilities) |

---

## 6. Bottom Line

All 15+ review items have been addressed. The docs are now in a state where the executor can run `npm create tauri-app@latest` without hitting undocumented decisions. The architecture itself remains solid — no approach changes were needed, only "write down the decision."

**Recommended next step:** executor runs `npm create tauri-app@latest` with React + TypeScript template, confirms `npm run tauri dev` opens a blank window, then proceeds to `IMPLEMENTATION_PLAN.md` §1.2.
