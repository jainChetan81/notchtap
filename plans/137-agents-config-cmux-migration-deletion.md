# 137 — `[agents]` config, cmux migration, cmux deletion

> v7 ticket 5 of 13 (spec §7, §12). Filed 2026-07-26.

**What to build:** the config surface for v7 and the complete removal
of cmux as a product concept. After this ticket, a user upgrading from
a cmux-era config/history loses nothing (everything reads as `Agent`),
and no active code, UI, hook, or doc mentions cmux except as history.

- New `[agents]` TOML block per §7 (enabled, retention, stale
  threshold, informational toggle, four per-kind priorities, four
  per-runtime enable flags), wired into tickets 133/135 behavior.
  Flat compat layer gains `agent_ttl_secs`/`agent_priority` only where
  still required; `[agents]` kind-priorities take precedence.
- Idempotent migration per §7: `SourceKind` drops `Cmux`, adds
  `Agent`; legacy `"cmux"` deserializes as `Agent` for one release;
  `cmux_priority`/`cmux_ttl_secs` alias to the new keys only when the
  new key is absent; rotation-order `Cmux` entry rewritten in place to
  `Agent` then existing heal/dedupe runs; persisted history Origin
  `Cmux` reads as `Agent`; serialization writes only new names.
- New default Rotation Order: `[Football, Manual, Weather, Agent,
  News]`.
- §12 deletion matrix, fully: `RequestSource::Cmux` in `/notify`;
  `hooks/notchtap-cmux-hook.sh`; cmux handling + `--source cmux` +
  env autodetection in the `notchtap` CLI (manual pushes stay Origin
  `Manual`); `CmuxSection.tsx` and its Settings tab (Agents section
  arrives in ticket 143 — the tab is removed now, not left dormant);
  frontend source labels/fixtures/styles across the ~8 cmux-touching
  TS/TSX files; active architecture/testing/roadmap text marks the
  relay superseded; prototypes updated or explicitly archived. Git
  history is the recovery mechanism — no dormant compatibility code.
- Update the cmux-relay memory/setup expectations is out of scope for
  code; note for operator: the mac mini's live cmux relay will post as
  Origin `Manual` (or be re-pointed to an adapter) after this lands.

**Blocked by:** 135 (needs `Agent` origin/source to migrate onto).

**Status:** ready-for-agent

- [ ] Migration tests from EVERY legacy field/value (§13): old config
      with `cmux_priority`/`cmux_ttl_secs`/rotation `Cmux`/history
      Origin `Cmux` all load correctly and re-serialize with new names
      only; running migration twice is a no-op.
- [ ] `grep -ri cmux` over `src/`, `src-tauri/`, `hooks/`, `notchtap`,
      `prototype/` returns nothing active (historical plan/review docs
      exempt).
- [ ] Settings still builds and renders without the removed tab.
- [ ] `cargo test` + `npx vitest run` + typecheck green.
