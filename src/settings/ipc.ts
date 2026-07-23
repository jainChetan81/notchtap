import { invoke } from "@tauri-apps/api/core";
import type {
  Config,
  ConnectorHealthDto,
  HistoryEntry,
  QueueItemSummary,
  SecretField,
  SecretStatus,
  TestSource,
} from "./types";

// Typed name → { args, result } map of every #[tauri::command] the
// settings window calls (plan 119) — the TS-side mirror of the
// security-load-bearing allowlist in src-tauri/build.rs
// (AppManifest::commands) plus capabilities/settings.json (CLAUDE.md
// "ipc & security"). Keep the lists in lockstep: a command added on the
// rust side must be added BOTH to build.rs (or it silently becomes
// callable from the overlay window) and here (or the settings window
// can't call it through settingsInvoke); a command named here but absent
// from build.rs fails at runtime with a permission error, it does not
// widen anything. This map grants nothing — it only types what the
// capability files already allow.
export interface SettingsCommands {
  clear_history: { args: undefined; result: null };
  clear_queue: { args: undefined; result: number };
  get_config: { args: undefined; result: Config };
  get_connector_health: { args: undefined; result: ConnectorHealthDto };
  get_default_config: { args: undefined; result: Config };
  get_history: { args: undefined; result: HistoryEntry[] };
  get_queue: { args: undefined; result: QueueItemSummary[] };
  get_recent_log_lines: { args: undefined; result: string[] };
  get_secret_status: { args: undefined; result: SecretStatus };
  save_config_and_relaunch: { args: { config: Config }; result: null };
  search_news_now: { args: { query: string }; result: number };
  send_test_notification: { args: { source: TestSource }; result: null };
  set_appearance: { args: { scale: number; radius: number; opacity: number }; result: null };
  set_secret: { args: { field: SecretField; value: string }; result: null };
  skip_current: { args: undefined; result: null };
}

export function settingsInvoke<C extends keyof SettingsCommands>(
  command: C,
  ...args: SettingsCommands[C]["args"] extends undefined ? [] : [SettingsCommands[C]["args"]]
): Promise<SettingsCommands[C]["result"]> {
  return invoke<SettingsCommands[C]["result"]>(
    command,
    args[0] as Record<string, unknown> | undefined,
  );
}
