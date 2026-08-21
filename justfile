# notchtap task runner — mirrors .github/workflows/ci.yml exactly.
# Run `just test-all` before declaring repository work complete.
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

# Rust gates run from src-tauri with the locked dependency graph.
test-rust:
    cd src-tauri && cargo test --locked

check-rust:
    cd src-tauri && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings

# frontend gates
test-web:
    npx vitest run

# lint/format + typecheck (biome, oxlint, then tsc — CI order)
check-web:
    npx biome ci . && npx oxlint && npx tsc --noEmit

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

# everything CI runs, locally, except cargo-audit (binary isn't
# installed on the dev machine; CI's rustsec/audit-check action runs it
# instead — see .github/workflows/ci.yml's rust job)
test-all: check-rust test-rust check-web audit-web test-web build-web check-cli check-swift

# manual push against the local endpoint
push title body:
    ./notchtap --title {{quote(title)}} --body {{quote(body)}}
