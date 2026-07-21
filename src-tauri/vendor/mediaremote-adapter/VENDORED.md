# Vendored: ungive/mediaremote-adapter

- **Upstream**: https://github.com/ungive/mediaremote-adapter
- **Pinned commit**: `3ac3d4bdf862c7b5399b4fba4df5689f5c38609a`
- **License**: BSD 3-Clause (see `LICENSE` in this directory — copied
  verbatim from upstream at the pinned commit)

**Frozen — never update without a reviewed plan.** This tree was
inspected file-by-file for network calls, filesystem writes outside the
process, and privilege escalation before being vendored — see
`docs/design/now-playing-adapter.md` §3 for the full inspection verdict
(plan 103's spike). Bumping the pinned commit requires repeating that
inspection checklist in a new reviewed plan, not a routine dependency
bump.

Copied subset (no `.git`, no build output): `bin/`, `src/`, `include/`,
`scripts/`, `CMakeLists.txt`, `Makefile`, `LICENSE`.

## Build / install

Run `just build-media-adapter` (plan 104) from the repo root. It
cmake-builds this tree into `build/` (git-ignored) and installs
`MediaRemoteAdapter.framework` + `bin/` to
`/usr/local/lib/notchtap/mediaremote-adapter/` — no sudo. The rust core
(`src-tauri/src/now_playing.rs`) shells out to the installed copy, never
to this vendored source tree directly.
