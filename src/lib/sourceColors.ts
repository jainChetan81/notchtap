// Plan 147: the TS twin of the overlay's colour system
// (src/overlay/source-identity.css + src/overlay/news-category.css) —
// consumed by the Settings window's Appearance swatches, which render
// outside `.card-root` and so can't just apply the CSS class and read
// the resulting `--cat` custom property back out. Every hex below is
// pinned against the actual CSS by src/lib/sourceColors.test.ts (a
// string-level parity scan, same register as
// src/overlayCardMirror.test.ts) so this table cannot silently drift
// from the rules that actually paint the overlay.
//
// Origin/runtime entries whose CSS rides a `var(--overlay-*)` /
// `var(--overlay-fg)` token (kimi, football, weather, manual, the
// agent fallback, sports, business) are resolved here to that token's
// underlying hex from vendor/shared-ui/design/tokens.css (:74-91) —
// each such entry is commented with the token name it mirrors.

// ---- non-news origins (source-identity.css) ----------------------------

export type SourceOriginToken = "manual" | "football" | "weather" | "agent" | "news";

export const SOURCE_ORIGIN_COLORS: Record<SourceOriginToken, string> = {
  // --overlay-blue ("CLI blue")
  manual: "#0a84ff",
  // --overlay-green
  football: "#7fe08d",
  // --overlay-amber
  weather: "#f0c46a",
  // --overlay-amber (runtime-unknown agent fallback, src-agent)
  agent: "#f0c46a",
  // --overlay-coral — the ORIGIN-level news identity (the news
  // status-dot's colour). On a card, news paints per-CATEGORY via
  // `cat-*` instead; this entry exists for origin-keyed surfaces
  // (History) where the category isn't the unit of identity.
  news: "#ff6b57",
};

// ---- agent runtimes (source-identity.css) -------------------------------

export type SourceRuntimeToken = "claude-code" | "codex" | "kimi" | "opencode";

export const SOURCE_RUNTIME_COLORS: Record<SourceRuntimeToken, string> = {
  // Anthropic "crail" terracotta
  "claude-code": "#d97757",
  // OpenAI green
  codex: "#10a37f",
  // --overlay-fg (Moonshot mono — restraint IS the brand)
  kimi: "#f5f7fa",
  // OpenCode's own TUI accent purple
  opencode: "#9d7cd8",
};

// ---- news categories (news-category.css) --------------------------------

export type SourceCategoryToken =
  | "politics"
  | "tech"
  | "sports"
  | "business"
  | "world"
  | "science"
  | "generic";

export const SOURCE_CATEGORY_COLORS: Record<SourceCategoryToken, string> = {
  politics: "#7c9df5",
  tech: "#5fd4e8",
  // --overlay-green
  sports: "#7fe08d",
  // --overlay-amber
  business: "#f0c46a",
  world: "#c99df0",
  science: "#f2a2c8",
  generic: "#aab3bd",
};
