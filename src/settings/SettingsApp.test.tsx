import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type Config, type HistoryEntry, type SecretStatus, SettingsApp } from "./SettingsApp";

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

  it("Appearance section is enabled and renders all four preview cards", async () => {
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
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(true);
    // plan 085: the toggle reflects the loaded config's resting_state
    // ("notch" in this fixture) — checked means "hidden while idle".
    expect((screen.getByLabelText("Hide overlay when idle") as HTMLInputElement).checked).toBe(
      true,
    );
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

    const toggle = (await screen.findByLabelText("Hide overlay when idle")) as HTMLInputElement;
    expect(toggle.checked).toBe(true); // fixture config has resting_state: "notch"

    fireEvent.click(toggle);
    expect(toggle.checked).toBe(false);

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
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(true);
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
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("Hide overlay when idle") as HTMLInputElement).checked).toBe(
      false,
    );
    expect(selectedPriorityLabel(screen.getByLabelText("Manual push priority"))).toBe("Medium");
    expect(rotationOrderRowNames()).toEqual([
      "Football",
      "Manual / CLI push",
      "Weather",
      "Cmux (agent relay)",
      "News",
    ]);

    fireEvent.click(screen.getByRole("button", { name: "Football" }));
    expect(((await screen.findByLabelText("Enable ESPN scores")) as HTMLInputElement).checked).toBe(
      true,
    );
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
      .filter((el) => el.className === "history-title")
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

    const toggle = (await screen.findByLabelText(
      "Record notification history",
    )) as HTMLInputElement;
    expect(toggle.checked).toBe(false);

    fireEvent.click(toggle);
    expect(toggle.checked).toBe(true);

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

    const toggle = (await screen.findByLabelText("Enable now playing")) as HTMLInputElement;
    expect(toggle.checked).toBe(false);

    fireEvent.click(toggle);
    expect(toggle.checked).toBe(true);

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
    // scoped to leave requestAnimationFrame real — AnimatePresence's
    // section-swap transition depends on it, and faking it would freeze
    // the exit animation mid-flight, so the new section would never mount.
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearInterval", "clearTimeout"] });
    try {
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

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2600);
      });
      await flush();
      expect(screen.queryByText("Queued")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

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
