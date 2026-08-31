export type SourceOriginToken = "manual" | "football" | "agent" | "news";

export const SOURCE_ORIGIN_COLORS = {
  manual: "#0a84ff",
  football: "#7fe08d",
  agent: "#f0c46a",
  news: "#ff6b57",
} satisfies Record<SourceOriginToken, string>;

export type SourceRuntimeToken = "claude-code" | "codex" | "kimi" | "opencode";

export const SOURCE_RUNTIME_COLORS = {
  "claude-code": "#d97757",
  codex: "#10a37f",
  kimi: "#f5f7fa",
  opencode: "#9d7cd8",
} satisfies Record<SourceRuntimeToken, string>;

export type SourceCategoryToken =
  | "politics"
  | "tech"
  | "sports"
  | "business"
  | "world"
  | "science"
  | "generic";

export const SOURCE_CATEGORY_COLORS = {
  politics: "#7c9df5",
  tech: "#5fd4e8",
  sports: "#7fe08d",
  business: "#f0c46a",
  world: "#c99df0",
  science: "#f2a2c8",
  generic: "#aab3bd",
} satisfies Record<SourceCategoryToken, string>;
