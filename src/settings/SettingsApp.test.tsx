import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsApp, type Config, type SecretStatus } from "./SettingsApp";

const config: Config = {
  port: 4321,
  default_ttl: 14,
  max_queued_per_tier: 75,
  detect_path: "/opt/notchtap-detect",
  start_paused: true,
  espn_enabled: false,
  espn_leagues: ["eng.1", "usa.1"],
  espn_poll_secs: 45,
  rss_enabled: true,
  rss_feeds: [
    { url: "https://example.com/world.xml", source: "Example", category: "world" },
    { url: "https://example.com/tech.xml", source: null, category: null },
  ],
  rss_poll_secs: 90,
  rss_ttl_secs: 18,
  rss_max_per_poll: 6,
  connectors: { telegram: { enabled: true } },
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
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("SettingsApp", () => {
  it("renders values from get_config", async () => {
    mockLoads({ ...unsetSecrets, telegram_bot_token: "set (…a1b2)" });
    render(<SettingsApp />);

    expect(await screen.findByDisplayValue("4321")).toBeTruthy();
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("Enable ESPN scores") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("Enable RSS news") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("Enable Telegram connector") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("League codes") as HTMLTextAreaElement).value).toBe("eng.1\nusa.1");
    expect((screen.getByLabelText("Feeds") as HTMLTextAreaElement).value).toBe(
      "https://example.com/world.xml\nhttps://example.com/tech.xml",
    );
    expect(screen.getByText("set (…a1b2)")).toBeTruthy();
  });

  it("renders every save rejection message", async () => {
    mockIPC((command) => {
      if (command === "get_config") return config;
      if (command === "get_secret_status") return unsetSecrets;
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

    const input = await screen.findByLabelText("OpenRouter API key") as HTMLInputElement;
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
});
