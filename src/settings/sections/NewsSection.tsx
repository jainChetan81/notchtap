import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ActionStatus, useActionStatus } from "../actionStatus";
import {
  ControlCopy,
  NumberControl,
  SettingsGroup,
  TestButtonRow,
  TextareaControl,
  ToggleControl,
} from "../controls/controls";
import { Segmented } from "../controls/Segmented";
import { settingsInvoke } from "../ipc";
import type { Config } from "../types";
import { PRIORITY_SEGMENT_OPTIONS } from "../types";

// plan 130 Step 3: an ad-hoc, unpersisted search — same ActionStatus
// pattern as ConnectorsSection's SecretRow (pending disables, success
// announces, the input clears only on success). Local component (not
// controls.tsx) since it's News-only, same precedent as SecretRow living
// in ConnectorsSection.tsx rather than the shared controls module.
function SearchNowRow() {
  const [query, setQuery] = useState("");
  const { status, run } = useActionStatus("search-news-now");
  const pending = status.state === "pending";

  async function search() {
    const trimmed = query.trim();
    if (trimmed.length === 0) return;
    await run(() => settingsInvoke("search_news_now", { query: trimmed }), {
      announce: true,
      okMessage: (count) => `${count} ${count === 1 ? "story" : "stories"} queued`,
      errorMessage: (reason) =>
        typeof reason === "string" ? reason : "search could not be completed",
    }).then((count) => {
      // The input clears only on success (plan 130 Step 3) — `run`
      // resolves `undefined` on a caught rejection, so a failed search
      // leaves the typed query in place to retry/edit.
      if (count !== undefined) setQuery("");
    });
  }

  return (
    <div className="search-now-row border-t border-border/60 pt-[11px] pb-3 first:border-t-0">
      <ControlCopy
        htmlFor="rss-search-now"
        name="Search now"
        help="Search Google News once, right now — merged into the same stream, not saved as a topic."
      />
      <div className="search-now-controls mt-2 grid grid-cols-[minmax(0,1fr)_auto] gap-[7px]">
        <Input
          id="rss-search-now"
          value={query}
          placeholder="e.g. aston villa transfers"
          onChange={(event) => setQuery(event.currentTarget.value)}
          className="search-now-input h-[31px] rounded-sm border-input bg-input/20 font-mono text-fs-secondary font-[560] text-foreground"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="text-fs-secondary"
          disabled={pending || query.trim().length === 0}
          onClick={() => void search()}
        >
          {pending ? "Searching…" : "Search"}
        </Button>
      </div>
      <ActionStatus status={status} className="search-now-status" />
    </div>
  );
}

export function NewsSection({
  config,
  feedsText,
  topicsText,
  patchConfig,
  setFeedsText,
  setTopicsText,
}: {
  config: Config;
  feedsText: string;
  topicsText: string;
  patchConfig: (patch: Partial<Config>) => void;
  setFeedsText: (value: string) => void;
  setTopicsText: (value: string) => void;
}) {
  return (
    <SettingsGroup title="RSS polling">
      <ToggleControl
        id="rss-enabled"
        name="RSS news"
        help="Poll configured feeds and rotate fresh headlines through the slot."
        label="Enable RSS news"
        checked={config.rss_enabled}
        onChange={(rss_enabled) => patchConfig({ rss_enabled })}
      />
      <TextareaControl
        id="rss-feeds"
        name="Feeds"
        help="Use one complete HTTP(S) feed URL per line."
        value={feedsText}
        caption="one feed URL per line"
        onChange={setFeedsText}
      />
      <TextareaControl
        id="rss-topics"
        name="Topics"
        help="Searched via Google News and merged into the same stream. Leave Feeds empty for topic-only news."
        value={topicsText}
        caption="one topic per line, e.g. aston villa transfers"
        onChange={setTopicsText}
      />
      <NumberControl
        id="rss-poll-secs"
        name="Poll interval"
        help="How often feeds/topics are checked — new stories appear in a burst after each check. 600 = news every 10 minutes."
        value={config.rss_poll_secs}
        min={5}
        max={3600}
        unit="SEC"
        onChange={(rss_poll_secs) => patchConfig({ rss_poll_secs })}
      />
      <NumberControl
        id="rss-ttl-secs"
        name="Headline rotation"
        help="How long each headline occupies the slot."
        value={config.rss_ttl_secs}
        min={1}
        max={3600}
        unit="SEC"
        onChange={(rss_ttl_secs) => patchConfig({ rss_ttl_secs })}
      />
      <NumberControl
        id="rss-max-per-poll"
        name="Maximum per poll"
        help="Burst cap: collecting more than this in one interval drops the extras — scale it up if you lengthen the poll interval."
        value={config.rss_max_per_poll}
        min={1}
        max={100}
        unit="ITEMS"
        onChange={(rss_max_per_poll) => patchConfig({ rss_max_per_poll })}
      />
      <Segmented
        id="rss-priority"
        name="Priority"
        help="Which tier a waiting headline promotes in."
        options={PRIORITY_SEGMENT_OPTIONS}
        value={config.rss_priority}
        onChange={(rss_priority) => patchConfig({ rss_priority })}
      />
      <TestButtonRow
        name="Test news notification"
        help="Send a one-off news headline to the overlay."
        source="news"
      />
      <SearchNowRow />
    </SettingsGroup>
  );
}
