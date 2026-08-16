import { invoke } from "@tauri-apps/api/core";
import type {
  AboutInfo,
  AdapterHealthDto,
  AgentWireRuntime,
  Config,
  HistoryEntry,
  QueueItemSummary,
  TestSource,
} from "./types";

type UnparsedValue = string | number | boolean | null | UnparsedObject | UnparsedValue[];
type UnparsedObject = { [key: string]: UnparsedValue };

// Typed mirror of the rust command allowlist (src-tauri/build.rs +
// capabilities/settings.json): the settings window is the only invoker; the
// overlay is receive-only. This map grants nothing — it only types the allowlist.
export interface SettingsCommands {
  clear_history: { args: undefined; result: null };
  clear_queue: { args: undefined; result: number };
  get_about_info: { args: undefined; result: AboutInfo };
  get_agent_health: { args: undefined; result: AdapterHealthDto[] };
  get_config: { args: undefined; result: Config };
  get_default_config: { args: undefined; result: Config };
  get_history: { args: undefined; result: HistoryEntry[] };
  get_queue: { args: undefined; result: QueueItemSummary[] };
  get_recent_log_lines: { args: undefined; result: string[] };
  save_config_and_relaunch: { args: { config: Config }; result: null };
  search_news_now: { args: { query: string }; result: number };
  send_agent_test_event: { args: { runtime: AgentWireRuntime }; result: null };
  send_test_notification: { args: { source: TestSource }; result: null };
  set_appearance: { args: { scale: number; radius: number; opacity: number }; result: null };
  skip_current: { args: undefined; result: null };
}

export function settingsInvoke<C extends keyof SettingsCommands>(
  command: C,
  ...args: SettingsCommands[C]["args"] extends undefined ? [] : [SettingsCommands[C]["args"]]
): Promise<SettingsCommands[C]["result"]> {
  return invoke<SettingsCommands[C]["result"]>(
    command,
    // SAFETY: args[0] is a statically-typed SettingsCommands arg — already proven to hold its allowlist shape; the cast restores the dictionary view invoke expects.
    args[0] as UnparsedObject | undefined,
  );
}
