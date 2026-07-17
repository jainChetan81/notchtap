import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { KeyRound, Save, Settings } from "lucide-react";

export interface RssFeedConfig {
  url: string;
  source: string | null;
  category: string | null;
}

export interface Config {
  port: number;
  default_ttl: number;
  max_queued_per_tier: number;
  detect_path: string;
  start_paused: boolean;
  espn_enabled: boolean;
  espn_leagues: string[];
  espn_poll_secs: number;
  rss_enabled: boolean;
  rss_feeds: RssFeedConfig[];
  rss_poll_secs: number;
  rss_ttl_secs: number;
  rss_max_per_poll: number;
  connectors: {
    telegram: {
      enabled: boolean;
    };
  };
}

export interface SecretStatus {
  openrouter_api_key: string | null;
  telegram_bot_token: string | null;
  telegram_chat_id: string | null;
}

type SecretField = keyof SecretStatus;

const secretRows: ReadonlyArray<{
  field: SecretField;
  id: string;
  label: string;
  placeholder: string;
}> = [
  {
    field: "openrouter_api_key",
    id: "openrouter-key",
    label: "OpenRouter API key",
    placeholder: "Enter a new key",
  },
  {
    field: "telegram_bot_token",
    id: "telegram-token",
    label: "Telegram bot token",
    placeholder: "Enter a replacement token",
  },
  {
    field: "telegram_chat_id",
    id: "telegram-chat-id",
    label: "Telegram chat id",
    placeholder: "Enter a new chat id",
  },
];

function lines(value: string): string[] {
  return value
    .split("\n")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function errorList(error: unknown): string[] {
  if (Array.isArray(error)) {
    return error.map(String);
  }
  return [typeof error === "string" ? error : "settings could not be saved"];
}

function Section({
  id,
  label,
  code,
  children,
}: {
  id: string;
  label: string;
  code: string;
  children: ReactNode;
}) {
  return (
    <section className="settings-section" aria-labelledby={id}>
      <div className="section-heading">
        <h2 id={id} className="section-label">
          {label}
        </h2>
        <div className="section-code">{code}</div>
      </div>
      {children}
    </section>
  );
}

function Switch({ id, label, checked, onChange }: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="switch-control" htmlFor={id}>
      <input
        id={id}
        type="checkbox"
        aria-label={label}
        checked={checked}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span className="switch" aria-hidden="true" />
    </label>
  );
}

function ControlCopy({ htmlFor, name, help }: { htmlFor: string; name: string; help: string }) {
  return (
    <div>
      <label className="control-name" htmlFor={htmlFor}>{name}</label>
      <span className="control-help">{help}</span>
    </div>
  );
}

function NumberControl({
  id,
  name,
  help,
  value,
  min,
  max,
  unit,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  onChange: (value: number) => void;
}) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <div className="number-field">
        <input
          id={id}
          type="number"
          min={min}
          max={max}
          value={value}
          inputMode="numeric"
          onChange={(event) => onChange(Number(event.currentTarget.value))}
        />
        {unit ? <span className="unit">{unit}</span> : null}
      </div>
    </div>
  );
}

function ToggleControl({
  id,
  name,
  help,
  label,
  checked,
  onChange,
}: {
  id: string;
  name: string;
  help: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="control-row">
      <ControlCopy htmlFor={id} name={name} help={help} />
      <Switch id={id} label={label} checked={checked} onChange={onChange} />
    </div>
  );
}

function ErrorPanel({ errors }: { errors: string[] }) {
  return (
    <AnimatePresence initial={false}>
      {errors.length > 0 ? (
        <motion.div
          className="error-panel"
          role="alert"
          aria-live="assertive"
          initial={{ opacity: 0, y: -4 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
        >
          <div className="error-title">Config rejected</div>
          <ul>{errors.map((error) => <li key={error}>{error}</li>)}</ul>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

function SecretRow({
  field,
  id,
  label,
  placeholder,
  status,
  onSaved,
}: {
  field: SecretField;
  id: string;
  label: string;
  placeholder: string;
  status: string | null;
  onSaved: () => Promise<void>;
}) {
  const [value, setValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function saveSecret() {
    setSaving(true);
    setError(null);
    try {
      await invoke("set_secret", { field, value });
      setValue("");
      await onSaved();
    } catch (reason) {
      setError(typeof reason === "string" ? reason : "secret could not be saved");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="secret-row">
      <div className="secret-meta">
        <label className="secret-label" htmlFor={id}>{label}</label>
        <span className={`status-chip${status ? " is-set" : ""}`} aria-live="polite">
          {status ?? "unset"}
        </span>
      </div>
      <div className="secret-controls">
        <input
          id={id}
          className="secret-input"
          type="password"
          autoComplete="new-password"
          placeholder={placeholder}
          value={value}
          onChange={(event) => setValue(event.currentTarget.value)}
        />
        <button
          className="secondary-button"
          type="button"
          aria-label={`Save ${label}`}
          disabled={saving || value.trim().length === 0}
          onClick={() => void saveSecret()}
        >
          <Save aria-hidden="true" />
          {saving ? "Saving…" : "Save key"}
        </button>
      </div>
      {error ? <div className="secret-error" role="alert">{error}</div> : null}
    </div>
  );
}

export function SettingsApp() {
  const [config, setConfig] = useState<Config | null>(null);
  const [secretStatus, setSecretStatus] = useState<SecretStatus | null>(null);
  const [espnLeaguesText, setEspnLeaguesText] = useState("");
  const [rssFeedsText, setRssFeedsText] = useState("");
  const [errors, setErrors] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  async function refreshSecretStatus() {
    setSecretStatus(await invoke<SecretStatus>("get_secret_status"));
  }

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Config>("get_config"),
      invoke<SecretStatus>("get_secret_status"),
    ]).then(([loadedConfig, loadedStatus]) => {
      if (active) {
        setConfig(loadedConfig);
        setSecretStatus(loadedStatus);
        setEspnLeaguesText(loadedConfig.espn_leagues.join("\n"));
        setRssFeedsText(loadedConfig.rss_feeds.map((feed) => feed.url).join("\n"));
      }
    }).catch((reason: unknown) => {
      if (active) setErrors(errorList(reason));
    });
    return () => {
      active = false;
    };
  }, []);

  function patchConfig(patch: Partial<Config>) {
    setConfig((current) => current ? { ...current, ...patch } : current);
  }

  async function saveConfig(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!config) return;
    const submittedConfig: Config = {
      ...config,
      espn_leagues: lines(espnLeaguesText),
      rss_feeds: lines(rssFeedsText).map((url) =>
        config.rss_feeds.find((feed) => feed.url === url) ?? { url, source: null, category: null },
      ),
    };
    setSaving(true);
    setErrors([]);
    try {
      await invoke("save_config_and_relaunch", { config: submittedConfig });
    } catch (reason) {
      setErrors(errorList(reason));
      setSaving(false);
    }
  }

  return (
    <MotionConfig reducedMotion="user" transition={{ duration: 0.24, ease: [0.22, 1, 0.36, 1] }}>
      <main className="settings-window" aria-labelledby="window-title">
        <header className="titlebar">
          <div className="traffic-lights" aria-hidden="true"><span /><span /><span /></div>
          <div className="title-copy">
            <div className="eyebrow">Control panel / v5</div>
            <h1 id="window-title">notchtap settings</h1>
          </div>
          <div className="title-icon" aria-hidden="true"><Settings /></div>
          <div className="system-track" aria-hidden="true">
            <span className="lit" /><span className="lit" /><span className={saving ? "lit" : undefined} />
          </div>
        </header>

        {config ? (
          <form id="settings-form" className="settings-content" noValidate onSubmit={(event) => void saveConfig(event)}>
            <Section id="engine-heading" label="Engine" code="CORE / 01">
              <ToggleControl
                id="start-paused"
                name="Start paused"
                help="Launches with promotion paused; the tray reads Resume."
                label="Start paused"
                checked={config.start_paused}
                onChange={(start_paused) => patchConfig({ start_paused })}
              />
              <NumberControl id="default-ttl" name="Default TTL" help="Time each one-shot notification occupies the slot." value={config.default_ttl} min={1} max={3600} unit="SEC" onChange={(default_ttl) => patchConfig({ default_ttl })} />
              <NumberControl id="port" name="Listener port" help="Local loopback endpoint used by the notchtap CLI." value={config.port} min={1024} max={65535} onChange={(port) => patchConfig({ port })} />
              <NumberControl id="queue-cap" name="Queue cap per tier" help="Maximum waiting items in each priority lane." value={config.max_queued_per_tier} min={1} max={1000} onChange={(max_queued_per_tier) => patchConfig({ max_queued_per_tier })} />
            </Section>

            <Section id="football-heading" label="Football" code="POLLER / 02">
              <ToggleControl id="espn-enabled" name="ESPN scores" help="Poll watched leagues for score and match-state changes." label="Enable ESPN scores" checked={config.espn_enabled} onChange={(espn_enabled) => patchConfig({ espn_enabled })} />
              <div className="textarea-field">
                <ControlCopy htmlFor="espn-leagues" name="League codes" help="One ESPN league code per line." />
                <div>
                  <textarea id="espn-leagues" spellCheck={false} value={espnLeaguesText} onChange={(event) => setEspnLeaguesText(event.currentTarget.value)} />
                  <div className="range">ONE CODE / LINE</div>
                </div>
              </div>
              <NumberControl id="poll-secs" name="Poll interval" help="How often enabled leagues are checked." value={config.espn_poll_secs} min={5} max={3600} unit="SEC" onChange={(espn_poll_secs) => patchConfig({ espn_poll_secs })} />
            </Section>

            <Section id="news-heading" label="News" code="POLLER / 03">
              <ToggleControl id="rss-enabled" name="RSS news" help="Poll configured feeds and rotate fresh headlines through the slot." label="Enable RSS news" checked={config.rss_enabled} onChange={(rss_enabled) => patchConfig({ rss_enabled })} />
              <div className="textarea-field">
                <ControlCopy htmlFor="rss-feeds" name="Feeds" help="One RSS feed URL per line." />
                <div>
                  <textarea id="rss-feeds" spellCheck={false} value={rssFeedsText} onChange={(event) => setRssFeedsText(event.currentTarget.value)} />
                  <div className="range">ONE HTTP(S) URL / LINE</div>
                </div>
              </div>
              <NumberControl id="rss-poll-secs" name="Poll interval" help="How often configured feeds are checked." value={config.rss_poll_secs} min={5} max={3600} unit="SEC" onChange={(rss_poll_secs) => patchConfig({ rss_poll_secs })} />
              <NumberControl id="rss-ttl-secs" name="Headline TTL" help="Slot time per headline." value={config.rss_ttl_secs} min={1} max={3600} unit="SEC" onChange={(rss_ttl_secs) => patchConfig({ rss_ttl_secs })} />
              <NumberControl id="rss-max-per-poll" name="Max per poll" help="New headlines accepted from one poll pass." value={config.rss_max_per_poll} min={1} max={100} onChange={(rss_max_per_poll) => patchConfig({ rss_max_per_poll })} />
            </Section>

            <Section id="connectors-heading" label="Connectors" code="OUTBOUND / 04">
              <div className="connector-box">
                <div>
                  <span className="config-stamp">Config · relaunch-applied</span>
                  <ControlCopy htmlFor="telegram-enabled" name="Telegram enabled" help="Forward every accepted event after the app relaunches." />
                </div>
                <Switch
                  id="telegram-enabled"
                  label="Enable Telegram connector"
                  checked={config.connectors.telegram.enabled}
                  onChange={(enabled) => patchConfig({ connectors: { telegram: { enabled } } })}
                />
              </div>
            </Section>

            <Section id="secrets-heading" label="Secrets" code="WRITE ONLY / 05">
              <div className="secrets-intro">
                <span className="key-mark" aria-hidden="true"><KeyRound /></span>
                <span>Keys are never displayed back. Saving a key writes it immediately; status shows only a masked suffix.</span>
              </div>
              {secretRows.map((row) => (
                <SecretRow
                  key={row.field}
                  {...row}
                  status={secretStatus?.[row.field] ?? null}
                  onSaved={refreshSecretStatus}
                />
              ))}
            </Section>
            <ErrorPanel errors={errors} />
          </form>
        ) : (
          <div className="loading-state" role="status">Loading settings…</div>
        )}

        <footer className="settings-footer">
          <div>
            <div className="footer-hint">Saving restarts the app so every config change is live.</div>
            {saving ? <div className="save-state" role="status">saved — relaunching…</div> : null}
          </div>
          <button className="primary-button" type="submit" form="settings-form" disabled={!config || saving}>
            <Save aria-hidden="true" />
            <span>{saving ? "Relaunching…" : "Save & Relaunch"}</span>
          </button>
        </footer>
      </main>
    </MotionConfig>
  );
}
