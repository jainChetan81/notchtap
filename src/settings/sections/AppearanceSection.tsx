import type { CSSProperties } from "react";
import { StatusRailCard } from "../../components/StatusRailCard";
import { SettingsGroup, TestButtonRow } from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import { PREVIEW_SAMPLES } from "../previewFixtures";
import type { AppearanceConfig, Config } from "../types";

export function AppearanceSection({
  config,
  patchConfig,
  applyAppearanceLive,
}: {
  config: Config;
  patchConfig: (patch: Partial<Config>) => void;
  // Owned by SettingsApp, not this component (plan 108 step A): this
  // section may be unmounted while another section is open — but
  // Reset/Reset to defaults are footer buttons, clickable from any
  // section. So the function lives one level up, and its status renders in
  // the footer (always visible), not here — see the settings-footer JSX.
  applyAppearanceLive: (scale: number, radius: number, opacity: number) => void;
}) {
  const { card_scale: scale, card_radius: radius, card_opacity: opacity } = config.appearance;

  function updateAppearance(partial: Partial<AppearanceConfig>) {
    const next = { ...config.appearance, ...partial };
    applyAppearanceLive(next.card_scale, next.card_radius, next.card_opacity);
    patchConfig({ appearance: next });
  }

  function updateScale(next: number) {
    updateAppearance({ card_scale: next });
  }

  function updateRadius(next: number) {
    updateAppearance({ card_radius: next });
  }

  function updateOpacity(next: number) {
    updateAppearance({ card_opacity: next });
  }

  const previewStyle: CSSProperties = {
    "--card-scale": scale,
    "--card-radius": `${radius}px`,
    "--card-opacity": opacity,
  } as CSSProperties;

  return (
    <div className="section-stack">
      <SettingsGroup
        title="Card shape"
        description="Adjust the overlay card size, corner radius, and opacity. Changes apply immediately."
      >
        <Segmented
          name="Scale"
          options={[
            { label: "Small", value: 0.85 },
            { label: "Medium", value: 1.0 },
            { label: "Large", value: 1.15 },
          ]}
          value={scale}
          onChange={updateScale}
        />
        <Segmented
          name="Radius"
          options={[
            { label: "Square", value: 0 },
            { label: "Soft", value: 8 },
            { label: "Round", value: 16 },
          ]}
          value={radius}
          onChange={updateRadius}
        />
        <Segmented
          name="Opacity"
          options={[
            { label: "Glass", value: 0.7 },
            { label: "Default", value: 0.9 },
            { label: "Solid", value: 1.0 },
          ]}
          value={opacity}
          onChange={updateOpacity}
        />
        <TestButtonRow
          name="Live check"
          help="Send a one-off manual notification to the overlay."
          source="manual"
        />
      </SettingsGroup>

      <SettingsGroup
        title="Overlay animations"
        description="These are the built-in card styles the overlay renders. The preview reflects the shape settings above."
      >
        <div className="appearance-preview" style={previewStyle}>
          {PREVIEW_SAMPLES.map(({ label, slot }) => (
            <div className="preview-row" key={slot.id}>
              <div className="preview-label">{label}</div>
              {/* plan 111: `.card-root` scopes the shared card-shape
                  stylesheet (overlay-card.css) — each sample gets its OWN
                  scope (one wrapper per card, matching the overlay's
                  one-wrapper-per-card shape), and `.preview-stage` is
                  already the per-sample frame box, so the scope class
                  composes onto it rather than adding a further nested
                  element. `.appearance-preview` itself stays frame chrome
                  only (settings.css) — never the scope host. */}
              <div className="preview-stage card-root">
                <StatusRailCard slot={slot} />
              </div>
            </div>
          ))}
        </div>
      </SettingsGroup>
    </div>
  );
}
