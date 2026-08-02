import {
  Bot,
  CloudSun,
  Command,
  History,
  Info,
  KeyRound,
  ListOrdered,
  type LucideIcon,
  Newspaper,
  Palette,
  ScrollText,
  SlidersHorizontal,
  Trophy,
} from "lucide-react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { type FormEvent, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import brandMark from "../../assets/branding/notchtap-mark-128.png";
import { NOTCHTAP_EASE } from "../animationTiming";
import { ActionStatus, useActionStatus } from "./actionStatus";
import { settingsInvoke } from "./ipc";
import { AboutSection } from "./sections/AboutSection";
import { AgentsSection } from "./sections/AgentsSection";
import { AppearanceSection } from "./sections/AppearanceSection";
import { ConnectorsSection } from "./sections/ConnectorsSection";
import { DiagnosticsSection } from "./sections/DiagnosticsSection";
import { FootballSection } from "./sections/FootballSection";
import { GeneralSection } from "./sections/GeneralSection";
import { HistorySection } from "./sections/HistorySection";
import { NewsSection } from "./sections/NewsSection";
import { QueueSection } from "./sections/QueueSection";
import { ShortcutsSection } from "./sections/ShortcutsSection";
import { WeatherSection } from "./sections/WeatherSection";
import type { Config, SecretStatus } from "./types";

// Type re-exports (plan 119): SettingsApp.tsx used to define every wire
// type inline; they now live in ./types so sections/controls/ipc can
// import them without pulling in the whole shell. Re-exporting here keeps
// every external import path (notably SettingsApp.test.tsx) unchanged.
export type {
  AboutInfo,
  AppearanceConfig,
  Config,
  HistoryDetailItem,
  HistoryEntry,
  HistoryEspnMeta,
  HistoryEvent,
  HistoryEventMeta,
  HistoryRotationSpec,
  PriorityLevel,
  QueueItemSummary,
  RestingState,
  RssFeedConfig,
  SecretStatus,
  SourceKind,
  Units,
} from "./types";

type SectionId =
  | "general"
  | "football"
  | "news"
  | "weather"
  | "connectors"
  | "agents"
  | "shortcuts"
  | "appearance"
  | "diagnostics"
  | "history"
  | "queue"
  | "about";

const navigation: ReadonlyArray<{
  id: SectionId;
  label: string;
  icon: LucideIcon;
}> = [
  { id: "general", label: "General", icon: SlidersHorizontal },
  { id: "football", label: "Football", icon: Trophy },
  { id: "news", label: "News", icon: Newspaper },
  { id: "weather", label: "Weather", icon: CloudSun },
  { id: "connectors", label: "Connectors & Keys", icon: KeyRound },
  { id: "agents", label: "Agents", icon: Bot },
  { id: "shortcuts", label: "Shortcuts", icon: Command },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "diagnostics", label: "Diagnostics", icon: ScrollText },
  { id: "history", label: "History", icon: History },
  { id: "queue", label: "Queue", icon: ListOrdered },
  { id: "about", label: "About", icon: Info },
];

const sectionCopy: Record<SectionId, { index: string; title: string; description: string }> = {
  general: {
    index: "01",
    title: "General",
    description: "Control startup, the local listener, and how notifications rotate.",
  },
  football: {
    index: "02",
    title: "Football",
    description: "Choose the leagues and cadence used for live score checks.",
  },
  news: {
    index: "03",
    title: "News",
    description: "Manage RSS sources and the pace of headline delivery.",
  },
  weather: {
    index: "04",
    title: "Weather",
    description:
      "Show ambient conditions in the idle rail and alert on rain and temperature thresholds.",
  },
  connectors: {
    index: "05",
    title: "Connectors & Keys",
    description: "Manage write-only credentials for outbound integrations.",
  },
  agents: {
    index: "06",
    title: "Agents",
    description:
      "Accept coding-agent lifecycle events, tune notification priority, and set up each adapter.",
  },
  shortcuts: {
    index: "07",
    title: "Shortcuts",
    description: "A reference for the global controls available while notchtap runs.",
  },
  appearance: {
    index: "08",
    title: "Appearance",
    description: "Preview the overlay's shape and animations, and send live test notifications.",
  },
  diagnostics: {
    index: "09",
    title: "Diagnostics",
    description: "Read the app's recent log lines without leaving settings.",
  },
  history: {
    index: "10",
    title: "History",
    description: "Review and clear recorded past notifications.",
  },
  queue: {
    index: "11",
    title: "Queue",
    description: "See what's waiting behind the visible card, and skip or clear it.",
  },
  about: {
    index: "12",
    title: "About",
    description: "What notchtap is, how to use it, and live process/system stats.",
  },
};

function copyConfig(config: Config): Config {
  return {
    ...config,
    espn_leagues: [...config.espn_leagues],
    rss_feeds: config.rss_feeds.map((feed) => ({ ...feed })),
    rss_topics: [...config.rss_topics],
    rotation_order: [...config.rotation_order],
  };
}

function lines(value: string): string[] {
  return value
    .split("\n")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

// Normalized match key for the rss_feeds rebuild (plan 021): strips the
// hash and a single trailing slash so a cosmetic edit (trailing "/", a
// fragment) still matches the old entry and keeps its source/category.
// A substantive change (different path/host) still resets metadata —
// correct, it IS a different feed. Limitation: this can't distinguish
// "same feed, meaningfully re-hosted" from "different feed" — that's
// out of scope without a source/category editing UI (see plan notes).
function feedKey(url: string): string {
  try {
    const u = new URL(url);
    u.hash = "";
    return u.href.replace(/\/$/, "");
  } catch {
    return url.trim();
  }
}

function errorList(error: unknown): string[] {
  if (Array.isArray(error)) {
    return error.map(String);
  }
  return [typeof error === "string" ? error : "settings could not be saved"];
}

// plan 112 Step 3: keeps its motion.div lifecycle verbatim (AnimatePresence
// enter/exit is application state, not a CSS concern Tailwind replaces) —
// only the box styling is ported to utilities, Card-equivalent rather than
// a forced <Card> wrapper swap. border/bg follow the token table's
// "error -> text-destructive / border-destructive/40" row; bg-destructive/10
// is a decorative background-opacity composition (allowed — the restriction
// is on TEXT opacity, which would threaten contrast; this is a tinted panel
// fill, and text-destructive against it still measures ~7:1, unchanged from
// before).
function ErrorPanel({ errors }: { errors: string[] }) {
  return (
    <AnimatePresence initial={false}>
      {errors.length > 0 ? (
        <motion.div
          className="error-panel mx-4 mt-2.5 rounded-sm border border-destructive/40 bg-destructive/10 px-2.5 py-2.5 text-destructive"
          role="alert"
          aria-live="assertive"
          initial={{ opacity: 0, y: -3 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -3 }}
        >
          <div className="error-title mb-[5px] text-fs-caption font-bold tracking-[0.1em] uppercase">
            Config rejected
          </div>
          <ul className="m-0 pl-[15px] text-fs-secondary leading-[1.45]">
            {errors.map((error) => (
              <li key={error}>{error}</li>
            ))}
          </ul>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

export function SettingsApp() {
  const [activeSection, setActiveSection] = useState<SectionId>("general");
  const [config, setConfig] = useState<Config | null>(null);
  const [lastLoadedConfig, setLastLoadedConfig] = useState<Config | null>(null);
  const [defaults, setDefaults] = useState<Config | null>(null);
  const [secretStatus, setSecretStatus] = useState<SecretStatus | null>(null);
  const [espnLeaguesText, setEspnLeaguesText] = useState("");
  const [rssFeedsText, setRssFeedsText] = useState("");
  const [rssTopicsText, setRssTopicsText] = useState("");
  const [errors, setErrors] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  // Owned here, not inside AppearanceSection — so a reset's own live-apply
  // failure survives regardless of which section is open. See
  // AppearanceSection's applyAppearanceLive prop.
  const appearanceStatus = useActionStatus("appearance-live-apply");
  // Defaults-fetch is a passive, mount-only read (plan 108) — never
  // announced.
  const defaultsStatus = useActionStatus("defaults");

  function runAppearanceApply(scale: number, radius: number, opacity: number) {
    void appearanceStatus.run(() => settingsInvoke("set_appearance", { scale, radius, opacity }), {
      announce: true,
      showPending: false,
      errorMessage: () => "Live preview couldn't update — will apply on Save & Relaunch",
    });
  }

  function applyForm(nextConfig: Config) {
    const next = copyConfig(nextConfig);
    setConfig(next);
    setEspnLeaguesText(next.espn_leagues.join("\n"));
    setRssFeedsText(next.rss_feeds.map((feed) => feed.url).join("\n"));
    setRssTopicsText(next.rss_topics.join("\n"));
    setErrors([]);
  }

  async function refreshSecretStatus() {
    setSecretStatus(await settingsInvoke("get_secret_status"));
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: mount-only config loader — applyForm is re-created every render, so adding it would re-invoke get_config on every render.
  useEffect(() => {
    let active = true;
    Promise.all([settingsInvoke("get_config"), settingsInvoke("get_secret_status")])
      .then(([loadedConfig, loadedStatus]) => {
        if (active) {
          const loaded = copyConfig(loadedConfig);
          setLastLoadedConfig(loaded);
          applyForm(loaded);
          setSecretStatus(loadedStatus);
        }
      })
      .catch((reason: unknown) => {
        if (active) setErrors(errorList(reason));
      });
    // defaults are advisory (Reset-to-defaults only) — isolate their failure
    // so it can never block the rest of the panel from loading; the button
    // just stays disabled (see the footer's `disabled={!defaults || saving}`)
    // — but now the disabled state carries a visible reason (plan 108).
    void defaultsStatus.run(
      () =>
        settingsInvoke("get_default_config").then((loadedDefaults) => {
          if (active) setDefaults(copyConfig(loadedDefaults));
        }),
      {
        announce: false,
        showPending: false,
        errorMessage: () => "Defaults unavailable — reset disabled",
      },
    );
    return () => {
      active = false;
    };
  }, []);

  function patchConfig(patch: Partial<Config>) {
    setConfig((current) => (current ? { ...current, ...patch } : current));
  }

  function resetLoaded() {
    if (!lastLoadedConfig) return;
    // Form state and live-apply are separate concerns (plan 108 step A):
    // the form always resets from lastLoadedConfig regardless of whether
    // the live overlay could be updated to match — a failed live-apply
    // reports through appearanceStatus, it doesn't block the form reset.
    applyForm(lastLoadedConfig);
    const { card_scale, card_radius, card_opacity } = lastLoadedConfig.appearance;
    runAppearanceApply(card_scale, card_radius, card_opacity);
  }

  function resetDefaults() {
    if (!defaults) return;
    applyForm(defaults);
    const { card_scale, card_radius, card_opacity } = defaults.appearance;
    runAppearanceApply(card_scale, card_radius, card_opacity);
  }

  async function saveConfig(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!config) return;
    const submittedConfig: Config = {
      ...config,
      espn_leagues: lines(espnLeaguesText),
      rss_feeds: lines(rssFeedsText).map((url) => {
        // Match by normalized key, but keep the url the user actually typed
        // — only source/category carry over from the old entry (plan 021:
        // "the saved value stays what the user typed").
        const match = config.rss_feeds.find((feed) => feedKey(feed.url) === feedKey(url));
        return match
          ? { url, source: match.source, category: match.category }
          : { url, source: null, category: null };
      }),
      rss_topics: lines(rssTopicsText),
    };
    setSaving(true);
    setErrors([]);
    try {
      await settingsInvoke("save_config_and_relaunch", { config: submittedConfig });
    } catch (reason) {
      setErrors(errorList(reason));
      setSaving(false);
    }
  }

  const currentSection = sectionCopy[activeSection];

  return (
    <MotionConfig reducedMotion="user" transition={{ duration: 0.16, ease: NOTCHTAP_EASE }}>
      <main
        className="settings-window grid h-full w-full grid-cols-[140px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)] overflow-hidden bg-background max-[430px]:grid-cols-[122px_minmax(0,1fr)]"
        aria-labelledby="section-title"
      >
        <aside
          className="settings-sidebar grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)_auto] border-r border-border bg-sidebar"
          aria-label="Settings sections"
        >
          <div className="sidebar-brand flex min-h-[71px] items-center gap-2.5 border-b border-border/60 px-3.5 pt-[17px] pb-3.5 font-mono text-fs-body leading-none font-bold tracking-[0.09em] text-foreground uppercase not-italic max-[430px]:px-[10px]">
            <img
              src={brandMark}
              alt=""
              aria-hidden="true"
              className="brand-mark h-[26px] w-[26px] flex-none"
            />
            <span>notchtap</span>
          </div>
          <nav className="sidebar-nav flex min-h-0 flex-col gap-[3px] overflow-y-auto overscroll-contain px-2 py-3">
            {navigation.map((item) => {
              const Icon = item.icon;
              const selected = item.id === activeSection;
              return (
                <button
                  key={item.id}
                  className={cn(
                    "nav-item relative grid min-h-[38px] min-w-0 grid-cols-[16px_minmax(0,1fr)] items-center gap-2 rounded-md border-0 border-l-2 border-l-transparent bg-transparent py-[7px] pr-2 pl-[6px] text-left text-muted-foreground outline-none transition-[color,background-color,border-color,transform] duration-[140ms] ease-notchtap hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.97]",
                    selected && "is-active border-l-primary bg-primary/15 text-foreground",
                  )}
                  type="button"
                  aria-current={selected ? "page" : undefined}
                  onClick={() => setActiveSection(item.id)}
                >
                  <Icon aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={1.75} />
                  <span className="min-w-0 overflow-hidden font-[560] text-fs-body text-ellipsis leading-[1.25]">
                    {item.label}
                  </span>
                </button>
              );
            })}
          </nav>
          <div className="sidebar-meta border-t border-border/60 px-3.5 pt-[11px] pb-5 font-mono text-fs-secondary font-bold tracking-[0.1em] text-muted-foreground uppercase max-[430px]:px-[10px]">
            settings / v5
          </div>
        </aside>

        <div className="settings-pane grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] bg-background">
          {config ? (
            <form
              id="settings-form"
              className="settings-form grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)] grid-rows-[auto_auto_minmax(0,1fr)] overflow-hidden"
              noValidate
              onSubmit={(event) => void saveConfig(event)}
            >
              <header className="content-header border-b border-border bg-background px-5 pt-[22px] pb-[17px] max-[430px]:px-4">
                <div className="section-index mb-2 font-mono text-fs-secondary font-bold tracking-[0.11em] text-muted-foreground uppercase">
                  Settings / {currentSection.index}
                </div>
                <h1
                  id="section-title"
                  className="m-0 text-fs-title leading-[1.15] font-[650] tracking-[-0.018em] text-foreground"
                >
                  {currentSection.title}
                </h1>
                <p className="mt-1.5 mr-0 mb-0 ml-0 max-w-[290px] text-fs-body leading-[1.45] text-muted-foreground">
                  {currentSection.description}
                </p>
              </header>

              <ErrorPanel errors={errors} />

              <div className="section-scroll min-h-0 overflow-y-auto overscroll-contain">
                <AnimatePresence mode="wait" initial={false}>
                  <motion.div
                    className="section-content min-h-full px-4 pt-4 pb-6 max-[430px]:px-3"
                    key={activeSection}
                    initial={{ opacity: 0, x: 3 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: -2 }}
                  >
                    {activeSection === "general" ? (
                      <GeneralSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "football" ? (
                      <FootballSection
                        config={config}
                        leaguesText={espnLeaguesText}
                        patchConfig={patchConfig}
                        setLeaguesText={setEspnLeaguesText}
                      />
                    ) : null}
                    {activeSection === "news" ? (
                      <NewsSection
                        config={config}
                        feedsText={rssFeedsText}
                        topicsText={rssTopicsText}
                        patchConfig={patchConfig}
                        setFeedsText={setRssFeedsText}
                        setTopicsText={setRssTopicsText}
                      />
                    ) : null}
                    {activeSection === "weather" ? (
                      <WeatherSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "connectors" ? (
                      <ConnectorsSection
                        secretStatus={secretStatus}
                        refreshSecretStatus={refreshSecretStatus}
                      />
                    ) : null}
                    {activeSection === "agents" ? (
                      <AgentsSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "shortcuts" ? (
                      <ShortcutsSection config={config} patchConfig={patchConfig} />
                    ) : null}
                    {activeSection === "diagnostics" ? <DiagnosticsSection /> : null}
                    {activeSection === "history" ? <HistorySection config={config} /> : null}
                    {activeSection === "queue" ? <QueueSection /> : null}
                    {activeSection === "about" ? <AboutSection /> : null}
                    {activeSection === "appearance" ? (
                      // AppearanceSection now reads config.appearance directly
                      // (plan 119 Step 3) — Reset/Reset-to-defaults update it
                      // through the normal config-propagation re-render, same
                      // as every other section, so no remount key is needed.
                      // The live-apply status itself renders in the footer,
                      // not here — this section doesn't even exist while
                      // another section is open, but Reset/Reset to defaults
                      // (which also drive this same status) are footer
                      // buttons reachable from every section.
                      <AppearanceSection
                        config={config}
                        patchConfig={patchConfig}
                        applyAppearanceLive={runAppearanceApply}
                      />
                    ) : null}
                  </motion.div>
                </AnimatePresence>
              </div>
            </form>
          ) : (
            <div
              className="loading-state grid min-h-0 place-items-center text-fs-secondary leading-none font-bold tracking-[0.1em] text-muted-foreground uppercase not-italic"
              role="status"
            >
              Loading settings…
            </div>
          )}

          <footer className="settings-footer flex min-h-[57px] items-center gap-1.5 border-t border-border bg-background px-3.5 py-3 max-[430px]:px-[10px]">
            <Button
              type="submit"
              form="settings-form"
              disabled={!config || saving}
              className="text-fs-secondary max-[430px]:px-[7px] max-[430px]:text-fs-caption"
            >
              {saving ? "Relaunching…" : "Save & Relaunch"}
            </Button>
            <Button
              variant="outline"
              type="button"
              disabled={!lastLoadedConfig || saving}
              onClick={resetLoaded}
              className="text-fs-secondary max-[430px]:px-[7px] max-[430px]:text-fs-caption"
            >
              Reset
            </Button>
            <Button
              variant="outline"
              type="button"
              disabled={!defaults || saving}
              onClick={resetDefaults}
              className="text-fs-secondary max-[430px]:px-[7px] max-[430px]:text-fs-caption"
            >
              Reset to defaults
            </Button>
            <ActionStatus
              status={defaultsStatus.status}
              className="defaults-status mt-0"
              showPending={false}
            />
            {/* Always visible regardless of active section (plan 108 step A):
                Reset and Reset to defaults are footer buttons reachable from
                any section, and the appearance sliders are high-frequency —
                pending/ok never render here, only a deduplicated error,
                cleared by the next successful apply. */}
            <ActionStatus
              status={appearanceStatus.status}
              className="appearance-live-status mt-0"
              showPending={false}
            />
          </footer>
        </div>
      </main>
    </MotionConfig>
  );
}
