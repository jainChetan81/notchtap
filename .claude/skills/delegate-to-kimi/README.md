# delegate-to-kimi

canonical home for this claude code skill, as of 2026-07-15. previously lived
in the kharcha repo; moved here because the skill isn't kharcha-specific —
it's a general pattern for delegating bounded coding tasks to `kimi`
(`~/.kimi-code/bin/kimi`, moonshot ai's coding cli) as a background execution
agent, with claude as manager: claude plans, kimi codes, claude verifies
independently.

## how to use it

- `SKILL.md` in this folder is the file itself — claude code (or any tool
  that reads skills) loads it directly from here.
- kharcha's `.claude/skills/delegate-to-kimi` and the global
  `~/.claude/skills/delegate-to-kimi` are both symlinks pointing at this
  copy. edit this file; both symlinks pick the change up automatically. don't
  edit the symlinked copies.
- trigger it with a one-line prompt like "delegate this to kimi" in any
  project where the skill is available.

## design history

the full record of how this skill got its current shape — five review
rounds, two pal-review (multi-model) passes, kimi's own review of itself
from the executor's seat, what was proposed and rejected — stays in the
kharcha repo, since that's where the work actually happened:
`~/Desktop/Kharcha/docs/delegate-to-kimi/` (`README.md`, `DESIGN_HISTORY.md`,
`kimi-review.md`).
