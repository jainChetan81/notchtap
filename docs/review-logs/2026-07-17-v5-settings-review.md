# review log — v5 settings backend security review — 2026-07-17

**setup**: two-reviewer panel over the rust-side v5 settings backend and
its ipc/acl posture. claude-sonnet-5 reviewed from a neutral senior
rust/tauri stance; openai/gpt-5.6-sol reviewed adversarially. the
frontend settings page was deliberately out of scope.

reviewed by: anthropic/claude-sonnet-5 (neutral) +
openai/gpt-5.6-sol (adversarial) — both completed.

## reviewer verdicts

**claude (neutral)**: **ship with minor fixes**. three low-severity
findings:

1. **low — label-gate regression gap**: `ensure_settings_window` was the
   defense-in-depth backstop but had no direct test.
2. **low — clipboard whitespace**: secret values were validated without
   trimming, so a normal paste with a trailing newline was rejected.
3. **low — `detect_path` trusted from ipc**: the ui would hide the field,
   but `save_config_and_relaunch` still accepted it in the submitted
   `Config`; the review explicitly noted the locked csp kept the practical
   risk low.

**codex (adversarial, gpt-5.6-sol)**: **needs rework**. six findings:

1. **high — malformed toml could disclose full secrets**: formatting
   `toml::de::Error` could echo the offending source line across ipc and
   into connector boot logs.
2. **high — fixed temp name broke the `0600` guarantee**: opening a stale
   permissive `secrets.toml.tmp` meant creation-time mode did not apply
   before secret content was written.
3. **medium — unlocked secrets read-modify-write**: concurrent updates
   could both read the old document and lose one field on write.
4. **medium — unknown secrets content was clobbered**: serde dropped
   unmodelled tables and fields before the whole document was rewritten.
5. **medium — `detect_path` was writable through ipc**: the file-only
   subprocess path was enforced only by the future ui, not by the rust
   boundary.
6. **low — rss url validation was only a prefix check**: structurally
   invalid urls could save successfully and then fail on every poll.

## synthesis

**verdict: fix before test.** the acl chain itself was sound — build-time
command opt-in, settings-only capability, and the caller-derived window
label check all aligned — but the two high-severity secrets findings and
the lost-update/clobber risks had to close before the settings frontend
exercised the surface.

## action taken

all seven test-pinned fix items landed the same day:

1. malformed parse detail is withheld in both settings-facing and
   connector-display errors —
   `malformed_secrets_error_never_echoes_secret_material` and
   `malformed_secrets_display_never_echoes_secret_material`.
2. atomic writes use unique same-directory temp names opened with
   `create_new(true)`; secrets start at `0600`, and a stale permissive
   fixed-name file is untouched —
   `stale_permissive_tmp_files_are_never_written_into`.
3. serde-flattened extra maps preserve unknown tables and fields —
   `unknown_tables_and_fields_survive_a_secret_write`.
4. secret values are trimmed before validation and storage —
   `secret_values_are_trimmed_before_validation_and_storage`.
5. `pin_uneditable_fields` restores the booted `detect_path` before
   validation and write — `detect_path_is_pinned_to_the_booted_value`.
6. the label backstop is exercised with `tauri::test::mock_app()` and
   `WebviewWindowBuilder` —
   `ensure_settings_window_gates_on_the_window_label`.
7. rss feeds must fully parse with an http(s) scheme and real host —
   `rss_feeds_require_a_real_parsed_host_not_just_a_prefix`.

the full secrets load→merge→write transaction is also serialized by the
in-process `SECRETS_LOCK`, closing the concurrent-invoke lost-update
finding. the manual devtools check that the generated tauri acl denies
`invoke("get_config")` from the `main` window remains in
`IMPLEMENTATION_PLAN.md` §6; the mock label test is defense-in-depth, not
a replacement for that runtime acl check.
