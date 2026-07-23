import {
  NumberControl,
  SettingsGroup,
  TestButtonRow,
  TextareaControl,
  ToggleControl,
} from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import type { Config } from "../types";
import { PRIORITY_SEGMENT_OPTIONS } from "../types";

export function FootballSection({
  config,
  leaguesText,
  patchConfig,
  setLeaguesText,
}: {
  config: Config;
  leaguesText: string;
  patchConfig: (patch: Partial<Config>) => void;
  setLeaguesText: (value: string) => void;
}) {
  return (
    <SettingsGroup title="Score polling">
      <ToggleControl
        id="espn-enabled"
        name="ESPN scores"
        help="Poll watched leagues for score and match-state changes."
        label="Enable ESPN scores"
        checked={config.espn_enabled}
        onChange={(espn_enabled) => patchConfig({ espn_enabled })}
      />
      <TextareaControl
        id="espn-leagues"
        name="Leagues"
        help="Use one ESPN league code per line."
        value={leaguesText}
        caption="one league code per line"
        onChange={setLeaguesText}
      />
      <NumberControl
        id="espn-poll-secs"
        name="Poll interval"
        help="How often enabled leagues are checked."
        value={config.espn_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(espn_poll_secs) => patchConfig({ espn_poll_secs })}
      />
      <NumberControl
        id="espn-ttl-secs"
        name="Rotation seconds"
        help="How long a score card occupies the slot once shown."
        value={config.espn_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(espn_ttl_secs) => patchConfig({ espn_ttl_secs })}
      />
      <ToggleControl
        id="espn-live-card"
        name="Live match card"
        help="Show one live match as a single updating card instead of a burst of one-shot cards."
        label="Consolidate live match updates"
        checked={config.espn_live_card}
        onChange={(espn_live_card) => patchConfig({ espn_live_card })}
      />
      <ToggleControl
        id="espn-rich-events"
        name="Richer match events"
        help="Poll for fouls, offsides, VAR checks, and substitutions in addition to goals and cards. Heavier polling — opt in per match."
        label="Show richer match events"
        checked={config.espn_rich_events}
        onChange={(espn_rich_events) => patchConfig({ espn_rich_events })}
      />
      <Segmented
        id="espn-priority"
        name="Priority"
        help="Which tier a waiting score/match-state update promotes in."
        options={PRIORITY_SEGMENT_OPTIONS}
        value={config.espn_priority}
        onChange={(espn_priority) => patchConfig({ espn_priority })}
      />
      <TestButtonRow
        name="Test football notification"
        help="Send a one-off football notification to the overlay."
        source="football"
      />
    </SettingsGroup>
  );
}
