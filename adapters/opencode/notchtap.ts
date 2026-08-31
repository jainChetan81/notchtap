// Standalone OpenCode adapter; see https://opencode.ai/docs/plugins/.
// Install as `.opencode/plugins/notchtap.ts` or `~/.config/opencode/plugins/notchtap.ts`.
// It maps lifecycle and tool hooks to schema-v1 `POST /agent/events` requests.
// Undocumented payloads are never guessed: missing session IDs and unknown statuses are dropped.
// Tool results expose no failure flag, so failures are not inferred from output text.
// OpenCode exposes neither subagent lifecycle nor host identity, so those capabilities stay absent.

type UnparsedValue = string | number | boolean | null | UnparsedObject | UnparsedValue[];
type UnparsedObject = { [key: string]: UnparsedValue };

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

/** Keep frozen and limited to signals the OpenCode API exposes. */
export const OPENCODE_CAPABILITIES: readonly AgentCapability[] = Object.freeze([
  "session_lifecycle",
  "permission_requests",
  "input_required",
  "completion",
  "failure",
  "tool_details",
]);

const MAX_ID_BYTES = 256;
const MAX_SUMMARY_SCALARS = 500;
const MAX_NAME_OR_LABEL_SCALARS = 120;
const MAX_VALUE_SCALARS = 1024;
const MAX_DETAILS = 12;

/** Matches Rust `char::is_control`: C0, DEL, and C1 controls only. */
// biome-ignore lint/suspicious/noControlCharactersInRegex: intentionally matching control characters to strip them
const CONTROL_CHARS = /[\u0000-\u001f\u007f-\u009f]/gu; // oxlint-disable-line eslint/no-control-regex

/** Trim before stripping controls to preserve Rust sanitization parity. */
function sanitizeTrim(s: string): string {
  return s.trim().replace(CONTROL_CHARS, "");
}

/** `Array.from` caps Unicode scalars without splitting surrogate pairs. */
function capScalars(s: string, max: number): string {
  const chars = Array.from(s);
  return chars.length <= max ? s : chars.slice(0, max).join("");
}

/** Cap opaque identifiers by UTF-8 bytes without splitting codepoints. */
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

/** Return only the final segment so local filesystem paths never leak. */
function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter((p) => p.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function isNonEmptyString(v: unknown): v is string {
  return typeof v === "string" && v.trim().length > 0;
}

export interface EventContext {
  eventId: string;
  occurredAtMs: number;
  sequence?: number;
}

export interface BusEvent {
  /** Keep the documented event union open for forward compatibility. */
  type: BusEventType | (string & Record<never, never>);
  properties?: UnparsedObject;
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
  const event: AgentWireEvent = {
    schemaVersion: SCHEMA_VERSION,
    eventId: sanitizeId(ctx.eventId),
    runtime: RUNTIME,
    sessionId: sanitizeId(sessionId),
    occurredAtMs: ctx.occurredAtMs,
    nativeEvent,
    kind,
    state,
    terminal,
    capabilities: [...OPENCODE_CAPABILITIES],
  };
  if (ctx.sequence !== undefined) {
    event.sequence = ctx.sequence;
  }
  return event;
}

/** Accept known ID locations in the undocumented payload; never invent one. */
function extractSessionId(properties: UnparsedObject | undefined): string | undefined {
  if (!properties) return undefined;
  const direct = properties.sessionID ?? properties.sessionId;
  if (isNonEmptyString(direct)) return direct;
  // SAFETY: validated as record via preceding checks.
  const info = properties.info as UnparsedObject | undefined;
  if (info && isNonEmptyString(info.id)) return info.id;
  // SAFETY: validated as record via preceding checks.
  const session = properties.session as UnparsedObject | undefined;
  if (session && isNonEmptyString(session.id)) return session.id;
  return undefined;
}

function extractProjectName(properties: UnparsedObject | undefined): string | undefined {
  if (!properties) return undefined;
  // SAFETY: validated as record via preceding checks.
  const info = properties.info as UnparsedObject | undefined;
  const title = properties.title ?? info?.title;
  return isNonEmptyString(title) ? sanitizeNameOrLabel(title) : undefined;
}

/** `cwd` uses the path-value cap, not the shorter display-name cap. */
function extractProjectCwd(properties: UnparsedObject | undefined): string | undefined {
  if (!properties) return undefined;
  // SAFETY: validated as record via preceding checks.
  const info = properties.info as UnparsedObject | undefined;
  const cwd = properties.directory ?? properties.worktree ?? info?.directory;
  return isNonEmptyString(cwd) ? sanitizeValue(cwd) : undefined;
}

function extractProject(properties: UnparsedObject | undefined): WireProject | undefined {
  const name = extractProjectName(properties);
  const cwd = extractProjectCwd(properties);
  if (name) {
    if (cwd) return { name, cwd };
    return { name };
  }
  return cwd ? { cwd } : undefined;
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
  // Never infer state from the undocumented update payload.
  const wire = baseEvent(event.type, sessionId, "informational", "working", false, ctx);
  wire.summary = sanitizeSummary("Session updated");
  const project = extractProject(event.properties);
  if (project) wire.project = project;
  return wire;
}

/** Accept only explicit input-required tokens; drop undocumented statuses. */
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

/** `session.idle` is per-turn; only `session.deleted` ends the session. */
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
  // Never expose the undocumented error message; only a bounded name is safe.
  wire.summary = sanitizeSummary("Session failed");
  // SAFETY: validated as record via preceding checks.
  const error = event.properties?.error as UnparsedObject | undefined;
  const name = error?.name;
  if (isNonEmptyString(name) && name.length <= MAX_NAME_OR_LABEL_SCALARS) {
    wire.details = sanitizeDetails([{ label: "Error", value: name }]);
  }
  return wire;
}

function mapSessionDeleted(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  const sessionId = extractSessionId(event.properties);
  if (!sessionId) return null;
  const wire = baseEvent(event.type, sessionId, "completed", "completed", true, ctx);
  wire.summary = sanitizeSummary("Session ended");
  return wire;
}

const BUS_EVENT_MAPPERS = new Map<
  BusEventType,
  (event: BusEvent, ctx: EventContext) => AgentWireEvent | null
>([
  ["permission.asked", mapPermissionAsked],
  ["permission.replied", mapPermissionReplied],
  ["session.created", mapSessionCreated],
  ["session.updated", mapSessionUpdated],
  ["session.status", mapSessionStatus],
  ["session.idle", mapSessionIdle],
  ["session.error", mapSessionError],
  ["session.deleted", mapSessionDeleted],
]);

export function mapBusEvent(event: BusEvent, ctx: EventContext): AgentWireEvent | null {
  // SAFETY: BusEventType is a closed union of known OpenCode bus event types — the Map lookup is the runtime check, and a miss returns null for future unknown types.
  const mapper = BUS_EVENT_MAPPERS.get(event.type as BusEventType);
  if (!mapper) return null;
  return mapper(event, ctx);
}

export interface ToolExecuteInput {
  tool?: unknown;
  sessionID?: unknown;
  sessionId?: unknown;
  callID?: unknown;
}

export interface ToolExecuteBeforeOutput {
  args?: UnparsedObject;
  title?: unknown;
}

export interface ToolExecuteAfterOutput {
  title?: unknown;
  output?: unknown;
  metadata?: unknown;
}

/** Never forward raw tool arguments; retain only the tool name and path basename. */
function safeToolDetail(toolName: string, args: UnparsedObject | undefined): WireDetail[] {
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
  // No failure flag exists here; never infer one or forward raw output/metadata.
  const wire = baseEvent("tool.execute.after", sessionId, "informational", "working", false, ctx);
  wire.summary = sanitizeSummary(`Finished ${toolName}`);
  const details: WireDetail[] = [{ label: "Tool", value: toolName }];
  if (isNonEmptyString(output?.title)) {
    details.push({ label: "Result", value: output.title });
  }
  wire.details = sanitizeDetails(details);
  return wire;
}

export interface DeliverOptions {
  port?: number;
  timeoutMs?: number;
  fetchImpl?: typeof fetch;
  /** Optional bounded diagnostic sink; delivery never throws or rejects. */
  onDiagnostic?: (message: string) => void;
}

export function resolvePort(
  // SAFETY: globalThis may not have process in non-Node hosts — the cast only narrows the optional env access, the optional chain handles absence.
  env: Record<string, string | undefined> | undefined = (
    globalThis as { process?: { env?: Record<string, string | undefined> } }
  ).process?.env,
): number {
  const raw = env?.NOTCHTAP_PORT;
  if (!raw) return DEFAULT_PORT;
  const parsed = Number.parseInt(raw, 10);
  return Number.isInteger(parsed) && parsed > 0 && parsed <= 65535 ? parsed : DEFAULT_PORT;
}

/** Fail-open delivery: catch every failure and bound requests with `timeoutMs`. */
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

function freshContext(sequence: { current: number }): EventContext {
  sequence.current += 1;
  return {
    eventId: crypto.randomUUID(),
    occurredAtMs: Date.now(),
    sequence: sequence.current,
  };
}

/** Structurally matches OpenCode's plugin shape without adding its package as a dependency. */
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

export const server = NotchtapPlugin;

export default {
  id: "notchtap",
  server,
};
