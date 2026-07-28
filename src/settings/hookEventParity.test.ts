// Plan 152: string-level parity pin between `doctor.rs`'s expected hook
// event consts (what `notchtap-agent doctor` counts as "wired") and the
// setup snippets AgentsSection.tsx tells the user to paste. The two lists
// are hand-synced, so without this pin a runtime could gain or lose a hook
// event in the UI and doctor would silently keep reporting the old count.
//
// Same cheap register as src/lib/sourceColors.test.ts — read the files as
// text, no parser — but region-scoped rather than whole-file: the Claude
// Code and Kimi event sets are IDENTICAL, so a whole-file scan could not
// tell which list an event name belonged to. Each side is sliced down to
// its own named constant first, then the two are compared as sets.
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readText(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf-8");
}

const doctorRs = readText("../../src-tauri/src/agents/providers/doctor.rs");
const agentsSectionTsx = readText("./sections/AgentsSection.tsx");

/** The text between a region's opening marker and its closing one. */
function region(source: string, start: string, end: string): string {
  const from = source.indexOf(start);
  if (from === -1) throw new Error(`region start not found: ${start}`);
  const to = source.indexOf(end, from + start.length);
  if (to === -1) throw new Error(`region end not found for: ${start}`);
  return source.slice(from, to);
}

function captures(text: string, pattern: RegExp): string[] {
  return [...text.matchAll(pattern)].map((match) => match[1]);
}

/** Every double-quoted string inside a `pub const NAME: [&str; N] = [...];`. */
function rustEvents(constName: string): string[] {
  return captures(region(doctorRs, `pub const ${constName}`, "];"), /"([^"]+)"/g);
}

/**
 * Every `"EventName": [{ "hooks"` key in a JSON setup snippet template
 * literal. The trailing `"hooks"` is required so the nested
 * `"hooks": [{ "type": ... }]` array — which shares the outer shape — is
 * not mistaken for an event name.
 */
function jsonSnippetEvents(constName: string): string[] {
  return captures(
    region(agentsSectionTsx, `const ${constName} =`, "`;"),
    /"([A-Za-z]+)":\s*\[\{\s*"hooks"/g,
  );
}

/** Every `event = "EventName"` value in the Kimi TOML setup snippet. */
function tomlSnippetEvents(constName: string): string[] {
  return captures(region(agentsSectionTsx, `const ${constName} =`, "`;"), /event = "([^"]+)"/g);
}

describe("doctor.rs hook events match the Settings setup snippets", () => {
  it("claude-code: ten events", () => {
    const fromDoctor = rustEvents("CLAUDE_CODE_HOOK_EVENTS");
    const fromSnippet = jsonSnippetEvents("CLAUDE_CODE_SNIPPET");
    expect(fromDoctor.length).toBe(10);
    expect(new Set(fromDoctor)).toEqual(new Set(fromSnippet));
  });

  it("codex: eight events", () => {
    const fromDoctor = rustEvents("CODEX_HOOK_EVENTS");
    const fromSnippet = jsonSnippetEvents("CODEX_SNIPPET");
    expect(fromDoctor.length).toBe(8);
    expect(new Set(fromDoctor)).toEqual(new Set(fromSnippet));
  });

  it("kimi: ten events", () => {
    const fromDoctor = rustEvents("KIMI_HOOK_EVENTS");
    const fromSnippet = tomlSnippetEvents("KIMI_SNIPPET");
    expect(fromDoctor.length).toBe(10);
    expect(new Set(fromDoctor)).toEqual(new Set(fromSnippet));
  });
});
