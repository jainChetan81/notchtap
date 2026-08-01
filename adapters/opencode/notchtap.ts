// notchtap — OpenCode plugin adapter (v7 ticket 9 of 13, plan 141,
// `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md` §3, §4.1, §4.5).
//
// OpenCode's lifecycle surface is a plugin event bus, not command hooks
// (unlike Claude Code/Codex/Kimi, which post via the `notchtap-agent`
// Rust binary — see `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
// §4.1-§4.4). This file is that plugin: it normalizes OpenCode's bus
// events into the same schema-v1 `POST /agent/events` body the Rust
// helper sends, and posts them to the same loopback endpoint
// (`127.0.0.1:9789` by default, `NOTCHTAP_PORT` override), matching its
// network behavior, caps, sanitization, and fail-open semantics exactly.
//
// ## Install
//
// OpenCode loads plugins from (per https://opencode.ai/docs/plugins/,
// fetched 2026-07-26):
//   - project directory:  .opencode/plugins/  (drop this file there)
//   - global directory:   ~/.config/opencode/plugins/
//   - or as an npm package referenced from `plugin` in opencode.json
//     (project `opencode.json`, or global `~/.config/opencode/opencode.json`)
//
// Simplest local install: copy (or symlink) this file to
// `.opencode/plugins/notchtap.ts` in the project you want notifications
// from. No build step, no extra dependency — this module only uses
// runtime globals (`fetch`, `AbortController`, `crypto.randomUUID`,
// `process.env`) that OpenCode's Bun/Node runtime already provides.
//
// ## Structure
//
// Everything that decides WHAT gets sent is a pure function (bus event
// in, `AgentWireEvent | null` out) — testable without OpenCode
// installed, mirroring the Rust adapter's own pure/impure split
// (`src-tauri/src/agents/adapter.rs` is pure wire parsing; the HTTP
// layer is separate). Only `NotchtapPlugin` at the bottom binds those
// pure functions to OpenCode's actual hook shape and performs the
// (fire-and-forget, fail-open) network call.
//
// ## OpenCode plugin API this was built against
//
// https://opencode.ai/docs/plugins/ (and the same content indexed as
// context7 library `/websites/opencode_ai_plugins`), fetched 2026-07-26:
//
//   export const MyPlugin = async ({ project, client, $, directory, worktree }) => {
//     return {
//       event: async ({ event }) => { if (event.type === "session.idle") { ... } },
//       "tool.execute.before": async (input, output) => { ... },
//       "tool.execute.after": async (input, output) => { ... },
//     };
//   };
//
// i.e. session/permission lifecycle arrives through ONE `event` hook key
// keyed by a `event.type` discriminated union (not one hook method per
// event name), while tool execution has its own two dedicated hook
// keys taking `(input, output)`. The docs enumerate `event.type` values
// including every one plan 141 asks for (`permission.asked`,
// `permission.replied`, `session.created`, `session.updated`,
// `session.status`, `session.idle`, `session.error`,
// `session.deleted`) but do NOT publish the exact `event.properties`
// payload shape per type, or whether `tool.execute.after`'s `output`
// carries a success/failure flag. See "Known gaps" below for how this
// file handles that.
//
// ## Known gaps vs. the documented surface (report these, don't guess)
//
// - `event.properties` field shapes for the session/permission events
//   are undocumented. This file reads only conservatively-named,
//   plausible fields (`sessionID`, `info.id`, `error.name`, etc.) and
//   drops (`null`) an event rather than inventing a shape when the
//   session id can't be found — schema v1 requires `sessionId`.
// - `session.status`'s exact status vocabulary is undocumented. Per
//   the spec's "wording is never parsed to infer state" rule, this
//   file only reacts to an explicit `waiting_for_input` / `input_required`
//   token and drops everything else, rather than guessing at synonyms.
// - `tool.execute.after`'s `output` is documented with `title` /
//   `output` / `metadata` fields but no explicit error flag, so this
//   adapter cannot distinguish a failed tool call at that hook and
//   never emits `kind: "failed"` from it (spec: never infer state from
//   wording/heuristics on undocumented payloads).
// - No subagent lifecycle event is listed in plan 141's event list or
//   the plugin docs, matching the §1 matrix row ("subagent lifecycle:
//   not declared until verified") — this adapter never emits a
//   `subagent` field and never declares the `subagents` capability.
// - OpenCode plugin docs don't expose Host app identity, so this
//   adapter never sends a `host` field and never declares
//   `open_or_focus`.

// ---------------------------------------------------------------------
// Wire schema v1 (mirrors src-tauri/src/agents/adapter.rs exactly)
// ---------------------------------------------------------------------

export const SCHEMA_VERSION = 1 as const;
export const RUNTIME = "opencode" as const;
export const DEFAULT_PORT = 9789;
export const DELIVERY_TIMEOUT_MS = 750;

export type AgentEventKind =
  | "permission_requested"
  | "input_required"
  | "completed"
  | "failed"
  | "informational";

export type AgentSessionState =
  | "starting"
  | "working"
  | "waiting_for_permission"
  | "waiting_for_input"
  | "completed"
  | "failed"
  | "stale";

export type AgentCapability =
  | "session_lifecycle"
  | "permission_requests"
  | "input_required"
  | "completion"
  | "failure"
  | "tool_details"
  | "subagents"
  | "open_or_focus";

export interface WireDetail {
  label: string;
  value: string;
}

export interface WireProject {
  name?: string;
  cwd?: string;
}

export interface AgentWireEvent {
  schemaVersion: typeof SCHEMA_VERSION;
  eventId: string;
  runtime: typeof RUNTIME;
  sessionId: string;
  occurredAtMs: number;
  sequence?: number;
  nativeEvent: string;
  kind: AgentEventKind;
  state: AgentSessionState;
  summary?: string;
  details?: WireDetail[];
  capabilities?: AgentCapability[];
  project?: WireProject;
  terminal: boolean;
}

/** §1 matrix row for OpenCode: session lifecycle, permission requests,
 * session/status-derived input-required, idle/session-derived
 * completion, session-error-derived failure, and tool detail are
 * declared. Subagent lifecycle and Open/Focus are deliberately absent
 * — see this file's header "Known gaps". Never mutate this in place;
 * treat as a frozen constant so every emitted event declares the same
 * truthful set. */
export const OPENCODE_CAPABILITIES: readonly AgentCapability[] = Object.freeze([
  "session_lifecycle",
  "permission_requests",
  "input_required",
  "completion",
  "failure",
  "tool_details",
]);

// ---------------------------------------------------------------------
// Sanitization (mirrors adapter.rs's caps table, spec §3.2)
// ---------------------------------------------------------------------

const MAX_ID_BYTES = 256;
const MAX_SUMMARY_SCALARS = 500;
const MAX_NAME_OR_LABEL_SCALARS = 120;
const MAX_VALUE_SCALARS = 1024;
const MAX_DETAILS = 12;

/** Unicode "control" category only (matches Rust's `char::is_control`:
 * C0 controls + DEL + C1 controls), not the broader "whitespace" or
 * "format" categories — same scope as adapter.rs's `sanitize_trim`. */
// biome-ignore lint/suspicious/noControlCharactersInRegex: intentionally matching control characters to strip them
const CONTROL_CHARS = /[\u0000-\u001f\u007f-\u009f]/gu;

/** Trim outer whitespace FIRST, then strip control characters — same
 * order as adapter.rs's `sanitize_trim`, for the same reason: stripping
 * first could leave now-interior whitespace unindented at the edges. */
function sanitizeTrim(s: string): string {
  return s.trim().replace(CONTROL_CHARS, "");
}

/** Truncate to at most `max` Unicode scalar values without splitting a
 * surrogate pair — `Array.from` iterates by codepoint, not UTF-16 code
 * unit, so this is the JS equivalent of adapter.rs's `cap_scalars`. */
function capScalars(s: string, max: number): string {
  const chars = Array.from(s);
  return chars.length <= max ? s : chars.slice(0, max).join("");
}

/** Truncate to at most `maxBytes` UTF-8 bytes without splitting a
 * codepoint — the JS equivalent of adapter.rs's `cap_bytes`, used only
 * for opaque identifiers (never display text). */
function capBytes(s: string, maxBytes: number): string {
  const encoder = new TextEncoder();
  if (encoder.encode(s).length <= maxBytes) return s;
  const chars = Array.from(s);
  let out = "";
  let bytes = 0;
  for (const ch of chars) {
    const chBytes = encoder.encode(ch).length;
    if (bytes + chBytes > maxBytes) break;
    out += ch;
    bytes += chBytes;
  }
  return out;
}

function sanitizeId(s: string): string {
  return capBytes(sanitizeTrim(s), MAX_ID_BYTES);
}

function sanitizeNameOrLabel(s: string): string | undefined {
  const cleaned = capScalars(sanitizeTrim(s), MAX_NAME_OR_LABEL_SCALARS);
  return cleaned.length === 0 ? undefined : cleaned;
}

function sanitizeValue(s: string): string | undefined {
  const cleaned = capScalars(sanitizeTrim(s), MAX_VALUE_SCALARS);
  return cleaned.length === 0 ? undefined : cleaned;
}

function sanitizeSummary(s: string): string | undefined {
  const cleaned = capScalars(sanitizeTrim(s), MAX_SUMMARY_SCALARS);
  return cleaned.length === 0 ? undefined : cleaned;
}

function sanitizeDetails(details: WireDetail[]): WireDetail[] | undefined {
  const cleaned = details
    .map((d) => ({
      label: capScalars(sanitizeTrim(d.label), MAX_NAME_OR_LABEL_SCALARS),
      value: capScalars(sanitizeTrim(d.value), MAX_VALUE_SCALARS),
    }))
    .filter((d) => d.label.length > 0)
    .slice(0, MAX_DETAILS);
  return cleaned.length === 0 ? undefined : cleaned;
}

/** Only a path's final segment, never the full path — used for
 * tool-argument file paths so a project layout / home directory never
 * leaks (spec §3.2: "a safe tool name, a basename, and a short human
 * summary", never raw tool input). */
function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter((p) => p.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function isNonEmptyString(v: unknown): v is string {
  return typeof v === "string" && v.trim().length > 0;
}

// ---------------------------------------------------------------------
// Pure mapping: OpenCode bus event -> AgentWireEvent | null
// ---------------------------------------------------------------------

export interface EventContext {
  /** Unique per event — the binding layer uses `crypto.randomUUID()`;
   * tests inject a deterministic id. */
  eventId: string;
  occurredAtMs: number;
  sequence?: number;
}

export interface BusEvent {
  /** OpenCode's documented `event.type` discriminated union. The
   * `string & Record<never, never>` widening keeps this an open union
   * (forward-compatible with event types this adapter doesn't know
   * about yet) while still giving autocomplete on the known values. */
  type: BusEventType | (string & Record<never, never>);
  properties?: Record<string, unknown>;
}

export type BusEventType =
  | "permission.asked"
  | "permission.replied"
  | "session.created"
  | "session.updated"
  | "session.status"
  | "session.idle"
  | "session.error"
  | "session.deleted";

function baseEvent(
  nativeEvent: string,
  sessionId: string,
  kind: AgentEventKind,
  state: AgentSessionState,
  terminal: boolean,
  ctx: EventContext,
): AgentWireEvent {
  return {
    schemaVersion: SCHEMA_VERSION,
    eventId: sanitizeId(ctx.eventId),
    runtime: RUNTIME,
    sessionId: sanitizeId(sessionId),
    occurredAtMs: ctx.occurredAtMs,
    ...(ctx.sequence !== undefined ? { sequence: ctx.sequence } : {}),
    nativeEvent,
    kind,
    state,
    terminal,
    capabilities: [...OPENCODE_CAPABILITIES],
  };
}

/** Reads a session id out of an undocumented `event.properties` shape.
 * Providers vary in whether the id sits at the top level or nested
 * under `info`/`session` — this checks the plausible spots and returns
 * `undefined` (never a guess) if none hold a non-empty string. */
function extractSessionId(properties: Record<string, unknown> | undefined): string | undefined {
  if (!properties) return undefined;
  const direct = properties.sessionID ?? properties.sessionId;
  if (isNonEmptyString(direct)) return direct;
  const info = properties.info as Record<string, unknown> | undefined;
  if (info && isNonEmptyString(info.id)) return info.id;
  const session = properties.session as Record<string, unknown> | undefined;
  if (session && isNonEmptyString(session.id)) return session.id;
  return undefined;
}

function extractProjectName(properties: Record<string, unknown> | undefined): string | undefined {
  if (!properties) return undefined;
  const info = properties.info as Record<string, unknown> | undefined;
  const title = properties.title ?? info?.title;
  return isNonEmptyString(title) ? sanitizeNameOrLabel(title) : undefined;
}

/** `cwd` is genuinely a local filesystem path, unlike `name` — spec
 * §3.1 keeps them as separate `WireProject` fields with separate caps
 * (120 scalars for `name`, 1,024 for `cwd`), so this uses
 * `sanitizeValue` rather than `sanitizeNameOrLabel`. */
function extractProjectCwd(properties: Record<string, unknown> | undefined): string | undefined {
  if (!properties) return undefined;
  const info = properties.info as Record<string, unknown> | undefined;
  const cwd = properties.directory ?? properties.worktree ?? info?.directory;
  return isNonEmptyString(cwd) ? sanitizeValue(cwd) : undefined;
}

function extractProject(properties: Record<string, unknown> | undefined): WireProject | undefined {
  const name = extractProjectName(properties);
  const cwd = extractProjectCwd(properties);
  return name || cwd ? { ...(name ? { name } : {}), ...(cwd ? { cwd } : {}) } : undefined;
}

function mapPermissionAsked(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(
    event.type,
    sessionId,
    "permission_requested",
    "waiting_for_permission",
    false,
    ctx,
  );
  wire.summary = sanitizeSummary("Permission requested");
  const permType = event.properties?.type;
  if (isNonEmptyString(permType)) {
    wire.details = sanitizeDetails([{ label: "Permission", value: permType }]);
  }
  return wire;
}

function mapPermissionReplied(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(event.type, sessionId, "informational", "working", false, ctx);
  wire.summary = sanitizeSummary("Permission response received");
  return wire;
}

function mapSessionCreated(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(event.type, sessionId, "informational", "starting", false, ctx);
  wire.summary = sanitizeSummary("Session started");
  const project = extractProject(event.properties);
  if (project) wire.project = project;
  return wire;
}

function mapSessionUpdated(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  // Undocumented payload — never infer a terminal/waiting state from it
  // (spec: "wording is never parsed to infer state"). session.idle and
  // session.error are the dedicated terminal signals.
  const wire = baseEvent(event.type, sessionId, "informational", "working", false, ctx);
  wire.summary = sanitizeSummary("Session updated");
  const project = extractProject(event.properties);
  if (project) wire.project = project;
  return wire;
}

/** `session.status`'s status vocabulary is undocumented (see file
 * header). Only the one explicit, unambiguous token is acted on; any
 * other value is dropped rather than guessed at. */
function mapSessionStatus(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const status = event.properties?.status;
  if (!isNonEmptyString(status)) return null;
  const normalized = status.trim().toLowerCase();
  if (normalized !== "waiting_for_input" && normalized !== "input_required") return null;
  const wire = baseEvent(event.type, sessionId, "input_required", "waiting_for_input", false, ctx);
  wire.summary = sanitizeSummary("Waiting for input");
  return wire;
}

/** Operator decision 2026-07-26 (spec §2.1): `session.idle` fires once
 * per turn (the agent finished and is awaiting the user), not once per
 * session — non-terminal, so the registry resolves this into
 * `WaitingForInput` rather than a terminal state. Only `session.deleted`
 * (`mapSessionDeleted`) is the explicit session-end signal. */
function mapSessionIdle(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(event.type, sessionId, "completed", "completed", false, ctx);
  wire.summary = sanitizeSummary("Session completed");
  return wire;
}

function mapSessionError(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(event.type, sessionId, "failed", "failed", true, ctx);
  // Deliberately a fixed, generic summary — an undocumented `error`
  // payload could be a stack trace or otherwise contain sensitive
  // command/prompt content, which spec §3.2 forbids forwarding. Only a
  // short, safe error *name*/*code* (never a message) is allowed
  // through as a detail.
  wire.summary = sanitizeSummary("Session failed");
  const error = event.properties?.error as Record<string, unknown> | undefined;
  const name = error?.name;
  if (isNonEmptyString(name) && name.length <= MAX_NAME_OR_LABEL_SCALARS) {
    wire.details = sanitizeDetails([{ label: "Error", value: name }]);
  }
  return wire;
}

/** `session.deleted` is OpenCode's explicit session-end signal, so it is
 * the counterpart of the other three runtimes' `SessionEnd` hook: a
 * TERMINAL `completed`. It used to emit `informational` + terminal,
 * which meant OpenCode's real session end produced no card at all (it
 * fell into the off-by-default `informational_notifications` gate) while
 * the other three runtimes carded — an inconsistency the core's
 * `Completed`-terminal split (2026-08-02) made visible. Emitting
 * `completed` here is what makes a real session end card identically
 * across all four runtimes; the registry's `next_state` lands both
 * `Completed`+terminal and `Informational`+terminal in the same terminal
 * `Completed` state, so the Agent Board is unaffected by the change. */
function mapSessionDeleted(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(event.type, sessionId, "completed", "completed", true, ctx);
  wire.summary = sanitizeSummary("Session ended");
  return wire;
}

const BUS_EVENT_MAPPERS: Record<
  BusEventType,
  (event: BusEvent, ctx: EventContext) => AgentWireEvent | null
> = {
  "permission.asked": mapPermissionAsked,
  "permission.replied": mapPermissionReplied,
  "session.created": mapSessionCreated,
  "session.updated": mapSessionUpdated,
  "session.status": mapSessionStatus,
  "session.idle": mapSessionIdle,
  "session.error": mapSessionError,
  "session.deleted": mapSessionDeleted,
};

/** The single pure entry point for the `event` hook. Returns `null` for
 * any event type this adapter doesn't recognize (including future
 * OpenCode event types) or one whose session id can't be established —
 * the binding layer simply skips delivery in that case. */
export function mapBusEvent(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const mapper = BUS_EVENT_MAPPERS[event.type as BusEventType];
  if (!mapper) return null;
  return mapper(event, ctx);
}

// ---------------------------------------------------------------------
// Pure mapping: tool.execute.before / tool.execute.after
// ---------------------------------------------------------------------

export interface ToolExecuteInput {
  tool?: unknown;
  sessionID?: unknown;
  sessionId?: unknown;
  callID?: unknown;
}

export interface ToolExecuteBeforeOutput {
  args?: Record<string, unknown>;
  title?: unknown;
}

export interface ToolExecuteAfterOutput {
  title?: unknown;
  output?: unknown;
  metadata?: unknown;
}

/** Never forwards `output.args` verbatim (a shell command, a full file
 * write payload, ...) — only the tool name plus, when an argument looks
 * like a file path, its basename. This is the "safe tool name, a
 * basename, and a short human summary" allowance from spec §3.2, not a
 * general args passthrough. */
function safeToolDetail(toolName: string, args: Record<string, unknown> | undefined): WireDetail[] {
  const details: WireDetail[] = [{ label: "Tool", value: toolName }];
  const filePath = args?.filePath ?? args?.path;
  if (isNonEmptyString(filePath)) {
    details.push({ label: "File", value: basename(filePath) });
  }
  return details;
}

export function mapToolExecuteBefore(
  input: ToolExecuteInput,
  output: ToolExecuteBeforeOutput,
  ctx: EventContext,
): AgentWireEvent | null {
  const sessionId = isNonEmptyString(input.sessionID)
    ? input.sessionID
    : isNonEmptyString(input.sessionId)
      ? input.sessionId
      : undefined;
  const toolName = input.tool;
  if (!sessionId || !isNonEmptyString(toolName)) return null;
  const wire = baseEvent("tool.execute.before", sessionId, "informational", "working", false, ctx);
  wire.summary = sanitizeSummary(`Running ${toolName}`);
  wire.details = sanitizeDetails(safeToolDetail(toolName, output?.args));
  return wire;
}

export function mapToolExecuteAfter(
  input: ToolExecuteInput,
  output: ToolExecuteAfterOutput,
  ctx: EventContext,
): AgentWireEvent | null {
  const sessionId = isNonEmptyString(input.sessionID)
    ? input.sessionID
    : isNonEmptyString(input.sessionId)
      ? input.sessionId
      : undefined;
  const toolName = input.tool;
  if (!sessionId || !isNonEmptyString(toolName)) return null;
  // No documented success/failure flag on `output` at this hook — see
  // file header "Known gaps". Always informational/working, never
  // `failed`, so this adapter never manufactures a failure signal it
  // can't actually observe. `output.title` (when present and a plain
  // string) is the one field of `output` this adapter forwards — a
  // short display title, never `output.output` (raw tool output) or
  // `output.metadata` (may carry provider-internal, potentially
  // sensitive data).
  const wire = baseEvent("tool.execute.after", sessionId, "informational", "working", false, ctx);
  wire.summary = sanitizeSummary(`Finished ${toolName}`);
  const details: WireDetail[] = [{ label: "Tool", value: toolName }];
  if (isNonEmptyString(output?.title)) {
    details.push({ label: "Result", value: output.title });
  }
  wire.details = sanitizeDetails(details);
  return wire;
}

// ---------------------------------------------------------------------
// Delivery (mirrors the Rust `notchtap-agent hook` helper, spec §4.1)
// ---------------------------------------------------------------------

export interface DeliverOptions {
  /** Defaults to `NOTCHTAP_PORT` env var, falling back to
   * `DEFAULT_PORT` — same override contract as the Rust helper. */
  port?: number;
  timeoutMs?: number;
  fetchImpl?: typeof fetch;
  /** Bounded diagnostic sink — the Rust helper writes to notchtap's
   * adapter log, never stdout; this defaults to a no-op so a plugin
   * host with no logging story stays silent by default. Never throws,
   * never rejects the caller. */
  onDiagnostic?: (message: string) => void;
}

/** Reads `NOTCHTAP_PORT` if present and a valid positive integer,
 * otherwise `DEFAULT_PORT` — mirrors adapter.rs/http.rs's own port
 * resolution. Guards `process` being undefined (non-Node hosts). */
export function resolvePort(
  env: Record<string, string | undefined> | undefined = typeof process !== "undefined"
    ? process.env
    : undefined,
): number {
  const raw = env?.NOTCHTAP_PORT;
  if (!raw) return DEFAULT_PORT;
  const parsed = Number.parseInt(raw, 10);
  return Number.isInteger(parsed) && parsed > 0 && parsed <= 65535 ? parsed : DEFAULT_PORT;
}

/** POSTs one normalized event to the loopback endpoint. Fail-open by
 * construction: every failure path (network error, timeout, non-2xx
 * response, thrown exception) is caught here and reported only through
 * `onDiagnostic` — this function's returned promise NEVER rejects, so a
 * caller can safely fire-and-forget it from inside an OpenCode hook
 * without risking the session. Bounded by `timeoutMs` (default 750ms,
 * spec §4.1) via `AbortController`, never leaving a hanging request. */
export async function deliverAgentEvent(
  event: AgentWireEvent,
  opts: DeliverOptions = {},
): Promise<void> {
  const port = opts.port ?? resolvePort();
  const timeoutMs = opts.timeoutMs ?? DELIVERY_TIMEOUT_MS;
  const fetchImpl = opts.fetchImpl ?? fetch;
  const diagnostic = opts.onDiagnostic ?? (() => {});

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetchImpl(`http://127.0.0.1:${port}/agent/events`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(event),
      signal: controller.signal,
    });
    if (!response.ok) {
      diagnostic(`notchtap-opencode: delivery rejected with status ${response.status}`);
    }
  } catch (err) {
    diagnostic(`notchtap-opencode: delivery failed: ${String(err)}`);
  } finally {
    clearTimeout(timer);
  }
}

// ---------------------------------------------------------------------
// Binding layer — the only OpenCode-shaped part of this file
// ---------------------------------------------------------------------

function freshContext(sequence: { current: number }): EventContext {
  sequence.current += 1;
  return {
    eventId: crypto.randomUUID(),
    occurredAtMs: Date.now(),
    sequence: sequence.current,
  };
}

/** The plugin export. Structurally matches OpenCode's documented
 * `Plugin` shape (`async (ctx) => Hooks`) without importing
 * `@opencode-ai/plugin` — this repo doesn't depend on OpenCode, and the
 * shape is small enough to satisfy structurally. A real install may add
 * `import type { Plugin } from "@opencode-ai/plugin"` and annotate this
 * export with it for editor support; that's optional, not required for
 * OpenCode to load the plugin. */
export const NotchtapPlugin = async () => {
  const sequence = { current: 0 };

  return {
    event: async ({ event }: { event: BusEvent }) => {
      const wire = mapBusEvent(event, freshContext(sequence));
      if (wire) void deliverAgentEvent(wire);
    },
    "tool.execute.before": async (input: ToolExecuteInput, output: ToolExecuteBeforeOutput) => {
      const wire = mapToolExecuteBefore(input, output, freshContext(sequence));
      if (wire) void deliverAgentEvent(wire);
    },
    "tool.execute.after": async (input: ToolExecuteInput, output: ToolExecuteAfterOutput) => {
      const wire = mapToolExecuteAfter(input, output, freshContext(sequence));
      if (wire) void deliverAgentEvent(wire);
    },
  };
};

export default NotchtapPlugin;
