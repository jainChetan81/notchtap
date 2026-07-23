import { NumberControl, SettingsGroup, TestButtonRow } from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import type { Config } from "../types";
import { PRIORITY_SEGMENT_OPTIONS } from "../types";

export function CmuxSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  return (
    <SettingsGroup
      title="Cmux relay"
      description="Cmux's notification command already calls the notchtap CLI, which auto-detects relayed pushes through CMUX_NOTIFICATION_BODY. Nothing needs enabling here; these controls set how relayed notifications are promoted and rotated."
    >
      <Segmented
        id="cmux-priority"
        name="Priority"
        help="Which tier a waiting cmux-relayed notification promotes in. Only applies when the request omits its own priority — cmux's built-in notification-command setting currently always passes --priority high explicitly, which overrides this. Drop that flag from cmux's own settings (not this app) to let this control take effect."
        options={PRIORITY_SEGMENT_OPTIONS}
        value={config.cmux_priority}
        onChange={(cmux_priority) => patchConfig({ cmux_priority })}
      />
      <NumberControl
        id="cmux-ttl-secs"
        name="Rotation seconds"
        help="How long a cmux-relayed notification occupies the slot once shown."
        value={config.cmux_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(cmux_ttl_secs) => patchConfig({ cmux_ttl_secs })}
      />
      <TestButtonRow
        name="Test cmux notification"
        help="Send a one-off cmux notification to the overlay."
        source="cmux"
      />
    </SettingsGroup>
  );
}
