import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { emitTo, listen, resetHandlers } from "./test-support/tauriEventMock";
import type { AgentState } from "./useAgentState";
import { useAgentState } from "./useAgentState";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

// deliberately keeps `unknown` — this file exercises malformed payloads
const emit = (payload: unknown) => act(() => emitTo("agent-state", payload));

function session(overrides: Partial<AgentState["sessions"][number]> = {}) {
  return {
    id: "hash1",
    runtime: "codex" as const,
    state: "waiting_for_permission" as const,
    capabilities: ["permission_requests" as const],
    summary: "Approval needed to run a command",
    details: [],
    project: { name: "notchtap", cwd: "/repo" },
    host: null,
    elapsedMs: 1200,
    retentionRemainingMs: null,
    history: [{ state: "starting" as const, elapsedMs: 5000 }],
    ...overrides,
  };
}

describe("useAgentState", () => {
  beforeEach(() => {
    resetHandlers();
    listen.mockClear();
  });

  async function renderReady() {
    const rendered = renderHook(() => useAgentState());
    await act(async () => {
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalled();
    return rendered;
  }

  it("starts empty before any event arrives", async () => {
    const { result } = await renderReady();
    expect(result.current.sessions).toEqual([]);
    expect(result.current.adapterHealth).toEqual([]);
    expect(result.current.revision).toBe(0);
  });

  it("renders a valid payload as-is when an event arrives", async () => {
    const { result } = await renderReady();
    const payload: AgentState = {
      revision: 1,
      capturedAtMs: 1_000,
      sessions: [session()],
      adapterHealth: [],
    };
    emit(payload);
    expect(result.current).toEqual(payload);
  });

  it("a new payload replaces the previous one directly", async () => {
    const { result } = await renderReady();
    emit({ revision: 1, capturedAtMs: 1_000, sessions: [session()], adapterHealth: [] });
    expect(result.current.sessions).toHaveLength(1);
    emit({ revision: 2, capturedAtMs: 2_000, sessions: [], adapterHealth: [] });
    expect(result.current.sessions).toHaveLength(0);
    expect(result.current.revision).toBe(2);
  });

  it("drops a session with an unrecognized runtime, keeping every valid sibling", async () => {
    const { result } = await renderReady();
    emit({
      revision: 1,
      capturedAtMs: 1_000,
      sessions: [session({ id: "bad", runtime: "cursor" as never }), session({ id: "good" })],
      adapterHealth: [],
    });
    expect(result.current.sessions.map((s) => s.id)).toEqual(["good"]);
  });

  it("drops a session with an unrecognized state", async () => {
    const { result } = await renderReady();
    emit({
      revision: 1,
      capturedAtMs: 1_000,
      sessions: [session({ id: "bad", state: "made_up" as never })],
      adapterHealth: [],
    });
    expect(result.current.sessions).toEqual([]);
  });

  it("ignores a top-level malformed payload, keeping the last good state", async () => {
    const { result } = await renderReady();
    const good: AgentState = {
      revision: 1,
      capturedAtMs: 1_000,
      sessions: [session()],
      adapterHealth: [],
    };
    emit(good);
    emit({ revision: "not-a-number" });
    expect(result.current).toEqual(good);
  });

  it("accepts a session that omits project/host entirely", async () => {
    const { result } = await renderReady();
    const { project, host, ...withoutOptional } = session();
    void project;
    void host;
    emit({ revision: 1, capturedAtMs: 1_000, sessions: [withoutOptional], adapterHealth: [] });
    expect(result.current.sessions).toHaveLength(1);
  });

  // Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): the wire view's
  // new `history` field — optional at validation time (an older cached
  // payload without it must not drop the session), sanitized down to an
  // empty array rather than left `undefined`.
  it("defaults a session that omits history entirely to an empty array", async () => {
    const { result } = await renderReady();
    const { history, ...withoutHistory } = session();
    void history;
    emit({ revision: 1, capturedAtMs: 1_000, sessions: [withoutHistory], adapterHealth: [] });
    expect(result.current.sessions).toHaveLength(1);
    expect(result.current.sessions[0].history).toEqual([]);
  });

  it("drops a session whose history entry carries an unrecognized state", async () => {
    const { result } = await renderReady();
    emit({
      revision: 1,
      capturedAtMs: 1_000,
      sessions: [
        session({
          id: "bad",
          history: [{ state: "made_up" as never, elapsedMs: 1 }],
        }),
      ],
      adapterHealth: [],
    });
    expect(result.current.sessions).toEqual([]);
  });
});
