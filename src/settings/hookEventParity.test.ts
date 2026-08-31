import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { describe, expect, it } from "vitest";

function readText(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf-8");
}

const doctorRs = readText("../../src-tauri/src/agents/providers/doctor.rs");
const agentsSectionTsx = readText("./sections/AgentsSection.tsx");

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

function rustEvents(constName: string): string[] {
  return captures(region(doctorRs, `pub const ${constName}`, "];"), /"([^"]+)"/g);
}

function jsonSnippetEvents(constName: string): string[] {
  return captures(
    region(agentsSectionTsx, `const ${constName} =`, "`;"),
    /"([A-Za-z]+)":\s*\[\{\s*"hooks"/g,
  );
}

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
