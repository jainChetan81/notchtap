# Plan 152: Add `notchtap-agent doctor` — a read-only report of which agent hooks are actually wired

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat acdaeb0..HEAD -- src-tauri/src/bin/notchtap_agent.rs src-tauri/src/agents/providers/ src/settings/sections/AgentsSection.tsx src/lib/sourceColors.test.ts docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md docs/TESTING_STRATEGY.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction / dx
- **Planned at**: commit `acdaeb0`, 2026-07-28

## Why this matters

notchtap supports four coding-agent runtimes (Claude Code, Codex, Kimi,
OpenCode). Installing the hooks is entirely the user's job — notchtap
never edits those config files. Today there is **no way to ask notchtap
whether the wiring worked**. The Settings window's Adapter Health cards
only report what has been *received*, so an un-wired runtime and a
correctly-wired-but-idle runtime look identical ("no events yet").

This has already cost real debugging time on this machine: the hooks had
to be rewritten to use an **absolute** path to the `notchtap-agent`
binary because a bare `notchtap-agent` did not resolve inside the
provider's spawn environment. Nothing in the product would have told the
user that.

After this plan, `notchtap-agent doctor` reads (never writes) the four
runtimes' config files and prints, per runtime: whether the config file
exists, how many of the expected hook events are wired, which are
missing, and — critically — whether the command string those hooks point
at actually resolves to an executable file. Plus the listener check and
the Kimi version gate that `status` already does.

## Current state

### The binary today

`src-tauri/src/bin/notchtap_agent.rs` is a thin argv-dispatch shell over
`notchtap_lib::agents::providers`. Three subcommands exist. Verbatim,
`notchtap_agent.rs:38-57`:

```rust
#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("hook") => run_hook(it.next().map(String::as_str)).await,
        Some("test") => run_test(it.next().map(String::as_str)).await,
        Some("status") => run_status(),
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage: notchtap-agent hook <claude-code|codex|kimi>\n       notchtap-agent test <runtime>\n       notchtap-agent status"
    );
}
```

Note `print_usage` writes to **stderr**, not stdout. Any check of its
output needs `2>&1`.

The module also carries a ```` ```text ```` usage block at
`notchtap_agent.rs:4-8` listing the same three subcommands.

`run_status` (`notchtap_agent.rs:246-288`) already does two of the checks
`doctor` needs. Verbatim, `notchtap_agent.rs:246-263`:

```rust
fn run_status() -> ExitCode {
    let port = delivery::resolve_port();
    let addr = format!("127.0.0.1:{port}");
    let listener_ok = match addr
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())
        .and_then(|sock| {
            TcpStream::connect_timeout(&sock, Duration::from_millis(500)).map_err(|e| e.to_string())
        }) {
        Ok(_) => {
            println!("notchtap: listening on 127.0.0.1:{port}");
            true
        }
        Err(e) => {
            println!("notchtap: not reachable on 127.0.0.1:{port} ({e})");
            false
        }
    };
```

`delivery::resolve_port()` (`src-tauri/src/agents/providers/delivery.rs:40-45`)
reads `$NOTCHTAP_PORT` and falls back to `DEFAULT_PORT` (9789,
`delivery.rs:27`). Reuse it; do not re-implement port resolution.

`kimi_version::probe_hook_support()` returns
`HookSupport::Supported { detected: String }` or
`HookSupport::Unavailable { detected: Option<String>, minimum: &'static str }`
(`src-tauri/src/agents/providers/kimi_version.rs:42-51`).

**The minimum version is `0.9.0`**, from
`kimi_version.rs:36` (`MINIMUM_HOOK_VERSION: (u32, u32, u32) = (0, 9, 0)`)
and `kimi_version.rs:40`
(`MINIMUM_HOOK_VERSION_STR: &str = "0.9.0"`). Always render this by
reading the constant. **Never hardcode a version number anywhere in your
code or tests.**

### The providers module

`src-tauri/src/agents/providers/mod.rs:29-36`, verbatim:

```rust
pub mod claude_code;
pub mod codex;
pub mod delivery;
pub mod diagnostics;
pub mod kimi;
pub mod kimi_version;
pub mod stub;
pub mod wire;
```

Above that list is a module doc with one `- [`name`] — description`
bullet per module. Your new module needs a line in both.

### The config files and expected hooks (authoritative today)

These strings live in `src/settings/sections/AgentsSection.tsx`. The
snippet constants are at `AgentsSection.tsx:47-117`
(`CLAUDE_CODE_SNIPPET` at `:47`, `CODEX_SNIPPET` at `:62`,
`KIMI_SNIPPET` at `:75`, `OPENCODE_SNIPPET` at `:115`) and the
`ADAPTER_CARDS` array with the target-file paths is at
`AgentsSection.tsx:119-152`.

| runtime | config file (global) | format |
|---|---|---|
| `claude-code` | `~/.claude/settings.json` | JSON |
| `codex` | `~/.codex/hooks.json` | JSON |
| `kimi` | `~/.kimi-code/config.toml` | TOML |
| `opencode` | `~/.config/opencode/plugins/notchtap.ts` | file presence |

The OpenCode **filename** is an inference: `AgentsSection.tsx:148` names
only the directory (`~/.config/opencode/plugins/`), and
`adapters/opencode/notchtap.ts:18` likewise. `notchtap.ts` is the name
the repo's own file uses and the name the uninstall text assumes
("Delete notchtap.ts from the plugins directory"), so use it — but
render the checked path in the output so a user with a different
filename can see what was looked for.

The JSON shape for Claude Code and Codex (identical) is:

```json
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "notchtap-agent hook claude-code" }] }]
  }
}
```

The TOML shape for Kimi is an array of tables:

```toml
[[hooks]]
event = "SessionStart"
command = "notchtap-agent hook kimi"
```

### Repo conventions you must follow

1. **Pure / impure split is a hard rule in this repo.** `CLAUDE.md:158-161`
   states it: *"keep the pure decision logic (`fn presentation_mode
   (safe_area_top_inset: f64) -> Mode`) separate from that subprocess
   call — the function is unit-testable, the subprocess call is not"*.
   `kimi_version.rs` is the in-module exemplar: a pure
   `hook_support(version_text)` decision function plus an isolated impure
   `detect_installed_version()`. **Every decision function in `doctor.rs`
   takes file *contents* (a `&str`), a `bool`, or an explicit `&Path` —
   none of them read `$HOME` or the environment.**
2. **Tests must never touch the real home directory.**
   `src-tauri/src/agents/providers/diagnostics.rs:67-71` states why: the
   testable core is split out *"so tests can point it at a throwaway temp
   dir instead of mutating the process-global `HOME` env var (which would
   race against every other test in this binary…)"*.
3. **No new dependencies.** The crate deliberately has no `clap`
   (`notchtap_agent.rs:16-19`). `serde_json` (1), `toml` (1.1.3) and
   `dirs` (6.0.0) are already in `src-tauri/Cargo.toml:32,38,39`.
4. **Error handling** (`CLAUDE.md:213-219`): `thiserror` for
   library/internal modules, `anyhow` at boundaries. Neither is needed
   here — a missing config file is normal *data*, not an error. Do not
   introduce an error type.
5. **Tests live in an in-file `#[cfg(test)] mod tests`.**
6. **Test counts live in `docs/TESTING_STRATEGY.md` §0 and nowhere else**
   (`CLAUDE.md`).

### Vocabulary (from `CONTEXT.md` — use these words in output and comments)

- **Agent Runtime** — the coding-agent product an Agent Session belongs
  to (Claude Code, Codex, Kimi, OpenCode). Say "runtime", not "provider".
- **Agent Adapter** — the per-runtime hook/plugin integration. What
  `doctor` inspects is the *Adapter* installation.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | exit 0, `0 failed` |
| Doctor tests only | `cd src-tauri && cargo test --locked doctor` | exit 0, `22 passed` |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Rust lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Build the binary | `cd src-tauri && cargo build --bin notchtap-agent` | exit 0 |
| Frontend tests | `npx vitest run` | exit 0, all files pass |
| One frontend file | `npx vitest run src/settings/hookEventParity.test.ts` | exit 0 |
| Frontend lint | `npx biome ci .` | exit 0 |
| Typecheck | `npx tsc --noEmit` | exit 0 |

If `cargo` is not on PATH, prefix with `PATH="$HOME/.cargo/bin:$PATH"`.
A cold `src-tauri/target/` makes the first `cargo` command take several
minutes. That is normal, not a failure.

## Scope

**In scope** (the only files you should modify or create):

- `src-tauri/src/agents/providers/doctor.rs` (create)
- `src-tauri/src/agents/providers/mod.rs` (one `pub mod` line + one doc bullet)
- `src-tauri/src/bin/notchtap_agent.rs` (new subcommand, usage lines, and the `run_status` restructure in Step 4)
- `src/settings/hookEventParity.test.ts` (create)
- `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` (§4.1's usage block at `:314-319`)
- `docs/TESTING_STRATEGY.md` §0 (test-count recount, last step only)
- `plans/README.md` (status row)

**Out of scope** (do NOT touch, even though they look related):

- **Any file under the user's home directory.** `doctor` is strictly
  read-only. It must never create, edit, or repair a runtime's config
  file. Auto-installing hooks is a separate, unapproved decision.
- `src-tauri/src/agents/health.rs` and the Settings Agents section —
  surfacing doctor results in the GUI is deliberately deferred (see
  Maintenance notes). Do not add a `#[tauri::command]`.
- `src-tauri/src/agents/providers/{claude_code,codex,kimi}.rs` — the
  payload parsers. `doctor` inspects *config files*, not payloads.
- `adapters/*/README.md` — they carry the same snippets and currently
  agree with the TSX; keeping them in the parity test is deferred.
- `src-tauri/capabilities/default.json` — must never change.
- `src-tauri/build.rs` / `settings_commands.rs` — no new IPC command, so
  the seventeen-command parity must stay at seventeen.

## Git workflow

- Branch: `advisor/152-agent-setup-doctor`
- Conventional-commit style, matching `git log` (e.g.
  `feat(agents): notchtap-agent doctor — read-only hook wiring report`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Create `doctor.rs`, register it, and define the report types

**1a. Create** `src-tauri/src/agents/providers/doctor.rs` with a module
doc saying: this is the read-only setup-inspection half of
`notchtap-agent doctor`; it NEVER writes; every decision function is pure
(takes contents, a `bool`, or an explicit `&Path`, and resolves nothing
from the environment).

**1b. Register it.** In `src-tauri/src/agents/providers/mod.rs`, add
`pub mod doctor;` to the list at `:29-36`, in alphabetical position
(between `diagnostics` and `kimi`), and add a matching bullet to the
module doc above, in the same style as the existing ones:

```
//! - [`doctor`] — the read-only Adapter setup inspection behind
//!   `notchtap-agent doctor`: parses each runtime's hook config file and
//!   reports what is wired. Never writes.
```

**1c. Define the expected hook events.** Transcribe these exactly — the
order is the canonical output order, and Step 5 pins these lists against
the Settings snippets. Write one literal per line so a regex can extract
them reliably and `cargo fmt` will not reflow them onto one line:

```rust
/// The hook events `AgentsSection.tsx`'s Claude Code setup snippet
/// installs. Pinned against that file by
/// `src/settings/hookEventParity.test.ts`.
pub const CLAUDE_CODE_HOOK_EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "PostToolUse",
    "PostToolUseFailure",
    "SubagentStart",
    "SubagentStop",
];

pub const CODEX_HOOK_EVENTS: [&str; 8] = [
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "PreToolUse",
    "PostToolUse",
];

pub const KIMI_HOOK_EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "PostToolUse",
    "PostToolUseFailure",
    "SubagentStart",
    "SubagentStop",
];
```

**1d. Define the report types.** All of them derive
`Debug, Clone, PartialEq, Eq`:

```rust
/// What one runtime's Adapter installation looks like on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterInstall {
    /// The config file does not exist at the inspected path.
    ConfigMissing,
    /// The file exists but could not be read or parsed. `reason` is a
    /// bounded category (an `io::ErrorKind` debug name, or a fixed
    /// "malformed json"/"malformed toml" string) — NEVER a raw error
    /// string, which can embed the user's absolute home path.
    ConfigUnreadable { reason: String },
    /// Parsed. `wired` and `missing` are both in the canonical order of
    /// the corresponding `*_HOOK_EVENTS` const, never file order.
    Inspected {
        wired: Vec<String>,
        missing: Vec<String>,
        /// Every distinct command string found on a notchtap hook entry,
        /// first-seen order. Normally exactly one; more than one means a
        /// partially edited install, and each is reported separately.
        commands: Vec<String>,
    },
    /// OpenCode only: plugin-file presence, no hook list.
    PluginFile { present: bool },
}

/// What a hook's command string actually points at. The program is the
/// first whitespace-separated token of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTarget {
    /// Contains a `/`, and that path is an existing executable file.
    Resolved { path: PathBuf },
    /// Contains a `/`, but nothing executable is there — the failure
    /// mode that silently breaks every hook.
    Broken { path: PathBuf },
    /// A bare name (no `/`), which the provider must resolve via PATH.
    BareName { name: String, found_on_path: Option<PathBuf> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub runtime: &'static str,
    /// Already home-relative — produced by [`display_path`] before this
    /// struct is built, so [`render`] stays a pure formatter with no
    /// path logic and no `home` parameter.
    pub config_path_display: String,
    pub install: AdapterInstall,
    /// One entry per distinct command string in
    /// `AdapterInstall::Inspected.commands`, same order. Empty for every
    /// other `AdapterInstall` variant.
    pub command_targets: Vec<(String, CommandTarget)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub listener_ok: bool,
    pub port: u16,
    pub runtimes: Vec<RuntimeReport>,
    /// The Kimi hook-version line, pre-rendered by the caller from
    /// `kimi_version::probe_hook_support()`. `None` when not probed.
    pub kimi_note: Option<String>,
}
```

**Verify**: `cd src-tauri && cargo build --lib` exits 0 **and**
`grep -c 'pub mod doctor;' src-tauri/src/agents/providers/mod.rs` returns
`1`. Both must hold — the build alone passes on an unregistered file.

### Step 2: Write the four pure inspectors

Three take file contents; the fourth takes a `bool`.

```rust
pub fn inspect_claude_code(json: &str) -> AdapterInstall
pub fn inspect_codex(json: &str) -> AdapterInstall
pub fn inspect_kimi(toml_text: &str) -> AdapterInstall
/// OpenCode ships a plugin file, not hook entries, so presence is the
/// whole check. Taking a `bool` (not a path) keeps this pure — the
/// caller does the `is_file()`.
pub fn inspect_plugin_file(present: bool) -> AdapterInstall
```

Claude Code and Codex share the JSON shape, so implement one private
`fn inspect_hooks_json(json: &str, expected: &[&str], runtime_token: &str) -> AdapterInstall`
and have both public functions call it with their own const and token
(`"claude-code"` / `"codex"`).

Rules, all load-bearing:

- **Wired test**: a hook entry counts as wired when its `command` string
  **contains** the substring `hook <runtime_token>` (e.g. `hook kimi`).
  A substring match, not equality — users are expected to use an absolute
  path, so the real command looks like
  `/Users/x/.local/bin/notchtap-agent hook kimi`.
- **Ordering**: build `wired`/`missing` by iterating the `expected` const,
  never the file, so both are in canonical order regardless of how the
  user's file is ordered.
- **`commands`**: collect each distinct command string that matched, in
  first-seen order. Deduplicate.
- **Malformed JSON** → `ConfigUnreadable { reason: "malformed json".into() }`.
  **Malformed TOML** → `ConfigUnreadable { reason: "malformed toml".into() }`.
  Do not put the parser's own error text in `reason` — it can contain
  file paths.
- **Valid JSON with no `hooks` key**, or valid TOML with no `hooks`
  array → `Inspected` with everything missing and `commands` empty. This
  is *not* `ConfigUnreadable`: the file parsed fine, the hooks just
  aren't there.
- For Kimi, walk the `hooks` array of tables reading each table's `event`
  and `command`; a table missing either key is skipped without panicking.

**Verify**: `cd src-tauri && cargo build --lib` exits 0.

### Step 3: Command classification and path display

Still in `doctor.rs`. Both are pure; the filesystem is injected.

```rust
/// Classifies a hook command's program. `path_dirs` is `$PATH` already
/// split by the caller, and `exists_executable` is injected so this is
/// unit-testable without touching the filesystem.
pub fn classify_command(
    command: &str,
    path_dirs: &[PathBuf],
    exists_executable: &dyn Fn(&Path) -> bool,
) -> CommandTarget
```

Rules:

- Program = `command.split_whitespace().next()`. If there is no token
  (empty or whitespace-only command), return
  `BareName { name: String::new(), found_on_path: None }`. Do not panic.
- If the program contains `/`: it is a path. `Resolved { path }` when
  `exists_executable(path)` is true, else `Broken { path }`.
- Otherwise it is a bare name: return `BareName { name, found_on_path }`
  where `found_on_path` is the first `dir.join(&name)` in `path_dirs`
  for which `exists_executable` is true, else `None`.

```rust
/// Renders `path` with the user's home directory replaced by `~`, so no
/// absolute home path ever reaches the output. Falls back to the full
/// path when it is not under `home`.
pub fn display_path(path: &Path, home: &Path) -> String
```

Then one thin impure wrapper with no logic in it:

```rust
/// The real-filesystem executable predicate: an existing *file* with any
/// unix execute bit set. `is_file()` first — a directory with the execute
/// bit is not a program.
pub fn is_executable_file(p: &Path) -> bool

/// `$PATH` split into directories. Empty when `PATH` is unset.
pub fn path_dirs_from_env() -> Vec<PathBuf>
```

Use `std::os::unix::fs::PermissionsExt` and test `mode() & 0o111 != 0`.

**Verify**: `cd src-tauri && cargo build --lib` exits 0.

### Step 4: The renderer, the exit rule, and the listener helper

**4a. The listener check**, moved into `doctor.rs` so both subcommands
share it. It returns data and prints nothing:

```rust
/// `Ok(())` when something is listening on `127.0.0.1:port`, `Err(reason)`
/// otherwise. Impure (opens a socket), deliberately tiny, and prints
/// nothing so each caller can word its own output.
pub fn listener_reachable(port: u16) -> Result<(), String>
```

Move the body of `run_status`'s existing `match` scrutinee
(`notchtap_agent.rs:249-254`) into it verbatim — the `addr.parse()`,
the `map_err`, and the `TcpStream::connect_timeout(&sock,
Duration::from_millis(500))`. Then rewrite `run_status` to call it while
**keeping its two existing output strings byte-identical**:

```rust
    let listener_ok = match doctor::listener_reachable(port) {
        Ok(()) => {
            println!("notchtap: listening on 127.0.0.1:{port}");
            true
        }
        Err(e) => {
            println!("notchtap: not reachable on 127.0.0.1:{port} ({e})");
            false
        }
    };
```

`run_status`'s printed output must not change. That is the one thing to
check by eye here.

**4b. The exit rule**, pure and separate so it can be asserted alone:

```rust
/// FAILURE only when the listener is unreachable, or when NOT ONE runtime
/// shows any evidence of installation. A runtime the user does not use
/// must never fail the command.
///
/// "Evidence of installation" means: `Inspected` with a non-empty `wired`
/// list (a partial install still counts — 8/10 is wired-but-incomplete,
/// not un-wired), or `PluginFile { present: true }`. `ConfigMissing`,
/// `ConfigUnreadable`, `PluginFile { present: false }`, and `Inspected`
/// with an empty `wired` list are all "no evidence".
pub fn is_healthy(report: &DoctorReport) -> bool
```

**4c. The renderer**, pure — no printing, no clock, no filesystem, no
`home` parameter (paths arrive pre-formatted by `display_path`):

```rust
pub fn render(report: &DoctorReport) -> String
```

Produce exactly this shape. Every variant's wording is specified here;
do not invent any:

```text
notchtap doctor

listener   127.0.0.1:9789   reachable

claude-code   ~/.claude/settings.json
  10/10 hooks wired
  command: /Users/x/.local/bin/notchtap-agent hook claude-code -> resolved

codex   ~/.codex/hooks.json
  config file not found — this runtime is not wired

kimi   ~/.kimi-code/config.toml
  8/10 hooks wired
  missing: SubagentStart, SubagentStop
  command: notchtap-agent hook kimi -> NOT FOUND on PATH
  kimi 0.20.1 detected (hooks require >= 0.9.0) — supported

opencode   ~/.config/opencode/plugins/notchtap.ts
  plugin file present
```

Exact strings per variant:

- listener: `reachable` / `not reachable ({reason})`
- `Inspected`: `  {wired}/{total} hooks wired`, then — only when
  `missing` is non-empty — `  missing: {comma-joined}`
- `ConfigMissing`: `  config file not found — this runtime is not wired`
- `ConfigUnreadable`: `  config file unreadable ({reason})`
- `PluginFile { present: true }`: `  plugin file present`
- `PluginFile { present: false }`: `  plugin file not found — this runtime is not wired`
- one `  command: {cmd} -> {suffix}` line per entry in `command_targets`, where suffix is:
  - `Resolved` → `resolved`
  - `Broken` → `NOT FOUND at that path`
  - `BareName { found_on_path: Some(p) }` → `resolved via PATH ({p})`
  - `BareName { found_on_path: None }` → `NOT FOUND on PATH`
- `kimi_note`, when `Some`, is printed verbatim as its own indented line.

**Verify**: `cd src-tauri && cargo build --lib` exits 0 and
`cd src-tauri && cargo build --bin notchtap-agent` exits 0.

### Step 5: Wire the `doctor` subcommand

In `src-tauri/src/bin/notchtap_agent.rs`:

1. Add `Some("doctor") => run_doctor(),` to the `match` in `main`
   (`notchtap_agent.rs:42-50`), after the `status` arm.
2. Add `\n       notchtap-agent doctor` to `print_usage`'s string
   (`notchtap_agent.rs:53-57`) and a `notchtap-agent doctor` line to the
   ```` ```text ```` usage block at `notchtap_agent.rs:4-8`.
3. Write `run_doctor()` as a thin shell containing **no decision logic**:
   - `let home = dirs::home_dir()` — if `None`, print one line
     (`notchtap doctor: cannot resolve a home directory`) and return
     `ExitCode::FAILURE`.
   - `let port = delivery::resolve_port();`
     `let listener_ok = doctor::listener_reachable(port).is_ok();`
   - Per runtime, build the config path under `home`
     (`.claude/settings.json`, `.codex/hooks.json`,
     `.kimi-code/config.toml`, `.config/opencode/plugins/notchtap.ts`),
     then:
     - OpenCode: `doctor::inspect_plugin_file(path.is_file())`.
     - The other three: `std::fs::read_to_string(&path)` and map the
       result — `Ok(s)` → the matching inspector;
       `Err(e) if e.kind() == ErrorKind::NotFound` →
       `AdapterInstall::ConfigMissing`; any other `Err(e)` →
       `AdapterInstall::ConfigUnreadable { reason: format!("{:?}", e.kind()) }`.
       Using the `ErrorKind` debug name (not `e.to_string()`) keeps the
       user's absolute path out of the output.
   - Build `command_targets` by mapping each string in the install's
     `commands` through `doctor::classify_command(cmd,
     &doctor::path_dirs_from_env(), &doctor::is_executable_file)`.
   - `config_path_display` = `doctor::display_path(&path, &home)`.
   - `kimi_note`: only when the Kimi row was inspected, render from
     `kimi_version::probe_hook_support()`, reading
     `kimi_version::MINIMUM_HOOK_VERSION_STR` for the minimum — never a
     literal.
   - `println!("{}", doctor::render(&report))`, return `ExitCode::SUCCESS`
     when `doctor::is_healthy(&report)` else `ExitCode::FAILURE`.

If you find yourself writing an `if` about *wiring* in
`notchtap_agent.rs`, it belongs in `doctor.rs`. Mapping an `io::Error` to
a variant and calling `path.is_file()` are filesystem reads, not
decisions — those belong here.

**Verify**: all three:
- `cd src-tauri && cargo build --bin notchtap-agent` → exit 0
- `cd src-tauri && ./target/debug/notchtap-agent 2>&1 | grep -c doctor` → `1`
- `cd src-tauri && ./target/debug/notchtap-agent doctor; echo "exit=$?"` →
  prints a report beginning `notchtap doctor` and `exit=` is `0` or `1`
  (which one depends on this machine's state). An exit of `101` is a
  panic — that is a failure.

### Step 6: Pin the expected hook lists against the Settings snippets

The event lists now exist in two places: `doctor.rs`'s consts and
`AgentsSection.tsx`'s snippets. Add the pin rather than trusting hand-sync.

Create `src/settings/hookEventParity.test.ts`. Take **only the
path-resolution idiom** from `src/lib/sourceColors.test.ts:19-22`:

```ts
function readCss(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf-8");
}
```

**Do NOT copy that file's assertion style.** Its checks are whole-file
`expect(css).toContain(hex)` substring tests, which cannot bind a value
to the entry it belongs to. Yours must bind each event name to its
runtime.

The test must:

1. Read `src-tauri/src/agents/providers/doctor.rs` and
   `src/settings/sections/AgentsSection.tsx` as text.
2. **Isolate each region before extracting names.** The Claude Code and
   Kimi event sets are *identical*, so a whole-file scan cannot tell them
   apart. On the TSX side, slice from `const CLAUDE_CODE_SNIPPET =` to the
   closing backtick-semicolon, and likewise for `CODEX_SNIPPET` and
   `KIMI_SNIPPET`. On the Rust side, slice from
   `pub const CLAUDE_CODE_HOOK_EVENTS` to the closing `];`, and likewise
   for the other two.
3. Extract names from each slice: on the Rust side every
   double-quoted string; on the TSX side, for JSON snippets every key
   before `: [{`, and for the Kimi TOML snippet every `event = "..."`
   value.
4. Assert, per runtime, that the two arrays are **equal as sets** and
   that the length matches the expected count (10 / 8 / 10). That is
   three `describe`/`it` blocks with two assertions each — six
   assertions total.

**Verify**: `npx vitest run src/settings/hookEventParity.test.ts` → exit 0,
3 tests passed.

Then prove it is not vacuous: temporarily change one event name in
`doctor.rs` (e.g. `"SessionStart"` → `"SessionStarted"`), re-run the same
command, confirm it **fails**, then revert. Confirm the revert with
`cd src-tauri && git diff --stat src/agents/providers/doctor.rs` showing
your intended changes only, and re-run the command to confirm it passes
again.

### Step 7: Tests for `doctor.rs`

See "Test plan". Add them to an in-file `#[cfg(test)] mod tests`.

**Verify**: `cd src-tauri && cargo test --locked doctor 2>&1 | grep 'test result'`
→ a line containing `22 passed`.

### Step 8: Docs and gates

1. In `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` §4.1, add
   `notchtap-agent doctor` to the ```` ```text ```` usage block at
   `:314-319`, and one sentence after the block: read-only, inspects the
   four runtimes' config files, never writes.
2. Recount `docs/TESTING_STRATEGY.md` §0 from live runs of both suites.
   This is the **only** place test counts live in this repo.
3. Run the full gate set.

**Verify**: all of
- `cd src-tauri && cargo fmt --check` → exit 0
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo test --locked` → `0 failed`
- `npx vitest run` → all pass
- `npx tsc --noEmit` → exit 0
- `npx biome ci .` → exit 0

## Test plan

Twenty-two tests in `doctor.rs`'s `mod tests`, modelled on
`kimi_version.rs`'s test module (pure functions, table of cases, no
filesystem). The count is pinned by Step 7's verify — if you write a
different number, update Step 7 and the Done criteria to match and say so
in your report.

Inspection, JSON (1–7):
1. All ten Claude Code events wired → `Inspected`, `missing` empty,
   `wired.len() == 10`.
2. Two events removed → exactly those two in `missing`, **in canonical
   const order**, not file order.
3. A hook entry whose command lacks `hook claude-code` → that event
   counts as missing.
4. An absolute-path command → still wired, and the absolute string
   appears in `commands`.
5. `inspect_claude_code` on a file wired for `hook codex` → nothing wired.
6. Malformed JSON → `ConfigUnreadable { reason: "malformed json" }`.
7. Valid JSON with no `hooks` key → `Inspected`, everything missing (not
   `ConfigUnreadable`).

Inspection, Codex + TOML (8–12):
8. `inspect_codex` with all eight wired → nothing missing (proves the
   shared helper is parameterised, not hardcoded to Claude Code).
9. All ten Kimi `[[hooks]]` tables → nothing missing.
10. A `[[hooks]]` table missing its `command` key → that event missing,
    no panic.
11. Malformed TOML → `ConfigUnreadable { reason: "malformed toml" }`.
12. A `config.toml` carrying unrelated Kimi settings alongside the hooks
    → parses and reports correctly.

`classify_command` (13–18), all with an injected `exists_executable`:
13. Absolute path, exists + executable → `Resolved`.
14. Absolute path, does not exist → `Broken`.
15. Absolute path, exists but not executable → `Broken`.
16. Bare name found in one of `path_dirs` → `BareName { found_on_path: Some }`.
17. Bare name in none of `path_dirs` → `BareName { found_on_path: None }`.
18. Empty command → `BareName { name: "", found_on_path: None }`, no panic.

`display_path` and `inspect_plugin_file` (19–20):
19. A path under `home` → starts with `~/`; a path outside `home` → returned
    whole.
20. `inspect_plugin_file(true)` / `(false)` → the two `PluginFile` variants.

`render` and `is_healthy` (21–22):
21. `render` of a report with one fully-wired runtime contains
    `10/10 hooks wired`, contains `~/`, and does **not** contain the
    string `/Users/`.
22. `is_healthy` across four cases in one test: listener down + all wired
    → false; listener up + zero evidence → false; listener up + one
    runtime at 8/10 + three `ConfigMissing` → **true**; listener up +
    only `PluginFile { present: true }` → **true**.

Frontend: `src/settings/hookEventParity.test.ts` — 3 tests, 6 assertions,
per Step 6.

## Done criteria

ALL must hold. Every line is a command with an expected result:

- [ ] `cd src-tauri && cargo test --locked` → `0 failed`
- [ ] `cd src-tauri && cargo test --locked doctor 2>&1 | grep -c '22 passed'` → `1`
- [ ] `cd src-tauri && cargo fmt --check` → exit 0
- [ ] `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- [ ] `npx vitest run` → exit 0
- [ ] `npx vitest run src/settings/hookEventParity.test.ts 2>&1 | grep -c '3 passed'` → `1`
- [ ] `npx tsc --noEmit` → exit 0
- [ ] `npx biome ci .` → exit 0
- [ ] `grep -c 'pub mod doctor;' src-tauri/src/agents/providers/mod.rs` → `1`
- [ ] `cd src-tauri && ./target/debug/notchtap-agent 2>&1 | grep -c doctor` → `1`
- [ ] `cd src-tauri && ./target/debug/notchtap-agent doctor >/dev/null 2>&1; test $? -le 1 && echo ok` → `ok` (exit 0 or 1; 101 means a panic)
- [ ] `grep -rn '1\.2\.0\|1\.4\.0' src-tauri/src/agents/providers/doctor.rs` → no matches (no hardcoded version numbers)
- [ ] `git diff --stat src-tauri/capabilities/` → empty
- [ ] `grep -r '#\[tauri::command\]' src-tauri/src | wc -l` → `17`
- [ ] `git status --porcelain` lists only files from the In-scope list
- [ ] `docs/TESTING_STRATEGY.md` §0 recounted from a live run
- [ ] `plans/README.md` status row updated (skip if your reviewer maintains the index)

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the
  excerpts — in particular `notchtap_agent.rs`'s `main` match,
  `run_status`'s body, `providers/mod.rs`'s module list, or
  `AgentsSection.tsx`'s snippet constants.
- The hook-event lists in `AgentsSection.tsx` disagree with Step 1c's
  consts. The TSX file wins; report the difference rather than silently
  encoding either version.
- `MINIMUM_HOOK_VERSION_STR` is not `"0.9.0"` — report the real value;
  do not hardcode either one.
- You conclude that inspecting a config file requires *writing* to the
  user's home directory for any reason.
- You conclude the feature needs a new `#[tauri::command]`. It does not;
  that would break the seventeen-command security parity.
- Step 6's non-vacuity check does not fail when you break a const name —
  that means the parity test is not actually binding names to runtimes.
- Any gate fails twice after a reasonable fix attempt.

## Maintenance notes

- **A fifth runtime means five edits**: the `*_HOOK_EVENTS` const, the
  path map in `run_doctor`, `AgentsSection.tsx`'s snippet, that runtime's
  `adapters/<name>/README.md`, and the parity test's runtime list.
  `AgentsSection.tsx:26-33` records that the READMEs are upstream of the
  TSX snippets — this plan pins TSX↔Rust only, so the README remains a
  hand-synced third copy. Extending the pin to it is a reasonable
  follow-up.
- **Do not weaken Step 6's test into a whole-file substring check.** The
  Claude Code and Kimi event sets are identical, so a substring check
  would pass even if the two were swapped.
- **Deliberately deferred**: surfacing doctor's results in the Settings
  window. That needs a new `#[tauri::command]`, hence a coordinated
  4-place change (`settings_commands.rs` + its count assertion,
  `lib.rs`'s `generate_handler!`, `capabilities/settings.json`,
  `build.rs`). Worth doing later; kept out to keep this plan low-risk.
- **Also deferred**: per-project config files (`./.claude/settings.json`,
  `./.opencode/plugins/`). `doctor` checks global paths only. If a user
  reports "doctor says not wired but it works", a project-local install
  is the first thing to check.
- **A reviewer should scrutinise**: that nothing in `doctor.rs` opens a
  file for writing; that no test resolves the real `$HOME`; that
  `is_executable_file` requires `is_file()` before checking mode bits;
  and that `run_status`'s printed output is byte-identical to before.
