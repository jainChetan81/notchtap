import { NumberControl, SettingsGroup, TestButtonRow, ToggleControl } from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import type { Config } from "../types";
import { PRIORITY_SEGMENT_OPTIONS, UNITS_SEGMENT_OPTIONS } from "../types";

export function WeatherSection({
  config,
  patchConfig,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
}) {
  return (
    <SettingsGroup
      title="Weather"
      description="Keyless Open-Meteo polling — set your coordinates once. The idle rail shows current conditions; rain and temperature thresholds send alert cards."
    >
      <ToggleControl
        id="weather-enabled"
        name="Weather"
        help="Poll Open-Meteo for current conditions and threshold alerts."
        label="Enable weather"
        checked={config.weather_enabled}
        onChange={(weather_enabled) => patchConfig({ weather_enabled })}
      />
      <NumberControl
        id="weather-lat"
        name="Latitude"
        help="Decimal degrees, e.g. 12.97 for Bangalore."
        value={config.weather_lat}
        min={-90}
        max={90}
        step="any"
        onChange={(weather_lat) => patchConfig({ weather_lat })}
      />
      <NumberControl
        id="weather-lon"
        name="Longitude"
        help="Decimal degrees, e.g. 77.59 for Bangalore."
        value={config.weather_lon}
        min={-180}
        max={180}
        step="any"
        onChange={(weather_lon) => patchConfig({ weather_lon })}
      />
      <Segmented
        id="weather-units"
        name="Units"
        help="Display units for the idle chip. Alert thresholds below are always in Celsius."
        options={UNITS_SEGMENT_OPTIONS}
        value={config.weather_units}
        onChange={(weather_units) => patchConfig({ weather_units })}
      />
      <NumberControl
        id="weather-poll-secs"
        name="Poll interval"
        help="How often conditions are refreshed."
        value={config.weather_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(weather_poll_secs) => patchConfig({ weather_poll_secs })}
      />
      <NumberControl
        id="weather-rain-threshold-pct"
        name="Rain threshold"
        help="Alert when the chance of rain reaches this."
        value={config.weather_rain_threshold_pct}
        min={0}
        max={100}
        unit="%"
        onChange={(weather_rain_threshold_pct) => patchConfig({ weather_rain_threshold_pct })}
      />
      <NumberControl
        id="weather-rain-lookahead-mins"
        name="Rain lookahead"
        help="How far ahead the rain check looks."
        value={config.weather_rain_lookahead_mins}
        min={5}
        max={120}
        unit="MIN"
        onChange={(weather_rain_lookahead_mins) => patchConfig({ weather_rain_lookahead_mins })}
      />
      <NumberControl
        id="weather-temp-hot-c"
        name="Hot threshold"
        help="Alert when the temperature reaches this, always in Celsius."
        value={config.weather_temp_hot_c}
        min={-50}
        max={60}
        unit="°C"
        onChange={(weather_temp_hot_c) => patchConfig({ weather_temp_hot_c })}
      />
      <NumberControl
        id="weather-temp-cold-c"
        name="Cold threshold"
        help="Alert when the temperature drops to this, always in Celsius."
        value={config.weather_temp_cold_c}
        min={-50}
        max={60}
        unit="°C"
        onChange={(weather_temp_cold_c) => patchConfig({ weather_temp_cold_c })}
      />
      <Segmented
        id="weather-priority"
        name="Priority"
        help="Which tier a waiting weather alert promotes in."
        options={PRIORITY_SEGMENT_OPTIONS}
        value={config.weather_priority}
        onChange={(weather_priority) => patchConfig({ weather_priority })}
      />
      <TestButtonRow
        name="Test weather notification"
        help="Send a one-off weather alert to the overlay."
        source="weather"
      />
    </SettingsGroup>
  );
}
