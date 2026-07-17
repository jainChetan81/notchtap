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
  it("renders sidebar navigation and switches among available sections", async () => {
    mockLoads();
    render(<SettingsApp />);

    expect(await screen.findByRole("heading", { level: 1, name: "General" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "General" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Football" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "News" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Connectors & Keys" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Shortcuts" })).toBeTruthy();

    const appearance = screen.getByRole("button", { name: "Appearance soon" }) as HTMLButtonElement;
    expect(appearance.disabled).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Football" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Football" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "News" }));
    expect(await screen.findByRole("heading", { level: 1, name: "News" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Connectors & Keys" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Connectors & Keys" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Shortcuts" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Shortcuts" })).toBeTruthy();
    expect(await screen.findByText("Expand or collapse the slot (manual)")).toBeTruthy();
    expect(await screen.findAllByText("planned · not implemented")).toHaveLength(4);
  });

  it("shows loaded values in General", async () => {
    mockLoads();
    render(<SettingsApp />);

    expect(await screen.findByDisplayValue("4321")).toBeTruthy();
    expect(screen.getByDisplayValue("14")).toBeTruthy();
    expect(screen.getByDisplayValue("75")).toBeTruthy();
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText("Waiting items promote high → medium → low. Priority chooses the next turn; it never interrupts the visible item.")).toBeTruthy();
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

    await screen.findByRole("heading", { level: 1, name: "General" });
    fireEvent.click(screen.getByRole("button", { name: "Connectors & Keys" }));

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

  it("Reset restores the values returned by get_config", async () => {
    mockLoads();
    render(<SettingsApp />);

    const port = await screen.findByLabelText("Listener port") as HTMLInputElement;
    fireEvent.change(port, { target: { value: "5555" } });
    expect(port.value).toBe("5555");

    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect((screen.getByLabelText("Listener port") as HTMLInputElement).value).toBe("4321");
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(true);
  });

  it("Reset to defaults applies the Rust Config defaults mirror", async () => {
    mockLoads();
    render(<SettingsApp />);

    await screen.findByDisplayValue("4321");
    fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));

    expect((screen.getByLabelText("Listener port") as HTMLInputElement).value).toBe("9789");
    expect((screen.getByLabelText("Rotation seconds") as HTMLInputElement).value).toBe("8");
    expect((screen.getByLabelText("Queue cap per priority tier") as HTMLInputElement).value).toBe("50");
    expect((screen.getByLabelText("Start paused") as HTMLInputElement).checked).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Football" }));
    expect((await screen.findByLabelText("Enable ESPN scores") as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText("Leagues") as HTMLTextAreaElement).value).toBe(
      "eng.1\nuefa.champions\nesp.1",
    );
  });
});
