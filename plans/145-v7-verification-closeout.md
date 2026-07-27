# 145 — v7 manual verification + doc closeout

> v7 ticket 13 of 13 (spec §13, §14 step 9). Filed 2026-07-26.

**What to build:** the release gate. v7 is not done until real
providers, real hardware, and the active docs all agree with the
shipped behavior.

- Manual real-session smoke for ALL FOUR runtimes (Claude Code,
  Codex, Kimi, OpenCode) — per §14, no runtime is shown as fully
  supported based only on synthetic fixtures.
- Manual T3 Code smoke for at least Codex and Claude Code (Host
  reporting + focus).
- Manual overlay scroll/pointer pass-through pass on the mac mini and
  notch-geometry pass on the macbook (per CLAUDE.md, notch-mode needs
  per-change verification there).
- `docs/TESTING_STRATEGY.md` gains §4.13 (agent test contract) and §0
  counts are RECOUNTED live, not transcribed.
- Active-doc closeout: `ARCHITECTURE.md`/`IMPLEMENTATION_PLAN.md`
  roadmap text marks the cmux relay historical; CONTEXT.md gains the
  v7 glossary terms actually shipped (Agent Adapter, Agent Board,
  Agent Session, Agent Registry) if not already added en route;
  prototypes updated or explicitly archived (§12 last row).
- Full gate: `cargo test`, `npx vitest run`, typecheck, `npx biome
  ci .`, builds — all green; `just test-all` mirror run.

**Blocked by:** 139, 140, 141, 142, 143, 144 (everything).

**Status:** ready-for-agent (final verification pass; parts need the
operator's machines)

- [ ] Four-runtime real-session smoke recorded (what worked, gaps
      observed vs declared capabilities).
- [ ] T3 Code smoke recorded for Codex + Claude Code.
- [ ] Mac mini pointer/scroll pass + macbook notch pass recorded.
- [ ] TESTING_STRATEGY §4.13 written; §0 recounted.
- [ ] Docs/prototypes closeout done; `grep -ri cmux` active surface
      still clean.
- [ ] `just test-all` green.
