import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  type Config,
  type HistoryEntry,
  type HistoryRotationSpec,
  type PriorityLevel,
  type QueueItemSummary,
  type SecretStatus,
  SettingsApp,
} from "./SettingsApp";

// plan 112 Step 4: jsdom has no ResizeObserver; the shadcn Switch
// (radix-ui's useSize hook, used to size its thumb) reads one on mount.
// A no-op stub is enough — nothing in this suite asserts on a resize
// callback, only on rendered DOM/ARIA state.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
// biome-ignore lint/suspicious/noExplicitAny: test-environment polyfill assignment, not app code.
(globalThis as any).ResizeObserver ??= ResizeObserverStub;

// Several plan-108 tests drive setInterval/setTimeout-based status transitions
// (connector-health poll, ok-message auto-clear) with fake timers. Those
// timers must be created UNDER the fake clock, so fake timers are engaged
// before render — which means we can't rely on RTL's findBy/waitFor (their
// internal polling assumes real timers). This flushes the microtask queue
// enough times to drain invoke() promise chains and any resulting state
// updates, entirely independent of the timer fake/real state.
async function flush(times = 6) {
  for (let i = 0; i < times; i++) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}

// plan 112 Step 4: the Switch contract change (native checkbox ->
// shadcn/radix Switch) moves the on/off signal from
// `HTMLInputElement.checked` to `aria-checked` on a real `<button
// role="switch">` — this reads that attribute instead, everywhere a
// test used to read `.checked` on a toggle.
function isChecked(element: HTMLElement): boolean {
  return element.getAttribute("aria-checked") === "true";
}

const config: Config = {
  port: 4321,
  default_ttl: 14,
  max_queued_per_tier: 75,
  detect_path: "/opt/notchtap-detect",
  start_paused: true,
  espn_enabled: false,
  espn_leagues: ["eng.1", "usa.1"],
  espn_poll_secs: 45,
  espn_priority: "medium",
  espn_ttl_secs: 22,
  espn_live_card: false,
  espn_rich_events: false,
  rss_enabled: true,
  rss_feeds: [
    {
      url: "https://example.com/world.xml",
      source: "Example",
      category: "world",
    },
    { url: "https://example.com/tech.xml", source: null, category: null },
  ],
  rss_topics: ["aston villa transfers"],
  rss_poll_secs: 90,
  rss_priority: "high",
  rss_ttl_secs: 18,
  rss_max_per_poll: 6,
  manual_default_priority: "low",
  cmux_priority: "medium",
  cmux_ttl_secs: 16,
  weather_enabled: true,
  weather_lat: 12.97,
  weather_lon: 77.59,
  weather_units: "celsius",
  weather_poll_secs: 900,
  weather_rain_threshold_pct: 60,
  weather_rain_lookahead_mins: 30,
  weather_temp_hot_c: 36,
  weather_temp_cold_c: 14,
  weather_priority: "medium",
  rotation_order: ["news", "cmux", "manual", "weather", "football"],
  connectors: { telegram: { enabled: true } },
  appearance: { card_scale: 1, card_radius: 8, card_opacity: 0.9 },
  resting_state: "notch",
  history_enabled: true,
  now_playing_enabled: true,
};

// Mirrors src-tauri/src/config.rs::Config::default() (served over IPC by
// get_default_config, plan 020) — the fixture the "Reset to defaults" test
// asserts concrete values against (port 9789, ttl 8, tier cap 50, ...).
const rustConfigDefaults: Config = {
  port: 9789,
  default_ttl: 8,
  max_queued_per_tier: 50,
  detect_path: "/usr/local/bin/notchtap-detect",
  start_paused: false,
  espn_enabled: true,
  espn_leagues: ["eng.1", "uefa.champions", "esp.1"],
  espn_poll_secs: 30,
  espn_priority: "high",
  espn_ttl_secs: 8,
  espn_live_card: false,
  espn_rich_events: false,
  rss_enabled: false,
  rss_feeds: [
    {
      url: "https://feeds.feedburner.com/ndtvnews-top-stories",
      source: "NDTV",
      category: null,
    },
  ],
  rss_topics: [],
  rss_poll_secs: 60,
  rss_priority: "low",
  rss_ttl_secs: 10,
  rss_max_per_poll: 10,
  manual_default_priority: "medium",
  cmux_priority: "high",
  cmux_ttl_secs: 8,
  weather_enabled: false,
  weather_lat: 0,
  weather_lon: 0,
  weather_units: "celsius",
  weather_poll_secs: 900,
  weather_rain_threshold_pct: 60,
  weather_rain_lookahead_mins: 30,
  weather_temp_hot_c: 36,
  weather_temp_cold_c: 14,
  weather_priority: "medium",
  rotation_order: ["football", "manual", "weather", "cmux", "news"],
  connectors: { telegram: { enabled: false } },
  appearance: { card_scale: 1, card_radius: 16, card_opacity: 0.9 },
  resting_state: "rail",
  history_enabled: false,
  now_playing_enabled: false,
};

const unsetSecrets: SecretStatus = {
  openrouter_api_key: null,
  telegram_bot_token: null,
  telegram_chat_id: null,
};

// get_history's wire shape (plan 089) — snake_case throughout, including
// `meta`; pinned against a live serde_json print of a real HistoryEntry
// rather than derived from the SlotState (camelCase) convention. 088's
// read_recent returns oldest -> newest; these two fixtures are ordered
// that way so the "newest first" display test can assert the UI does the
// reversal, not the mock data.
const historyEntryOlder: HistoryEntry = {
  recorded_at_ms: 1700000000000,
  event: {
    id: "11111111-1111-1111-1111-111111111111",
    event_type: "generic",
    priority: "medium",
    rotation: { kind: "one_shot", ttl_secs: 8 },
    topic: null,
    payload: { title: "First notification", body: "body one" },
    meta: {
      source: null,
      category: null,
      published_at_ms: null,
      link: null,
      subtitle: null,
      details: [],
    },
    signal: "generic",
    origin: "manual",
  },
};

const historyEntryNewer: HistoryEntry = {
  recorded_at_ms: 1700000100000,
  event: {
    id: "22222222-2222-2222-2222-222222222222",
    event_type: "news_item",
    priority: "low",
    rotation: { kind: "one_shot", ttl_secs: 10 },
    topic: null,
    payload: { title: "Second notification", body: "body two" },
    meta: {
      source: "Example",
      category: null,
      published_at_ms: null,
      link: null,
      subtitle: null,
      details: [],
    },
    signal: "generic",
    origin: "news",
  },
};

// plan 110 (Step A): every optional field populated, to exercise the
// metadata chips + the expandable <details> block together.
const historyEntryFullMeta: HistoryEntry = {
  recorded_at_ms: 1700000200000,
  event: {
    id: "33333333-3333-3333-3333-333333333333",
    event_type: "score_update",
    priority: "high",
    rotation: { kind: "recurring", display_secs: 30 },
    topic: "arsenal-vs-chelsea",
    payload: { title: "Third notification", body: "body three" },
    meta: {
      source: "ESPN",
      category: "Sports",
      published_at_ms: 1700000000000,
      link: "https://example.com/story",
      subtitle: "Full-time report",
      details: [{ label: "Attendance", value: "60,000" }],
      espn: {
        league: "ENG.1",
        homeAbbrev: "ARS",
        awayAbbrev: "CHE",
        homeScore: 2,
        awayScore: 0,
        clock: "FT",
        homeCards: [1, 0],
        awayCards: [0, 1],
        homeCrest: null,
        awayCrest: null,
      },
    },
    signal: "generic",
    origin: "football",
  },
};

function mockLoads(status: SecretStatus = unsetSecrets) {
  mockIPC((command) => {
    if (command === "get_config") return config;
    if (command === "get_secret_status") return status;
    if (command === "get_default_config") return rustConfigDefaults;
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
  // defensive: a test that enables fake timers (plan 108's connector-health
  // and auto-clear tests) always restores real timers itself, but this
  // guards against a leak into later tests if one fails mid-test.
  vi.useRealTimers();
});

describe("SettingsApp", () => {
  it("renders sidebar navigation and switches among available sections", async () => {
    mockLoads();
    render(<SettingsApp />);

    expect(await screen.findByRole("heading", { level: 1, name: "General" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "General" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Football" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "News" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cmux" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Connectors & Keys" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Shortcuts" })).toBeTruthy();

    const appearance = screen.getByRole("button", {
      name: "Appearance",
    }) as HTMLButtonElement;
    expect(appearance.disabled).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Football" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Football" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "News" }));
    expect(await screen.findByRole("heading", { level: 1, name: "News" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Cmux" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Cmux" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Connectors & Keys" }));
    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: "Connectors & Keys",
      }),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Shortcuts" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Shortcuts" })).toBeTruthy();
    expect(await screen.findByText("Expand or collapse the slot (manual)")).toBeTruthy();
    const shortcutTable = screen.getByRole("table", {
      name: "Keyboard shortcuts",
    });
    expect(within(shortcutTable).getAllByText("active")).toHaveLength(6);
    expect(screen.queryAllByText("planned · not implemented")).toHaveLength(0);
  });

  it("Appearance section is enabled and renders all eight preview cards", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    const appearanceButton = screen.getByRole("button", {
      name: "Appearance",
    }) as HTMLButtonElement;
    expect(appearanceButton.disabled).toBe(false);

    fireEvent.click(appearanceButton);
    expect(await screen.findByRole("heading", { level: 1, name: "Appearance" })).toBeTruthy();
    expect(await screen.findByText("Goal (High priority, football)")).toBeTruthy();
    expect(await screen.findByText("Red card (High priority, football)")).toBeTruthy();
    expect(await screen.findByText("Generic alert (High priority, cmux)")).toBeTruthy();
    expect(await screen.findByText("News headline (Low priority)")).toBeTruthy();
    // plan 111 Step 3: the four states the old four-sample gallery could
    // never show — the ones most sensitive to CSS drift.
    expect(await screen.findByText("Compact (collapsed manifest, medium priority)")).toBeTruthy();
    expect(await screen.findByText("Live match (recurring scorecard, football)")).toBeTruthy();
    expect(await screen.findByText("Weather alert (medium priority)")).toBeTruthy();
    expect(await screen.findByText("News headline, compact (single timestamp)")).toBeTruthy();
    expect(await screen.findByText("GOAL")).toBeTruthy();
    expect(
      await screen.findByText("Parliament passes the landmark digital rights bill"),
    ).toBeTruthy();
    // the cmux sample's body carries inline markdown (plan 032 step 6) —
    // the command must render as <code> elements, not literal backticks.
    // it shows twice on the expanded sample: compact .body + manifest
    // Message cell both run renderInlineMarkdown.
    const previewCommands = await screen.findAllByText("git push origin master");
    expect(previewCommands.length).toBe(2);
    expect(previewCommands.every((el) => el.tagName === "CODE")).toBe(true);

    const appearanceSection = screen
      .getByRole("heading", { level: 1, name: "Appearance" })
      .closest("form") as HTMLElement;
    expect(
      within(appearanceSection).getByRole("button", {
        name: "Send test notification",
      }),
    ).toBeTruthy();
  });

  // plan 111 Step 3's own verify clause: the compact fixture renders
  // `.compact` (the collapsed-manifest state), and the live fixture
  // renders its `.chip-live` (proof `espn` meta reached the recurring
  // scorecard branch, not the generic compact/manifest branch).
  it("Appearance gallery: every fixture renders without error, compact shows .compact, live shows its live chip", async () => {
    mockLoads();
    const { container } = render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    await screen.findByRole("heading", { level: 1, name: "Appearance" });
    await screen.findByText("GOAL");

    const stages = container.querySelectorAll(".preview-stage.card-root");
    expect(stages.length).toBe(8);
    stages.forEach((stage) => {
      expect(stage.querySelector(".card-assembly")).not.toBeNull();
    });

    const compactRow = (
      await screen.findByText("Compact (collapsed manifest, medium priority)")
    ).closest(".preview-row") as HTMLElement;
    expect(within(compactRow).getByText("Build finished")).toBeTruthy();
    expect(compactRow.querySelector(".compact")).not.toBeNull();
    expect(compactRow.querySelector(".card-assembly.expanded")).toBeNull();

    const liveRow = (await screen.findByText("Live match (recurring scorecard, football)")).closest(
      ".preview-row",
    ) as HTMLElement;
    expect(liveRow.querySelector(".chip-live")).not.toBeNull();
    expect(liveRow.querySelector(".notif-block")).not.toBeNull();
  });

  it("calls set_appearance with scale/radius/opacity, not card_scale/card_radius/card_opacity", async () => {
    const setAppearance = vi.fn();
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") {
        setAppearance(payload);
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    await screen.findByRole("heading", { level: 1, name: "Appearance" });

    const scaleToggle = await screen.findByRole("group", { name: "Scale" });
    fireEvent.click(within(scaleToggle).getByRole("button", { name: "Large" }));

    await waitFor(() => {
      expect(setAppearance).toHaveBeenCalledWith({
        scale: 1.15,
        radius: 8,
        opacity: 0.9,
      });
    });
  });

  it("shows loaded values in General", async () => {
    mockLoads();
    render(<SettingsApp />);

    expect(await screen.findByDisplayValue("4321")).toBeTruthy();
    expect(screen.getByDisplayValue("14")).toBeTruthy();
    expect(screen.getByDisplayValue("75")).toBeTruthy();
    expect(isChecked(screen.getByLabelText("Start paused"))).toBe(true);
    // plan 085: the toggle reflects the loaded config's resting_state
    // ("notch" in this fixture) — checked means "hidden while idle".
    expect(isChecked(screen.getByLabelText("Hide overlay when idle"))).toBe(true);
    expect(
      screen.getByText(
        "Waiting items promote high → medium → low. Priority chooses the next turn; it never interrupts the visible item.",
      ),
    ).toBeTruthy();
  });

  // plan 085: the hide-when-idle toggle patches resting_state and it rides
  // the same Save & Relaunch path as every other General-section field —
  // the toggle's help text says so, and this test pins that it's true.
  it("toggling Hide overlay when idle patches resting_state into the saved config", async () => {
    let savedConfig: Config | null = null;
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        savedConfig = (payload as { config: Config }).config;
        return null;
      }
    });
    render(<SettingsApp />);

    const toggle = await screen.findByLabelText("Hide overlay when idle");
    expect(isChecked(toggle)).toBe(true); // fixture config has resting_state: "notch"

    fireEvent.click(toggle);
    expect(isChecked(toggle)).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Save & Relaunch" }));

    await waitFor(() => expect(savedConfig).not.toBeNull());
    // biome-ignore lint/style/noNonNullAssertion: guaranteed non-null by the waitFor above.
    expect(savedConfig!.resting_state).toBe("rail");
  });

  it("renders every save rejection message", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        return Promise.reject([
          "port must be at least 1024",
          "rss_poll_secs must be between 5 and 3600",
        ]);
      }
    });
    render(<SettingsApp />);

    fireEvent.click(await screen.findByRole("button", { name: "Save & Relaunch" }));

    expect(await screen.findByText("port must be at least 1024")).toBeTruthy();
    expect(screen.getByText("rss_poll_secs must be between 5 and 3600")).toBeTruthy();
    expect(screen.getByText("Config rejected")).toBeTruthy();
  });

  it("saves a key with its snake_case field, clears the input, and refreshes status", async () => {
    let statusReads = 0;
    const setSecret = vi.fn();
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_secret_status") {
        statusReads += 1;
        return statusReads === 1
          ? unsetSecrets
          : { ...unsetSecrets, openrouter_api_key: "set (…9xyz)" };
      }
      if (command === "set_secret") {
        setSecret(payload);
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Connectors & Keys" }));

    const input = (await screen.findByLabelText("OpenRouter API key")) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "sk-or-secret-9xyz" } });
    fireEvent.click(screen.getByRole("button", { name: "Save OpenRouter API key" }));

    await waitFor(() => {
      expect(setSecret).toHaveBeenCalledWith({
        field: "openrouter_api_key",
        value: "sk-or-secret-9xyz",
      });
      expect(input.value).toBe("");
    });
    expect(await screen.findByText("set (…9xyz)")).toBeTruthy();
  });

  it("Reset restores the values returned by get_config", async () => {
    mockLoads();
    render(<SettingsApp />);

    const port = (await screen.findByLabelText("Listener port")) as HTMLInputElement;
    fireEvent.change(port, { target: { value: "5555" } });
    expect(port.value).toBe("5555");

    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect((screen.getByLabelText("Listener port") as HTMLInputElement).value).toBe("4321");
    expect(isChecked(screen.getByLabelText("Start paused"))).toBe(true);
  });

  it('a NumberControl clearing to retype doesn\'t snap to 0, and decimal fields carry step="any"', async () => {
    mockLoads();
    render(<SettingsApp />);

    // integer field: clear-then-retype must not flash to 0 mid-edit —
    // the old `onChange(Number(e.target.value))` pattern turned a
    // cleared field into `Number("") === 0` on every keystroke.
    const port = (await screen.findByLabelText("Listener port")) as HTMLInputElement;
    fireEvent.change(port, { target: { value: "" } });
    expect(port.value).toBe("");
    fireEvent.change(port, { target: { value: "8" } });
    expect(port.value).toBe("8");
    fireEvent.change(port, { target: { value: "80" } });
    expect(port.value).toBe("80");

    // latitude/longitude are decimal signed fields: step="any" (not the
    // HTML default of 1) is what stops a value like 12.5 or -77.59 from
    // registering as a stepMismatch.
    fireEvent.click(screen.getByRole("button", { name: "Weather" }));
    const lat = (await screen.findByLabelText("Latitude")) as HTMLInputElement;
    const lon = (await screen.findByLabelText("Longitude")) as HTMLInputElement;
    expect(lat.step).toBe("any");
    expect(lon.step).toBe("any");
    expect(lat.value).toBe("12.97");
    expect(lon.value).toBe("77.59");

    // a full decimal replacement round-trips correctly (jsdom's own
    // number-input sanitization — same as real browsers — only
    // discards genuinely-invalid intermediate strings like a bare "."
    // or "-"; this NumberControl no longer forces `Number(value)` back
    // onto the field on every keystroke, so a complete decimal is never
    // fought by the controlled value).
    fireEvent.change(lat, { target: { value: "13.5" } });
    expect(lat.value).toBe("13.5");
  });

  it("a NumberControl left empty on blur restores the last-committed value", async () => {
    mockLoads();
    render(<SettingsApp />);

    const port = (await screen.findByLabelText("Listener port")) as HTMLInputElement;
    expect(port.value).toBe("4321");
    fireEvent.change(port, { target: { value: "" } });
    expect(port.value).toBe("");
    fireEvent.blur(port);
    expect(port.value).toBe("4321");
  });

  it("Reset to defaults applies the defaults served by get_default_config", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByDisplayValue("4321");
    // get_default_config now resolves on its own microtask, separate from
    // get_config/get_secret_status — wait for it to land (button enabled)
    // before clicking, rather than assuming it's already there.
    await waitFor(() => {
      expect(
        (
          screen.getByRole("button", {
            name: "Reset to defaults",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false);
    });
    fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));

    expect((screen.getByLabelText("Listener port") as HTMLInputElement).value).toBe("9789");
    expect((screen.getByLabelText("Rotation seconds") as HTMLInputElement).value).toBe("8");
    expect((screen.getByLabelText("Queue cap per priority tier") as HTMLInputElement).value).toBe(
      "50",
    );
    expect(isChecked(screen.getByLabelText("Start paused"))).toBe(false);
    expect(isChecked(screen.getByLabelText("Hide overlay when idle"))).toBe(false);
    expect(selectedPriorityLabel(screen.getByLabelText("Manual push priority"))).toBe("Medium");
    expect(rotationOrderRowNames()).toEqual([
      "Football",
      "Manual / CLI push",
      "Weather",
      "Cmux (agent relay)",
      "News",
    ]);

    fireEvent.click(screen.getByRole("button", { name: "Football" }));
    expect(isChecked(await screen.findByLabelText("Enable ESPN scores"))).toBe(true);
    expect((screen.getByLabelText("Leagues") as HTMLTextAreaElement).value).toBe(
      "eng.1\nuefa.champions\nesp.1",
    );
    expect((screen.getByLabelText("Rotation seconds") as HTMLInputElement).value).toBe("8");
    expect(selectedPriorityLabel(screen.getByLabelText("Priority"))).toBe("High");
  });

  it("Appearance controls re-seed from config after Reset to defaults", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") return null;
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    await screen.findByRole("heading", { level: 1, name: "Appearance" });

    const scaleToggle = await screen.findByRole("group", { name: "Scale" });
    fireEvent.click(within(scaleToggle).getByRole("button", { name: "Large" }));
    await waitFor(() => {
      expect(
        within(scaleToggle).getByRole("button", { name: "Large" }).getAttribute("aria-pressed"),
      ).toBe("true");
    });

    // get_default_config resolves on its own microtask — wait for the button
    // to enable before clicking, or the reset asserts against un-reset state.
    await waitFor(() => {
      expect(
        (screen.getByRole("button", { name: "Reset to defaults" }) as HTMLButtonElement).disabled,
      ).toBe(false);
    });
    fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));

    const resetScaleToggle = await screen.findByRole("group", { name: "Scale" });
    expect(
      within(resetScaleToggle).getByRole("button", { name: "Medium" }).getAttribute("aria-pressed"),
    ).toBe("true");
  });

  function rotationOrderRowNames() {
    return screen
      .getAllByRole("listitem")
      .map((row) => row.querySelector(".rotation-order-name")?.textContent);
  }

  function selectedPriorityLabel(toggle: HTMLElement) {
    const selected = within(toggle)
      .getAllByRole("button")
      .find((button) => button.getAttribute("aria-pressed") === "true");
    return selected?.textContent ?? null;
  }

  it("shows each source's loaded priority and reflects a click immediately", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    const manualToggle = screen.getByLabelText("Manual push priority");
    expect(selectedPriorityLabel(manualToggle)).toBe("Low");

    fireEvent.click(within(manualToggle).getByRole("button", { name: "High" }));
    expect(selectedPriorityLabel(manualToggle)).toBe("High");

    fireEvent.click(screen.getByRole("button", { name: "Football" }));
    const espnToggle = await screen.findByLabelText("Priority");
    expect(selectedPriorityLabel(espnToggle)).toBe("Medium");
    fireEvent.click(within(espnToggle).getByRole("button", { name: "High" }));
    expect(selectedPriorityLabel(espnToggle)).toBe("High");

    fireEvent.click(screen.getByRole("button", { name: "News" }));
    const rssToggle = await screen.findByLabelText("Priority");
    expect(selectedPriorityLabel(rssToggle)).toBe("High");
  });

  it("shows the loaded Cmux priority and reflects a click immediately", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Cmux" }));

    const cmuxToggle = await screen.findByLabelText("Priority");
    expect(selectedPriorityLabel(cmuxToggle)).toBe("Medium");
    fireEvent.click(within(cmuxToggle).getByRole("button", { name: "High" }));
    expect(selectedPriorityLabel(cmuxToggle)).toBe("High");
    expect((screen.getByLabelText("Rotation seconds") as HTMLInputElement).value).toBe("16");
  });

  it("preserves a feed's source/category when its url is edited by a trailing slash", async () => {
    let savedConfig: Config | null = null;
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        savedConfig = (payload as { config: Config }).config;
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "News" }));
    const feeds = (await screen.findByLabelText("Feeds")) as HTMLTextAreaElement;
    expect(feeds.value).toBe("https://example.com/world.xml\nhttps://example.com/tech.xml");

    fireEvent.change(feeds, {
      target: {
        value: "https://example.com/world.xml/\nhttps://example.com/tech.xml",
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save & Relaunch" }));

    await waitFor(() => expect(savedConfig).not.toBeNull());
    // biome-ignore lint/style/noNonNullAssertion: guaranteed non-null by the waitFor above; the suggested ?. fix breaks tsc (CFA narrows savedConfig to null → never).
    expect(savedConfig!.rss_feeds).toEqual([
      {
        url: "https://example.com/world.xml/",
        source: "Example",
        category: "world",
      },
      { url: "https://example.com/tech.xml", source: null, category: null },
    ]);
  });

  it("resets a feed's source/category to null when its url is replaced with a different host", async () => {
    let savedConfig: Config | null = null;
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        savedConfig = (payload as { config: Config }).config;
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "News" }));
    const feeds = (await screen.findByLabelText("Feeds")) as HTMLTextAreaElement;

    fireEvent.change(feeds, {
      target: {
        value: "https://different.example/rss.xml\nhttps://example.com/tech.xml",
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save & Relaunch" }));

    await waitFor(() => expect(savedConfig).not.toBeNull());
    // biome-ignore lint/style/noNonNullAssertion: guaranteed non-null by the waitFor above; the suggested ?. fix breaks tsc (CFA narrows savedConfig to null → never).
    expect(savedConfig!.rss_feeds).toEqual([
      {
        url: "https://different.example/rss.xml",
        source: null,
        category: null,
      },
      { url: "https://example.com/tech.xml", source: null, category: null },
    ]);
  });

  // --- plan 130: Topics textarea (merges with Feeds, not either/or) ---

  it("loads Topics from config and saves edited lines back, trimmed and empty lines dropped", async () => {
    let savedConfig: Config | null = null;
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        savedConfig = (payload as { config: Config }).config;
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "News" }));
    const topics = (await screen.findByLabelText("Topics")) as HTMLTextAreaElement;
    expect(topics.value).toBe("aston villa transfers");

    fireEvent.change(topics, {
      target: { value: "  formula 1  \n\n   \nnvidia earnings" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save & Relaunch" }));

    await waitFor(() => expect(savedConfig).not.toBeNull());
    // biome-ignore lint/style/noNonNullAssertion: guaranteed non-null by the waitFor above; the suggested ?. fix breaks tsc (CFA narrows savedConfig to null → never).
    expect(savedConfig!.rss_topics).toEqual(["formula 1", "nvidia earnings"]);
  });

  // --- plan 130 Step 3: on-the-go search (search_news_now) ---

  describe("Search now (plan 130 Step 3)", () => {
    async function openNews() {
      render(<SettingsApp />);
      await screen.findByRole("heading", { level: 1, name: "General" });
      fireEvent.click(screen.getByRole("button", { name: "News" }));
      // Waits for the News section's own content (not just the header,
      // which updates a render ahead of the AnimatePresence-keyed content
      // swap) — mirrors the existing feed tests' `findByLabelText("Feeds")`
      // pattern above.
      await screen.findByLabelText("Topics");
    }

    it("the Search button is disabled while the input is empty", async () => {
      mockLoads();
      await openNews();

      const button = screen.getByRole("button", { name: "Search" }) as HTMLButtonElement;
      expect(button.disabled).toBe(true);
    });

    it("invokes search_news_now with the typed query and announces the count on success", async () => {
      let invokedQuery: string | null = null;
      mockIPC((command, payload) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "search_news_now") {
          invokedQuery = (payload as { query: string }).query;
          return 3;
        }
      });
      await openNews();

      const input = screen.getByPlaceholderText("e.g. aston villa transfers") as HTMLInputElement;
      fireEvent.change(input, { target: { value: "aston villa transfers" } });
      fireEvent.click(screen.getByRole("button", { name: "Search" }));

      await waitFor(() => expect(invokedQuery).toBe("aston villa transfers"));
      expect(await screen.findByText("3 stories queued")).toBeTruthy();
      // input clears on success
      await waitFor(() => expect(input.value).toBe(""));
    });

    it("shows the singular form for a single result", async () => {
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "search_news_now") return 1;
      });
      await openNews();

      const input = screen.getByPlaceholderText("e.g. aston villa transfers") as HTMLInputElement;
      fireEvent.change(input, { target: { value: "formula 1" } });
      fireEvent.click(screen.getByRole("button", { name: "Search" }));

      expect(await screen.findByText("1 story queued")).toBeTruthy();
    });

    it("surfaces a rejection as an error and leaves the input for retry", async () => {
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "search_news_now") return Promise.reject("already searching");
      });
      await openNews();

      const input = screen.getByPlaceholderText("e.g. aston villa transfers") as HTMLInputElement;
      fireEvent.change(input, { target: { value: "formula 1" } });
      fireEvent.click(screen.getByRole("button", { name: "Search" }));

      expect(await screen.findByText("already searching")).toBeTruthy();
      expect(input.value).toBe("formula 1");
    });

    it("never invokes search_news_now for a whitespace-only query", async () => {
      const searchNow = vi.fn();
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "search_news_now") {
          searchNow();
          return 0;
        }
      });
      await openNews();

      const input = screen.getByPlaceholderText("e.g. aston villa transfers") as HTMLInputElement;
      fireEvent.change(input, { target: { value: "   " } });
      const button = screen.getByRole("button", { name: "Search" }) as HTMLButtonElement;
      expect(button.disabled).toBe(true);
      fireEvent.click(button);

      await flush();
      expect(searchNow).not.toHaveBeenCalled();
    });
  });

  it("rotation order loads in the saved order and reorders with edge-disabled buttons", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    expect(rotationOrderRowNames()).toEqual([
      "News",
      "Cmux (agent relay)",
      "Manual / CLI push",
      "Weather",
      "Football",
    ]);

    const rows = screen.getAllByRole("listitem");
    const [newsRow, cmuxRow, manualRow, , footballRow] = rows;
    expect(
      (
        within(newsRow).getByRole("button", {
          name: /earlier/,
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (
        within(footballRow).getByRole("button", {
          name: /later/,
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (
        within(manualRow).getByRole("button", {
          name: /earlier/,
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
    expect(
      (
        within(cmuxRow).getByRole("button", {
          name: /earlier/,
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);

    fireEvent.click(within(manualRow).getByRole("button", { name: /earlier/ }));
    expect(rotationOrderRowNames()).toEqual([
      "News",
      "Manual / CLI push",
      "Cmux (agent relay)",
      "Weather",
      "Football",
    ]);
  });

  it("renders the Telegram delivery-health line from get_connector_health", async () => {
    // same mock-and-assert shape as the SecretStatus-fetch tests above:
    // the health fetch is advisory and resolves on its own microtask.
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_connector_health") {
        return { lastAttemptMs: 1000, lastSuccessMs: 120000, consecutiveFailures: 0 };
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Connectors & Keys" }));

    expect(await screen.findByText("Last delivered: 2 min ago")).toBeTruthy();
  });

  it("Diagnostics section renders the lines returned by get_recent_log_lines", async () => {
    // same advisory-fetch shape as the connector-health test above: the
    // fetch fires on section-open and resolves on its own microtask.
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_recent_log_lines") {
        return ["INFO notchtap: boot complete", "WARN notchtap: queue full"];
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Diagnostics" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Diagnostics" })).toBeTruthy();
    expect(await screen.findByText(/INFO notchtap: boot complete/)).toBeTruthy();
    expect(screen.getByText(/WARN notchtap: queue full/)).toBeTruthy();
  });

  it("History section renders entries newest-first from a mocked get_history", async () => {
    // fixture entries are oldest -> newest (088's read_recent contract);
    // the UI must reverse them for display, not the mock.
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return [historyEntryOlder, historyEntryNewer];
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    expect(await screen.findByRole("heading", { level: 1, name: "History" })).toBeTruthy();
    expect(await screen.findByText("Second notification")).toBeTruthy();
    expect(screen.getByText("First notification")).toBeTruthy();

    const titles = screen
      .getAllByText(/notification$/)
      // plan 112 Step 4: history-title now carries utility classes
      // alongside its stable "history-title" hook class, so an exact
      // className match no longer isolates it — check for the token
      // instead (classList.contains), same selection intent.
      .filter((el) => el.classList.contains("history-title"))
      .map((el) => el.textContent);
    expect(titles).toEqual(["Second notification", "First notification"]);
  });

  it("Empty-history state renders the 'nothing recorded yet' copy", async () => {
    // fixture config has history_enabled: true, so an empty result reads
    // as "on, nothing recorded" rather than "off".
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return [];
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    expect(
      await screen.findByText("History is on, but nothing has been recorded yet."),
    ).toBeTruthy();
    expect(screen.queryByText(/History is off/)).toBeNull();
  });

  it("History-disabled state renders the distinct 'history is off' copy", async () => {
    const disabledConfig: Config = { ...config, history_enabled: false };
    mockIPC((command) => {
      if (command === "get_config") return disabledConfig;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return [];
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    expect(
      await screen.findByText(
        'History is off. Turn on "Record notification history" in General to start recording.',
      ),
    ).toBeTruthy();
    expect(screen.queryByText("History is on, but nothing has been recorded yet.")).toBeNull();
  });

  it("the clear control requires a second confirming click before clear_history is invoked", async () => {
    const clearHistory = vi.fn();
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return [historyEntryOlder];
      if (command === "clear_history") {
        clearHistory();
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    const clearButton = await screen.findByRole("button", { name: "Clear history" });
    fireEvent.click(clearButton);
    // first click only arms the confirmation — clear_history must NOT fire yet
    expect(clearHistory).not.toHaveBeenCalled();
    expect(await screen.findByRole("button", { name: "Really clear?" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Really clear?" }));
    await waitFor(() => expect(clearHistory).toHaveBeenCalledTimes(1));
  });

  // plan 129 (K1): same fix, same test shape as QueueSection's own K1 test
  // above — HistorySection carried the identical parent-level-conditional
  // bug (`entries.length === 0 ? <p> : <ul>…`), so Clear history's own
  // outgoing row never got to exit-animate either.
  it("Clear history lets the row exit-animate instead of vanishing outright, with the empty state appearing immediately alongside it", async () => {
    const clearHistory = vi.fn();
    let entries: HistoryEntry[] = [historyEntryOlder];
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return entries;
      if (command === "clear_history") {
        clearHistory();
        entries = [];
        return null;
      }
    });
    render(<SettingsApp />);
    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    await screen.findByText("First notification");
    fireEvent.click(screen.getByRole("button", { name: "Clear history" }));
    fireEvent.click(await screen.findByRole("button", { name: "Really clear?" }));
    await waitFor(() => expect(clearHistory).toHaveBeenCalledTimes(1));

    // the render where `entries` actually flipped to `[]`: the empty-state
    // text is already up, but the outgoing row is STILL in the DOM,
    // mid-exit — never both-at-once under the old parent-level ternary.
    expect(
      await screen.findByText("History is on, but nothing has been recorded yet."),
    ).toBeTruthy();
    expect(screen.getByText("First notification")).toBeTruthy();

    // once the row's own 180ms exit animation actually finishes, it
    // leaves the DOM for good.
    await waitFor(() => {
      expect(screen.queryByText("First notification")).toBeNull();
    });
  });

  // plan 110 (Step A): history richness — the metadata row + expandable
  // details.
  describe("history richness (plan 110)", () => {
    function mockHistory(entries: HistoryEntry[]) {
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_history") return entries;
      });
    }

    async function openHistory() {
      render(<SettingsApp />);
      await screen.findByRole("heading", { level: 1, name: "General" });
      fireEvent.click(screen.getByRole("button", { name: "History" }));
      await screen.findByRole("heading", { level: 1, name: "History" });
    }

    it("renders the metadata chips and an expandable, togglable details block for an entry with full meta", async () => {
      mockHistory([historyEntryFullMeta]);
      await openHistory();

      const row = (await screen.findByText("Third notification")).closest(
        ".history-row",
      ) as HTMLElement;
      expect(row).not.toBeNull();

      // always-present chips: source, category, priority, event_type, rotation
      expect(within(row).getByText("ESPN")).toBeTruthy();
      expect(within(row).getByText("Sports")).toBeTruthy();
      expect(within(row).getByText("High")).toBeTruthy();
      expect(within(row).getByText("Score update")).toBeTruthy();
      expect(within(row).getByText("every 30s")).toBeTruthy();

      // the disclosure starts closed, is queryable/togglable by its
      // accessible (visible) name, and flips `open` on click.
      const details = row.querySelector(".history-details") as HTMLDetailsElement;
      expect(details).not.toBeNull();
      expect(details.open).toBe(false);
      const summary = within(row).getByText("More details");
      fireEvent.click(summary);
      expect(details.open).toBe(true);

      // expanded-only content
      expect(within(row).getByText("Full-time report")).toBeTruthy();
      expect(within(row).getByText("arsenal-vs-chelsea")).toBeTruthy();
      expect(within(row).getByText("Attendance")).toBeTruthy();
      expect(within(row).getByText("60,000")).toBeTruthy();
      expect(within(row).getByText("22:13")).toBeTruthy(); // published_at_ms (1700000000000 = 22:13 UTC)
      expect(within(row).getByText(/ENG\.1: ARS 2–0 CHE \(FT\)/)).toBeTruthy();
      expect(within(row).getByText("https://example.com/story")).toBeTruthy();
    });

    it("renders no empty chrome (no source/category chip, no details disclosure) for an entry with empty meta", async () => {
      mockHistory([historyEntryOlder]);
      await openHistory();

      const row = (await screen.findByText("First notification")).closest(
        ".history-row",
      ) as HTMLElement;
      expect(row).not.toBeNull();

      // only the three always-present chips (priority, event_type,
      // rotation) — no source/category chip.
      expect(row.querySelectorAll(".history-meta-chip")).toHaveLength(3);
      expect(within(row).getByText("Medium")).toBeTruthy();
      expect(within(row).getByText("Generic")).toBeTruthy();
      expect(within(row).getByText("TTL 8s")).toBeTruthy();

      // no optional-richness fields exist on this fixture, so no
      // disclosure affordance renders at all.
      expect(row.querySelector(".history-details")).toBeNull();
      expect(within(row).queryByText("More details")).toBeNull();
    });

    it("falls back to the raw wire value for an unrecognized priority or rotation kind, instead of a blank chip", async () => {
      // `get_history` crosses the tauri IPC boundary as untyped JSON — an
      // unexpected priority/rotation.kind (a future variant, a bug, a
      // hand-edited history.jsonl) must render legibly rather than as an
      // empty chip (an un-narrowed `Record` lookup would return
      // `undefined`, which React renders as nothing).
      const malformed: HistoryEntry = {
        ...historyEntryOlder,
        event: {
          ...historyEntryOlder.event,
          // deliberately off-contract values simulating untyped IPC
          // input — routed through `unknown` (not `any`) to keep the
          // rest of the object's real typing intact.
          priority: "urgent" as unknown as PriorityLevel,
          rotation: { kind: "hourly", every_secs: 60 } as unknown as HistoryRotationSpec,
        },
      };
      mockHistory([malformed]);
      await openHistory();

      const row = (await screen.findByText("First notification")).closest(
        ".history-row",
      ) as HTMLElement;
      expect(row).not.toBeNull();
      expect(within(row).getByText("urgent")).toBeTruthy();
      expect(within(row).getByText("hourly")).toBeTruthy();
    });

    it("renders a 300-char unbroken-token body without widening the row (pins the .history-body class)", async () => {
      const longToken = "x".repeat(300);
      const entry: HistoryEntry = {
        ...historyEntryOlder,
        event: {
          ...historyEntryOlder.event,
          payload: { title: "Long body notification", body: longToken },
        },
      };
      mockHistory([entry]);
      await openHistory();

      const body = await screen.findByText(longToken);
      expect(body.classList.contains("history-body")).toBe(true);
    });

    it("renders a non-http-scheme link as literal, non-clickable text — never an <a href>", async () => {
      const maliciousLink = "javascript:alert(1)";
      const entry: HistoryEntry = {
        ...historyEntryFullMeta,
        event: {
          ...historyEntryFullMeta.event,
          meta: { ...historyEntryFullMeta.event.meta, link: maliciousLink },
        },
      };
      mockHistory([entry]);
      await openHistory();

      const row = (await screen.findByText("Third notification")).closest(
        ".history-row",
      ) as HTMLElement;
      fireEvent.click(within(row).getByText("More details"));

      const linkText = within(row).getByText(maliciousLink);
      expect(linkText.tagName).not.toBe("A");
      expect(row.querySelector("a")).toBeNull();
      expect(within(row).queryByRole("link")).toBeNull();
      expect(linkText.classList.contains("history-link-text")).toBe(true);
    });

    it("renders markup-like feed text literally — no created img/script element, no HTML injection", async () => {
      const markupSubtitle = '<img src=x onerror="alert(1)">';
      const markupBody = "<script>alert(1)</script>";
      const entry: HistoryEntry = {
        ...historyEntryFullMeta,
        event: {
          ...historyEntryFullMeta.event,
          payload: { title: "Markup notification", body: markupBody },
          meta: { ...historyEntryFullMeta.event.meta, subtitle: markupSubtitle },
        },
      };
      mockHistory([entry]);
      await openHistory();

      const row = (await screen.findByText("Markup notification")).closest(
        ".history-row",
      ) as HTMLElement;
      expect(within(row).getByText(markupBody)).toBeTruthy();
      fireEvent.click(within(row).getByText("More details"));
      expect(within(row).getByText(markupSubtitle)).toBeTruthy();

      // literal text, never parsed as markup: no element actually created
      // from either string.
      expect(row.querySelector("script")).toBeNull();
      expect(row.querySelector("img")).toBeNull();
    });
  });

  it("the history_enabled toggle round-trips into the saved config payload", async () => {
    let savedConfig: Config | null = null;
    const disabledConfig: Config = { ...config, history_enabled: false };
    mockIPC((command, payload) => {
      if (command === "get_config") return disabledConfig;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        savedConfig = (payload as { config: Config }).config;
        return null;
      }
    });
    render(<SettingsApp />);

    const toggle = await screen.findByLabelText("Record notification history");
    expect(isChecked(toggle)).toBe(false);

    fireEvent.click(toggle);
    expect(isChecked(toggle)).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Save & Relaunch" }));

    await waitFor(() => expect(savedConfig).not.toBeNull());
    // biome-ignore lint/style/noNonNullAssertion: guaranteed non-null by the waitFor above.
    expect(savedConfig!.history_enabled).toBe(true);
  });

  // plan 104: weather_enabled's own round-trip test is this test's pattern.
  it("the now_playing_enabled toggle round-trips into the saved config payload", async () => {
    let savedConfig: Config | null = null;
    const disabledConfig: Config = { ...config, now_playing_enabled: false };
    mockIPC((command, payload) => {
      if (command === "get_config") return disabledConfig;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "save_config_and_relaunch") {
        savedConfig = (payload as { config: Config }).config;
        return null;
      }
    });
    render(<SettingsApp />);

    const toggle = await screen.findByLabelText("Enable now playing");
    expect(isChecked(toggle)).toBe(false);

    fireEvent.click(toggle);
    expect(isChecked(toggle)).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Save & Relaunch" }));

    await waitFor(() => expect(savedConfig).not.toBeNull());
    // biome-ignore lint/style/noNonNullAssertion: guaranteed non-null by the waitFor above.
    expect(savedConfig!.now_playing_enabled).toBe(true);
  });

  // done criterion: the kill switch must never surface in this UI.
  it("never renders a control for the now_playing_adapter_enabled kill switch", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
    });
    render(<SettingsApp />);
    await screen.findByLabelText("Enable now playing");
    expect(screen.queryByText(/adapter.enabled/i)).toBeNull();
    expect(screen.queryByLabelText(/kill switch/i)).toBeNull();
  });

  // plan 121: settings-window queue visibility + clear/skip.
  describe("Queue section (plan 121)", () => {
    const waitingHigh: QueueItemSummary = {
      title: "High priority waiting item",
      priority: "high",
      source: "football",
    };
    const waitingLow: QueueItemSummary = {
      title: "Low priority waiting item",
      priority: "low",
      source: "manual",
    };

    function mockQueue(items: QueueItemSummary[]) {
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") return items;
      });
    }

    async function openQueue() {
      render(<SettingsApp />);
      await screen.findByRole("heading", { level: 1, name: "General" });
      fireEvent.click(screen.getByRole("button", { name: "Queue" }));
      await screen.findByRole("heading", { level: 1, name: "Queue" });
    }

    it("renders waiting items with a title and a priority tag, from a mocked get_queue", async () => {
      mockQueue([waitingHigh, waitingLow]);
      await openQueue();

      expect(await screen.findByText("High priority waiting item")).toBeTruthy();
      expect(screen.getByText("Low priority waiting item")).toBeTruthy();
      const rows = screen
        .getAllByText(/waiting item$/)
        .filter((el) => el.classList.contains("queue-title"));
      expect(rows).toHaveLength(2);
      // plan 124 (F5a): pin the tag CONTENT, not just the row count — a tag
      // that silently rendered the same label for every priority (or the
      // raw enum value instead of PRIORITY_LABELS) would still pass the
      // count-only assertion above.
      const tags = screen
        .getAllByText(/^(High|Low)$/)
        .filter((el) => el.classList.contains("queue-priority-tag"));
      expect(tags.map((el) => el.textContent)).toEqual(["High", "Low"]);
    });

    it("renders the empty state when nothing is waiting", async () => {
      mockQueue([]);
      await openQueue();

      expect(await screen.findByText("Queue is empty.")).toBeTruthy();
    });

    it("Skip current invokes skip_current and refetches the list", async () => {
      const skipCurrent = vi.fn();
      let queueItems: QueueItemSummary[] = [waitingHigh];
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") return queueItems;
        if (command === "skip_current") {
          skipCurrent();
          queueItems = [];
          return null;
        }
      });
      render(<SettingsApp />);
      await screen.findByRole("heading", { level: 1, name: "General" });
      fireEvent.click(screen.getByRole("button", { name: "Queue" }));
      await screen.findByRole("heading", { level: 1, name: "Queue" });
      await screen.findByText("High priority waiting item");

      fireEvent.click(screen.getByRole("button", { name: "Skip current" }));
      await waitFor(() => expect(skipCurrent).toHaveBeenCalledTimes(1));
      expect(await screen.findByText("Skipped")).toBeTruthy();
      await waitFor(() => expect(screen.getByText("Queue is empty.")).toBeTruthy());
    });

    it("Clear queue invokes clear_queue and refetches the list", async () => {
      const clearQueue = vi.fn();
      let queueItems: QueueItemSummary[] = [waitingHigh, waitingLow];
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") return queueItems;
        if (command === "clear_queue") {
          clearQueue();
          queueItems = [];
          return 2;
        }
      });
      render(<SettingsApp />);
      await screen.findByRole("heading", { level: 1, name: "General" });
      fireEvent.click(screen.getByRole("button", { name: "Queue" }));
      await screen.findByRole("heading", { level: 1, name: "Queue" });
      await screen.findByText("High priority waiting item");

      fireEvent.click(screen.getByRole("button", { name: "Clear queue" }));
      await waitFor(() => expect(clearQueue).toHaveBeenCalledTimes(1));
      expect(await screen.findByText("Queue cleared")).toBeTruthy();
      await waitFor(() => expect(screen.getByText("Queue is empty.")).toBeTruthy());
    });

    // plan 129 (K1): the actual choreography fix, pinned directly. Before
    // this plan, `items.length === 0 ? <p> : <ul>…` unmounted the whole
    // `<ul>` (AnimatePresence included) the instant Clear emptied the
    // array, so the outgoing row's own exit animation never got a chance
    // to play — a hard cut, not a collapse. The fix keeps the `<ul>` +
    // AnimatePresence mounted and renders the empty-state `<p>` as a
    // sibling instead, so both can be true on the SAME render: the row is
    // still in the DOM (exiting) AND the empty-state text has already
    // appeared, then the row actually leaves once its own exit window
    // elapses.
    it("Clear queue lets the row exit-animate instead of vanishing outright, with the empty state appearing immediately alongside it", async () => {
      const clearQueue = vi.fn();
      let queueItems: QueueItemSummary[] = [waitingHigh];
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") return queueItems;
        if (command === "clear_queue") {
          clearQueue();
          queueItems = [];
          return 1;
        }
      });
      render(<SettingsApp />);
      await screen.findByRole("heading", { level: 1, name: "General" });
      fireEvent.click(screen.getByRole("button", { name: "Queue" }));
      await screen.findByRole("heading", { level: 1, name: "Queue" });
      await screen.findByText("High priority waiting item");

      fireEvent.click(screen.getByRole("button", { name: "Clear queue" }));
      await waitFor(() => expect(clearQueue).toHaveBeenCalledTimes(1));

      // the render where `items` actually flipped to `[]`: the empty-state
      // text is already up, but the outgoing row is STILL in the DOM,
      // mid-exit — never both-at-once under the old parent-level ternary.
      expect(await screen.findByText("Queue is empty.")).toBeTruthy();
      expect(screen.getByText("High priority waiting item")).toBeTruthy();

      // once the row's own 180ms exit animation actually finishes, it
      // leaves the DOM for good.
      await waitFor(() => {
        expect(screen.queryByText("High priority waiting item")).toBeNull();
      });
    });

    it("a failed get_queue reports an error via ActionStatus", async () => {
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") return Promise.reject("disk error");
      });
      await openQueue();

      expect(await screen.findByText("Couldn't load the queue")).toBeTruthy();
      // plan 124 (F6): before this fix, a failed mount fetch left `items`
      // at `null` forever, so "Loading…" rendered underneath the sticky
      // ActionStatus error above it — the load never resolves into either
      // an error-aware or an empty state. Assert it's actually gone, not
      // merely that the error text is present alongside it.
      expect(screen.queryByText("Loading…")).toBeNull();
      expect(screen.getByText(/Couldn't load the queue — Refresh to retry/)).toBeTruthy();
    });

    // plan 124 (F1): the manual Refresh control — following
    // DiagnosticsSection's own Refresh-button test precedent
    // ("Diagnostics Refresh-button failure is announced..." below), a
    // user-initiated call is `announce: true`, unlike the passive mount
    // fetch.
    it("Refresh re-invokes get_queue and announces its outcome", async () => {
      let queueItems: QueueItemSummary[] = [waitingHigh];
      const getQueue = vi.fn();
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") {
          getQueue();
          return queueItems;
        }
      });
      await openQueue();
      await screen.findByText("High priority waiting item");
      expect(getQueue).toHaveBeenCalledTimes(1);

      queueItems = [waitingHigh, waitingLow];
      fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

      await screen.findByText("Low priority waiting item");
      expect(getQueue).toHaveBeenCalledTimes(2);
    });

    // plan 126: the row key used to be `${index}:${item.title}` — stable
    // only as long as the list never reorders, which a refetch that lands
    // duplicate/reordered summaries could violate. The
    // priority:source:title:occurrenceIndex key stays stable across a
    // refetch that returns the identical list, which is what lets
    // AnimatePresence treat unchanged rows as "still here" (no exit+enter)
    // rather than remounting every row on every Refresh. Same DOM node
    // identity (not just equal content) is the proof: a remount would
    // produce a brand-new element.
    //
    // plan 129 (T2, deep-review fix): the ORIGINAL version of this test
    // refetched the exact same two-item list unchanged — a case where a
    // positional `${index}:${item.title}` key and the content-based key
    // above compute the IDENTICAL string for every row (nothing shifted
    // index), so the test passed under both implementations and never
    // actually discriminated between them. Replaced with a refetch that
    // DROPS the first row: `waitingLow` moves from index 1 to index 0,
    // which a positional key would compute as a different key (`1:...`
    // -> `0:...`) for the exact same surviving row, forcing an
    // AnimatePresence exit+enter remount — the content-based key doesn't
    // care about position at all, so the row's identity survives. Verified
    // this fails against the old positional-key implementation (reverted
    // `withQueueRowKeys` locally, re-ran, saw the DOM-node-identity
    // assertion fail as expected, then restored the fix — not committed).
    it("a surviving row keeps its DOM node identity when a refetch drops an earlier row — no remount from a positional key", async () => {
      let queueItems: QueueItemSummary[] = [waitingHigh, waitingLow];
      // Not `mockQueue(queueItems)`: that helper closes over the array
      // reference it's CALLED with, so reassigning the outer `queueItems`
      // variable below wouldn't be visible to it. Reading `queueItems`
      // directly inside the handler (same shape as the Skip
      // current/Clear queue tests above) is what lets the second
      // `get_queue` return a genuinely different list.
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_queue") return queueItems;
      });
      await openQueue();

      const rowBefore = (await screen.findByText("Low priority waiting item")).closest(
        ".queue-row",
      ) as HTMLElement;

      // waitingHigh (the FIRST row) is gone from this refetch — waitingLow
      // is now at index 0, not index 1.
      queueItems = [waitingLow];
      fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
      await waitFor(() => {
        expect(screen.queryByText("High priority waiting item")).toBeNull();
      });

      const rowAfter = screen
        .getByText("Low priority waiting item")
        .closest(".queue-row") as HTMLElement;

      expect(rowAfter).toBe(rowBefore);
    });

    // plan 124 (F5b): the section's own top-of-file comment cites this
    // exact rule — titles are UNTRUSTED wire data, rendered as plain text
    // only. History precedent: "renders markup-like feed text literally"
    // (above, plan 110 Step A).
    it("renders a markup-like title literally — no img/script element ever created, no HTML injection", async () => {
      const markupTitle = '<img src=x onerror="alert(1)">';
      mockQueue([{ ...waitingHigh, title: markupTitle }]);
      await openQueue();

      const titleEl = await screen.findByText(markupTitle);
      expect(titleEl.classList.contains("queue-title")).toBe(true);
      const row = titleEl.closest(".queue-row") as HTMLElement;
      expect(row.querySelector("script")).toBeNull();
      expect(row.querySelector("img")).toBeNull();
    });

    // plan 124 (F5b): mirrors History's own "300-char unbroken-token body"
    // pin (`SettingsApp.test.tsx`'s History describe block) — jsdom can't
    // measure the CSS effect, only that `.queue-title` still carries the
    // `[overflow-wrap:anywhere]` utility for a title with no natural break
    // point.
    it("renders a 300-char unbroken-token title without widening the row (pins the .queue-title overflow-wrap utility)", async () => {
      const longToken = "x".repeat(300);
      mockQueue([{ ...waitingHigh, title: longToken }]);
      await openQueue();

      const titleEl = await screen.findByText(longToken);
      expect(titleEl.classList.contains("queue-title")).toBe(true);
      expect(titleEl.classList.contains("[overflow-wrap:anywhere]")).toBe(true);
    });
  });
});

// Plan 108: resets hot-apply the live overlay, and every operation that can
// silently fail now reports its outcome through the shared ActionStatus
// mechanism. Each of the seven operations gets independent coverage below.
describe("SettingsApp — action status (plan 108)", () => {
  it("Reset invokes set_appearance with the loaded config's saved values, not the currently-adjusted ones", async () => {
    const setAppearance = vi.fn();
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") {
        setAppearance(payload);
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    const scaleToggle = await screen.findByRole("group", { name: "Scale" });
    fireEvent.click(within(scaleToggle).getByRole("button", { name: "Large" }));
    await waitFor(() => {
      expect(setAppearance).toHaveBeenLastCalledWith({ scale: 1.15, radius: 8, opacity: 0.9 });
    });

    fireEvent.click(screen.getByRole("button", { name: "Reset" }));

    await waitFor(() => {
      // config fixture's appearance is { card_scale: 1, card_radius: 8, card_opacity: 0.9 }
      expect(setAppearance).toHaveBeenLastCalledWith({ scale: 1, radius: 8, opacity: 0.9 });
    });
  });

  it("Reset to defaults invokes set_appearance with the defaults' values", async () => {
    const setAppearance = vi.fn();
    mockIPC((command, payload) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") {
        setAppearance(payload);
        return null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    await waitFor(() => {
      expect(
        (screen.getByRole("button", { name: "Reset to defaults" }) as HTMLButtonElement).disabled,
      ).toBe(false);
    });

    fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));

    await waitFor(() => {
      // rustConfigDefaults' appearance is { card_scale: 1, card_radius: 16, card_opacity: 0.9 }
      expect(setAppearance).toHaveBeenCalledWith({ scale: 1, radius: 16, opacity: 0.9 });
    });
  });

  it("a failed live-apply from Reset still resets the form, and renders the shared, announced appearance error", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") return Promise.reject("overlay unreachable");
    });
    render(<SettingsApp />);

    const port = (await screen.findByLabelText("Listener port")) as HTMLInputElement;
    fireEvent.change(port, { target: { value: "5555" } });

    fireEvent.click(screen.getByRole("button", { name: "Reset" }));

    // form state and live-apply are separate concerns — the form still resets.
    await waitFor(() => {
      expect((screen.getByLabelText("Listener port") as HTMLInputElement).value).toBe("4321");
    });

    const message = await screen.findByText(
      "Live preview couldn't update — will apply on Save & Relaunch",
    );
    expect(message.getAttribute("aria-live")).toBe("polite");
  });

  it("a failed live-apply from Reset to defaults still resets the form, and renders the shared, announced appearance error", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") return Promise.reject("overlay unreachable");
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    await waitFor(() => {
      expect(
        (screen.getByRole("button", { name: "Reset to defaults" }) as HTMLButtonElement).disabled,
      ).toBe(false);
    });

    fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));

    await waitFor(() => {
      expect((screen.getByLabelText("Listener port") as HTMLInputElement).value).toBe("9789");
    });

    const message = await screen.findByText(
      "Live preview couldn't update — will apply on Save & Relaunch",
    );
    expect(message.getAttribute("aria-live")).toBe("polite");
  });

  it("appearance slider hot-apply: repeated identical failures render one deduplicated, announced error with no pending/ok chatter, cleared by the next success", async () => {
    let shouldFail = true;
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "set_appearance") {
        return shouldFail ? Promise.reject("overlay unreachable") : null;
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    const scaleToggle = await screen.findByRole("group", { name: "Scale" });

    // never shows a pending/ok "Working…" flicker for this high-frequency action
    fireEvent.click(within(scaleToggle).getByRole("button", { name: "Large" }));
    await screen.findByText("Live preview couldn't update — will apply on Save & Relaunch");
    expect(screen.queryByText("Working…")).toBeNull();

    // a second, identical failure must not duplicate the message
    fireEvent.click(within(scaleToggle).getByRole("button", { name: "Medium" }));
    await waitFor(() => {
      expect(
        screen.getAllByText("Live preview couldn't update — will apply on Save & Relaunch"),
      ).toHaveLength(1);
    });

    // the next successful apply clears it — no lingering "ok" chatter either
    shouldFail = false;
    fireEvent.click(within(scaleToggle).getByRole("button", { name: "Small" }));
    await waitFor(() => {
      expect(
        screen.queryByText("Live preview couldn't update — will apply on Save & Relaunch"),
      ).toBeNull();
    });
    expect(screen.queryByText("Working…")).toBeNull();
  });

  it("Send test: pending disables the button, and success announces 'Queued'", async () => {
    let resolveInvoke: (() => void) | null = null;
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "send_test_notification") {
        return new Promise<void>((resolve) => {
          resolveInvoke = resolve;
        });
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    const button = screen.getByRole("button", {
      name: "Send test notification",
    }) as HTMLButtonElement;
    fireEvent.click(button);

    await waitFor(() => expect(button.disabled).toBe(true));
    expect(button.textContent).toBe("Sending…");

    // biome-ignore lint/style/noNonNullAssertion: assigned synchronously by mockIPC's executor before this line runs.
    resolveInvoke!();
    await waitFor(() => expect(button.disabled).toBe(false));
    const message = screen.getByText("Queued");
    expect(message.getAttribute("aria-live")).toBe("polite");
  });

  it("Send test failure shows the rejection reason inline, announced", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "send_test_notification") return Promise.reject("queue is full");
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Send test notification" }));

    const message = await screen.findByText("queue is full");
    expect(message.getAttribute("aria-live")).toBe("polite");
  });

  it("Send test success message auto-clears", async () => {
    // No fake timers here (plan 126): ActionStatus's ok-clear now unmounts
    // through an AnimatePresence exit fade, which runs on real
    // requestAnimationFrame ticks. A faked setTimeout clock, even one
    // later swapped back to real, leaves any in-flight animation that
    // started under it wedged mid-transition (its internal scheduling
    // captured the fake clock at start) — so this test waits out the real
    // 2.5s ok-clear window plus the exit fade on the real clock, with a
    // longer per-test timeout to match.
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "send_test_notification") return null;
    });
    render(<SettingsApp />);
    await flush();

    fireEvent.click(screen.getByRole("button", { name: "Send test notification" }));
    await flush();
    expect(screen.getByText("Queued")).toBeTruthy();

    await waitFor(
      () => {
        expect(screen.queryByText("Queued")).toBeNull();
      },
      { timeout: 4000 },
    );
  }, 6000);

  it("History load failure renders 'Couldn't load history' without aria-live — a passive mount read", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return Promise.reject("disk error");
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    const message = await screen.findByText("Couldn't load history");
    expect(message.getAttribute("aria-live")).toBeNull();
  });

  it("History clear failure renders 'Couldn't clear history' near the button, announced, and disables the button while pending", async () => {
    let rejectClear: ((reason?: unknown) => void) | null = null;
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return [historyEntryOlder];
      if (command === "clear_history") {
        return new Promise((_resolve, reject) => {
          rejectClear = reject;
        });
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    const clearButton = (await screen.findByRole("button", {
      name: "Clear history",
    })) as HTMLButtonElement;
    fireEvent.click(clearButton);
    fireEvent.click(screen.getByRole("button", { name: "Really clear?" }));

    await waitFor(() => expect(clearButton.disabled).toBe(true));

    // biome-ignore lint/style/noNonNullAssertion: assigned synchronously by mockIPC's executor before this line runs.
    rejectClear!("network error");
    const message = await screen.findByText("Couldn't clear history");
    expect(message.getAttribute("aria-live")).toBe("polite");
    await waitFor(() => expect(clearButton.disabled).toBe(false));
  });

  it("History clear success shows 'History cleared', distinct from the load status's own location", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_history") return [historyEntryOlder];
      if (command === "clear_history") return null;
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "History" }));
    fireEvent.click(await screen.findByRole("button", { name: "Clear history" }));
    fireEvent.click(screen.getByRole("button", { name: "Really clear?" }));

    const message = await screen.findByText("History cleared");
    expect(message.getAttribute("aria-live")).toBe("polite");
    expect(screen.queryByText("Couldn't load history")).toBeNull();
  });

  it("Diagnostics passive mount-read failure has no aria-live", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_recent_log_lines") return Promise.reject("log file missing");
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Diagnostics" }));

    const message = await screen.findByText("Couldn't read log lines");
    expect(message.getAttribute("aria-live")).toBeNull();
  });

  it("Diagnostics Refresh-button failure is announced (interactive), unlike the passive mount read", async () => {
    let callCount = 0;
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_recent_log_lines") {
        callCount += 1;
        return Promise.reject("log file missing");
      }
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Diagnostics" }));
    await screen.findByText("Couldn't read log lines");
    expect(screen.getByText("Couldn't read log lines").getAttribute("aria-live")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(callCount).toBe(2));
    const message = screen.getByText("Couldn't read log lines");
    expect(message.getAttribute("aria-live")).toBe("polite");
  });

  it("get_default_config failure renders the disabled-reason note by Reset to defaults, which stays disabled", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return Promise.reject("defaults endpoint down");
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    const message = await screen.findByText("Defaults unavailable — reset disabled");
    expect(message.getAttribute("aria-live")).toBeNull();
    expect(
      (screen.getByRole("button", { name: "Reset to defaults" }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("connector-health poll: repeated identical failures collapse to one transition; recovery is a second transition (transition-only)", async () => {
    // Fake timers from before render, scoped to setInterval/clearInterval
    // only — this test never navigates sections, so AnimatePresence's
    // exit/enter transition (which needs real setTimeout/requestAnimationFrame
    // ticks) is never in the picture. Only the poll's own setInterval cadence
    // needs to be under our control. The state transitions asserted below
    // happen in SettingsApp regardless of which section is mounted to
    // visualize them — the DOM-visible wiring itself gets its own test below.
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    try {
      const debugSpy = vi.spyOn(console, "debug").mockImplementation(() => {});
      let healthCalls = 0;
      mockIPC((command) => {
        if (command === "get_config") return config;
        if (command === "get_secret_status") return unsetSecrets;
        if (command === "get_default_config") return rustConfigDefaults;
        if (command === "get_connector_health") {
          healthCalls += 1;
          if (healthCalls <= 2) return Promise.reject("network down");
          return { lastAttemptMs: 1000, lastSuccessMs: 1000, consecutiveFailures: 0 };
        }
      });

      render(<SettingsApp />);
      await flush();

      const transitionCalls = () =>
        debugSpy.mock.calls.filter((call) => call[0] === "[action-status:connector-health]").length;

      // mount-time fetchHealth() is the first poll — already failed.
      expect(healthCalls).toBe(1);
      expect(transitionCalls()).toBe(1);

      // second poll, 5s later: identical failure — must NOT add a transition.
      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000);
      });
      await flush();
      expect(healthCalls).toBe(2);
      expect(transitionCalls()).toBe(1);

      // third poll, 5s later: succeeds — exactly one recovery transition.
      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000);
      });
      await flush();
      expect(healthCalls).toBe(3);
      expect(transitionCalls()).toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("connector-health read failure renders 'Health unavailable' inline, with no aria-live (a poll is never user-initiated)", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
      if (command === "get_default_config") return rustConfigDefaults;
      if (command === "get_connector_health") return Promise.reject("network down");
    });
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Connectors & Keys" }));
    await screen.findByRole("heading", { level: 1, name: "Connectors & Keys" });

    const message = await screen.findByText("Health unavailable");
    expect(message.getAttribute("aria-live")).toBeNull();
  });
});

// plan 109: the emulated role="group"/"list"/"table" markup (and its
// lint suppressions) is gone — these pin the *native* element
// relationships (fieldset/legend, ul/li, table/thead/tbody/th/td, and
// label-to-control) that replaced it, not just that the ARIA roles still
// resolve.
describe("SettingsApp — native semantic markup (plan 109)", () => {
  it("the Scale segmented control is a real <fieldset> named by its <legend>", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    await screen.findByRole("heading", { level: 1, name: "Appearance" });

    const scaleToggle = await screen.findByRole("group", { name: "Scale" });
    expect(scaleToggle.tagName).toBe("FIELDSET");
    const legend = scaleToggle.querySelector("legend");
    expect(legend).toBeTruthy();
    expect(legend?.textContent).toBe("Scale");
    expect(legend?.parentElement).toBe(scaleToggle);
  });

  it("a Priority toggle is a real <fieldset>, and its visible ControlCopy label resolves to it via getByLabelText", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Cmux" }));
    await screen.findByRole("heading", { level: 1, name: "Cmux" });

    const priorityToggle = await screen.findByLabelText("Priority");
    expect(priorityToggle.tagName).toBe("FIELDSET");
    const legend = priorityToggle.querySelector("legend");
    expect(legend).toBeTruthy();
    expect(legend?.textContent).toBe("Priority");
  });

  it("rotation order items are real <li> elements that belong to a real <ul>", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    const list = screen.getByRole("list", { name: "Rotation order" });
    expect(list.tagName).toBe("UL");
    const items = screen.getAllByRole("listitem");
    expect(items.length).toBeGreaterThan(0);
    for (const item of items) {
      expect(item.tagName).toBe("LI");
      expect(item.parentElement).toBe(list);
    }
  });

  it("the shortcuts cheatsheet is a real <table> with thead/tbody/th/td belonging to it", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Shortcuts" }));
    await screen.findByRole("heading", { level: 1, name: "Shortcuts" });

    // AnimatePresence mode="wait" swaps the section body in after the h1
    // (which isn't animated) already shows the new title, so wait for
    // the table itself rather than racing the exit/enter transition.
    const table = await screen.findByRole("table", { name: "Keyboard shortcuts" });
    expect(table.tagName).toBe("TABLE");

    const thead = table.querySelector("thead");
    const tbody = table.querySelector("tbody");
    expect(thead?.parentElement).toBe(table);
    expect(tbody?.parentElement).toBe(table);

    const headerCells = within(table).getAllByRole("columnheader");
    expect(headerCells.map((cell) => cell.textContent)).toEqual(["Keys", "Action", "Status"]);
    for (const cell of headerCells) {
      expect(cell.tagName).toBe("TH");
      expect(cell.parentElement?.parentElement).toBe(thead);
    }

    const rowHeaderCells = within(table).getAllByRole("rowheader");
    expect(rowHeaderCells.length).toBeGreaterThan(0);
    for (const cell of rowHeaderCells) {
      expect(cell.tagName).toBe("TH");
      expect(cell.getAttribute("scope")).toBe("row");
      expect(cell.parentElement?.parentElement).toBe(tbody);
    }

    const dataCells = within(table)
      .getAllByRole("cell")
      .filter((cell) => cell.tagName === "TD");
    expect(dataCells.length).toBeGreaterThan(0);
    for (const cell of dataCells) {
      expect(cell.parentElement?.parentElement).toBe(tbody);
    }
  });
});

// plan 112 Step 4: the shadcn Switch contract, in two parts —
// (a) through the app's own ToggleControl call sites (accessible name,
// checked-state reflection — already covered by the isChecked() round-
// trip tests above); (b) here, direct primitive-level coverage of the
// pieces isChecked() alone doesn't prove: real button/role semantics
// (keyboard-operable by native browser behavior, not a custom
// keydown handler), and disabled behavior — ToggleControl itself never
// passes `disabled` today (grep confirms no call site does), so
// disabled coverage renders the shadcn Switch + Label pair directly,
// the same components ToggleControl composes.
describe("SettingsApp — shadcn Switch contract (plan 112 Step 4)", () => {
  it("is a real role=switch button, named by an associated Label, reflecting aria-checked", () => {
    render(
      <>
        <Label htmlFor="demo-switch">Demo switch</Label>
        <Switch id="demo-switch" checked={false} onCheckedChange={() => {}} />
      </>,
    );

    const toggle = screen.getByLabelText("Demo switch");
    expect(toggle.tagName).toBe("BUTTON");
    expect(toggle.getAttribute("type")).toBe("button");
    expect(toggle.getAttribute("role")).toBe("switch");
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });

  it("clicking calls onCheckedChange with the flipped value — the same activation a Space/Enter key press dispatches on any native <button> (browser default, not a custom key handler)", () => {
    const onCheckedChange = vi.fn();
    render(
      <>
        <Label htmlFor="demo-switch-2">Demo switch two</Label>
        <Switch id="demo-switch-2" checked={false} onCheckedChange={onCheckedChange} />
      </>,
    );

    const toggle = screen.getByLabelText("Demo switch two");
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(toggle);
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it("a disabled Switch carries aria-disabled/disabled semantics and ignores activation", () => {
    const onCheckedChange = vi.fn();
    render(
      <>
        <Label htmlFor="demo-switch-3">Demo switch three</Label>
        <Switch id="demo-switch-3" checked={false} disabled onCheckedChange={onCheckedChange} />
      </>,
    );

    const toggle = screen.getByLabelText("Demo switch three") as HTMLButtonElement;
    expect(toggle.disabled).toBe(true);
    fireEvent.click(toggle);
    expect(onCheckedChange).not.toHaveBeenCalled();
  });

  it("every General-section toggle in the live app is a real role=switch button reflecting its loaded checked state", async () => {
    mockLoads();
    render(<SettingsApp />);

    const startPaused = await screen.findByLabelText("Start paused");
    expect(startPaused.tagName).toBe("BUTTON");
    expect(startPaused.getAttribute("role")).toBe("switch");
    expect(startPaused.getAttribute("aria-checked")).toBe("true"); // fixture: start_paused true

    const hideWhenIdle = screen.getByLabelText("Hide overlay when idle");
    expect(hideWhenIdle.getAttribute("aria-checked")).toBe("true"); // fixture: resting_state "notch"
  });
});
