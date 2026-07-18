import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type Config, type SecretStatus, SettingsApp } from "./SettingsApp";

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
  rotation_order: ["news", "cmux", "manual", "football"],
  connectors: { telegram: { enabled: true } },
  appearance: { card_scale: 1, card_radius: 8, card_opacity: 0.9 },
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
  rotation_order: ["football", "manual", "cmux", "news"],
  connectors: { telegram: { enabled: false } },
  appearance: { card_scale: 1, card_radius: 16, card_opacity: 0.9 },
};

const unsetSecrets: SecretStatus = {
  openrouter_api_key: null,
  telegram_bot_token: null,
  telegram_chat_id: null,
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
    expect(
      screen.getByText(
        "Waiting items promote high → medium → low. Priority chooses the next turn; it never interrupts the visible item.",
      ),
    ).toBeTruthy();
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
    expect(selectedPriorityLabel(screen.getByLabelText("Manual push priority"))).toBe("Medium");
    expect(rotationOrderRowNames()).toEqual([
      "Football",
      "Manual / CLI push",
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
      "Football",
    ]);

    const rows = screen.getAllByRole("listitem");
    const [newsRow, cmuxRow, manualRow, footballRow] = rows;
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
      "Football",
    ]);
  });
});
