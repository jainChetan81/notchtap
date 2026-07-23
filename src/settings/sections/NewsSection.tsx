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

export function NewsSection({
  config,
  feedsText,
  patchConfig,
  setFeedsText,
}: {
  config: Config;
  feedsText: string;
  patchConfig: (patch: Partial<Config>) => void;
  setFeedsText: (value: string) => void;
}) {
  return (
    <SettingsGroup title="RSS polling">
      <ToggleControl
        id="rss-enabled"
        name="RSS news"
        help="Poll configured feeds and rotate fresh headlines through the slot."
        label="Enable RSS news"
        checked={config.rss_enabled}
        onChange={(rss_enabled) => patchConfig({ rss_enabled })}
      />
      <TextareaControl
        id="rss-feeds"
        name="Feeds"
        help="Use one complete HTTP(S) feed URL per line."
        value={feedsText}
        caption="one feed URL per line"
        onChange={setFeedsText}
      />
      <NumberControl
        id="rss-poll-secs"
        name="Poll interval"
        help="How often configured feeds are checked."
        value={config.rss_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(rss_poll_secs) => patchConfig({ rss_poll_secs })}
      />
      <NumberControl
        id="rss-ttl-secs"
        name="Headline rotation"
        help="How long each headline occupies the slot."
        value={config.rss_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(rss_ttl_secs) => patchConfig({ rss_ttl_secs })}
      />
      <NumberControl
        id="rss-max-per-poll"
        name="Maximum per poll"
        help="New headlines accepted from a single poll pass."
        value={config.rss_max_per_poll}
        min={1}
        max={100}
        unit="ITEMS"
        onChange={(rss_max_per_poll) => patchConfig({ rss_max_per_poll })}
      />
      <Segmented
        id="rss-priority"
        name="Priority"
        help="Which tier a waiting headline promotes in."
        options={PRIORITY_SEGMENT_OPTIONS}
        value={config.rss_priority}
        onChange={(rss_priority) => patchConfig({ rss_priority })}
      />
      <TestButtonRow
        name="Test news notification"
        help="Send a one-off news headline to the overlay."
        source="news"
      />
    </SettingsGroup>
  );
}
