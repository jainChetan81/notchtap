# notchtap task runner — mirrors .github/workflows/ci.yml exactly.
# `just test-all` before calling any phase done (IMPLEMENTATION_PLAN.md §6).
# fresh clones: run `just setup` first — CI's web job runs the equivalent
# `npm ci` before its own gates; `test-all` does not do this for you.

default:
    @just --list

# one-time / after-pull: install web deps (rust toolchain via rustup is
# a prerequisite, not installed here)
setup:
    npm ci

# run the app in dev mode
dev:
    npm run tauri dev

# rust gates (run from src-tauri, as CI does — `--locked` per plan 007)
test-rust:
    cd src-tauri && cargo test --locked

check-rust:
    cd src-tauri && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings

# frontend gates
test-web:
    npx vitest run

# lint/format + typecheck (biome from plan 016, then tsc — CI order)
check-web:
    npx biome ci . && npx tsc --noEmit

audit-web:
    npm audit --audit-level=high

build-web:
    npx vite build

# script gate
check-cli:
    sh -n notchtap

# swift detect-binary compile check
check-swift:
    cd notchtap-detect && swift build

# plan 104: build the vendored, SHA-pinned mediaremote-adapter
# (src-tauri/vendor/mediaremote-adapter/VENDORED.md) and install it to
# the fixed runtime path now_playing_adapter_dir defaults to. Mirrors
# notchtap-detect's own "built/installed out of band" convention
# (config.rs's detect_path doc comment) — the rust core never builds
# this itself, only shells out to whatever's already installed. `build/`
# is git-ignored (cmake output, never committed). NO sudo: if
# /usr/local/lib isn't writable, this fails loudly rather than silently
# escalating privileges — create /usr/local/lib/notchtap yourself (with
# whatever ownership your machine needs) before re-running this recipe.
build-media-adapter:
    cd src-tauri/vendor/mediaremote-adapter && cmake -B build -DCMAKE_BUILD_TYPE=Release
    cd src-tauri/vendor/mediaremote-adapter && cmake --build build
    mkdir -p /usr/local/lib/notchtap/mediaremote-adapter
    cp -R src-tauri/vendor/mediaremote-adapter/build/MediaRemoteAdapter.framework /usr/local/lib/notchtap/mediaremote-adapter/
    cp -R src-tauri/vendor/mediaremote-adapter/bin /usr/local/lib/notchtap/mediaremote-adapter/
    perl /usr/local/lib/notchtap/mediaremote-adapter/bin/mediaremote-adapter.pl /usr/local/lib/notchtap/mediaremote-adapter/MediaRemoteAdapter.framework test

# everything CI runs, locally, except cargo-audit (binary isn't
# installed on the dev machine; CI's rustsec/audit-check action runs it
# instead — see .github/workflows/ci.yml's rust job)
test-all: check-rust test-rust check-web audit-web test-web build-web check-cli check-swift

# manual push against the local endpoint
push title body:
    ./notchtap --title "{{title}}" --body "{{body}}"
