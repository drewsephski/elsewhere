"use client";

import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { RunDetail } from "@/lib/api-types";
import {
  applyAssistantDelta,
  emptyAssistantStream,
  type AssistantStreamState,
} from "@/lib/assistant-stream";
import type { ApprovalRequestedPayload, ApprovalTerminalState } from "@/components/app/approval-card";
import type { UserQuestionPayload } from "@/components/app/user-question-card";
import type { ConnectorNeed } from "@/lib/connector-need";
import {
  applyRunStreamEvent,
  emptyRunTimelineState,
  type RunTimelineState,
} from "@/lib/run-event-timeline";
import type { SubagentActivity } from "@/lib/subagent-events";
import {
  fetchHumanInterventionStatus,
  type PendingHumanIntervention,
} from "@/lib/human-intervention";
import { formatUserFacingError } from "@/lib/format-api-error";
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type ToolActivityState = "running" | "complete" | "error";

export type ToolActivity = {
  toolName: string;
  label: string;
  state: ToolActivityState;
  technical?: string;
};

export type RunActivityItem =
  | { id: string; kind: "text"; text: string; technical?: string }
  | { id: string; kind: "tool"; tool: ToolActivity }
  | { id: string; kind: "approval"; approval: ApprovalRequestedPayload; decision?: ApprovalTerminalState }
  | { id: string; kind: "connector"; need: ConnectorNeed }
  | { id: string; kind: "subagent"; subagent: SubagentActivity }
  | { id: string; kind: "question"; question: UserQuestionPayload };

export type ActiveRunState = {
  runId: string | null;
  detail: RunDetail | null;
  timeline: RunActivityItem[];
  assistantStream: AssistantStreamState;
  lastEventId: string | null;
  browserPreviewGeneration: number;
  workspaceRefreshGeneration: number;
  lastBrowserToolError: string | null;
  connection: string | null;
  error: string | null;
  pendingHumanIntervention: PendingHumanIntervention | null;
};

const ActiveRunContext = createContext<ActiveRunState | null>(null);

function runIsActive(status: string): boolean {
  return status === "queued" || status === "running";
}

function toolNameFromPayload(payload: Record<string, unknown>): string {
  return String(payload.tool ?? payload.name ?? "").toLowerCase();
}

function normalizedToolName(payload: Record<string, unknown>): string {
  const raw = toolNameFromPayload(payload);
  const segment = raw.includes("/") ? raw.split("/").pop() ?? raw : raw;
  return segment.replace(/^elsewhere[_-]/, "");
}

function isBrowserToolResult(eventType: string, payload: Record<string, unknown>): boolean {
  if (eventType !== "tool_result") {
    return false;
  }
  if (payload.ok === false) {
    return false;
  }
  const tool = normalizedToolName(payload);
  return tool.includes("browser");
}

function isWorkspaceMutationToolResult(
  eventType: string,
  payload: Record<string, unknown>,
): boolean {
  if (eventType !== "tool_result" || payload.ok === false) {
    return false;
  }
  const tool = normalizedToolName(payload);
  return (
    tool === "workspace_write" ||
    tool === "workspace_exec" ||
    tool === "browser_screenshot" ||
    tool === "browser_download"
  );
}

export function ActiveRunProvider({
  runId,
  children,
}: {
  runId: string | null;
  children: ReactNode;
}) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [timelineState, setTimelineState] = useState<RunTimelineState>(emptyRunTimelineState);
  const timeline = timelineState.items;
  const pendingHumanIntervention = timelineState.pendingHumanIntervention;
  const [assistantStream, setAssistantStream] = useState<AssistantStreamState>(emptyAssistantStream);
  const [lastEventId, setLastEventId] = useState<string | null>(null);
  const [browserPreviewGeneration, setBrowserPreviewGeneration] = useState(0);
  const [workspaceRefreshGeneration, setWorkspaceRefreshGeneration] = useState(0);
  const [lastBrowserToolError, setLastBrowserToolError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState<string | null>(null);
  useEffect(() => {
    if (!runId) {
      setDetail(null);
      setTimelineState(emptyRunTimelineState());
      setAssistantStream(emptyAssistantStream());
      setLastEventId(null);
      setBrowserPreviewGeneration(0);
      setWorkspaceRefreshGeneration(0);
      setLastBrowserToolError(null);
      setError(null);
      setConnection(null);
      return;
    }

    const activeRunId = runId;
    const controller = new AbortController();
    let lastEventIdLocal: string | undefined;
    let timer: ReturnType<typeof setTimeout>;
    const seenEventIds = new Set<string>();
    const itemProgress = new Map<string, number>();

    function rememberEventId(id: string | undefined): boolean {
      if (!id) {
        return true;
      }
      if (seenEventIds.has(id)) {
        return false;
      }
      seenEventIds.add(id);
      return true;
    }

    async function sync() {
      try {
        const response = await cloudHostFetch(`/v1/runs/${activeRunId}`, {
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error("Could not load work");
        }
        const current: RunDetail = await response.json();
        if (controller.signal.aborted) {
          return;
        }
        setDetail(current);
        setConnection(runIsActive(current.status) ? "Working" : "Saved");
        setError(null);

        try {
          const intervention = await fetchHumanInterventionStatus(activeRunId);
          if (!controller.signal.aborted) {
            setTimelineState((previous) => ({
              ...previous,
              pendingHumanIntervention: intervention.pending,
            }));
          }
        } catch {
          /* non-fatal */
        }

        await cloudHostEventStream(`/v1/runs/${activeRunId}/events`, {
          signal: controller.signal,
          lastEventId: lastEventIdLocal,
          onEvent(event) {
            if (event.id) {
              lastEventIdLocal = event.id;
              setLastEventId(event.id);
            }
            const payload: Record<string, unknown> = JSON.parse(event.data);
            if (event.event === "stream_error") {
              throw new Error("Progress connection interrupted");
            }

            const isNew = rememberEventId(event.id);
            if (!isNew && event.event === "assistant_delta") {
              return;
            }

            if (event.event === "assistant_delta") {
              setAssistantStream((previous) =>
                applyAssistantDelta(previous, itemProgress, payload),
              );
              return;
            }

            if (event.event === "terminal") {
              setBrowserPreviewGeneration((value) => value + 1);
              setWorkspaceRefreshGeneration((value) => value + 1);
              const fullContent =
                typeof payload.fullContent === "string"
                  ? payload.fullContent
                  : typeof payload.fullContent === "number"
                    ? String(payload.fullContent)
                    : null;
              if (fullContent) {
                setAssistantStream({
                  answerText: fullContent,
                  commentaryText: null,
                  streaming: false,
                });
              } else {
                setAssistantStream((previous) => ({
                  ...previous,
                  streaming: false,
                }));
              }
            }

            if (typeof payload.status === "string") {
              setDetail((previous) =>
                previous ? { ...previous, status: payload.status as string } : previous,
              );
            }
            setTimelineState((previous) =>
              applyRunStreamEvent(previous, activeRunId, event, {
                skipIfSeenId: () => isNew,
              }),
            );

            if (event.event === "tool_result" && normalizedToolName(payload).includes("browser")) {
              if (payload.ok === false) {
                const message =
                  typeof payload.error === "string"
                    ? payload.error
                    : typeof payload.output === "string"
                      ? payload.output
                      : "Browser operation failed";
                setLastBrowserToolError(message);
                setBrowserPreviewGeneration((value) => value + 1);
              } else {
                setLastBrowserToolError(null);
                setBrowserPreviewGeneration((value) => value + 1);
              }
            }
            if (isWorkspaceMutationToolResult(event.event, payload)) {
              setWorkspaceRefreshGeneration((value) => value + 1);
            }
          },
        });

        if (controller.signal.aborted) {
          return;
        }

        const refreshed = await cloudHostFetch(`/v1/runs/${activeRunId}`, {
          signal: controller.signal,
        });
        if (!refreshed.ok) {
          throw new Error("Could not refresh work status");
        }
        const result: RunDetail = await refreshed.json();
        setDetail(result);
        if (!runIsActive(result.status)) {
          setAssistantStream((previous) => ({ ...previous, streaming: false }));
        }
        if (runIsActive(result.status)) {
          timer = setTimeout(() => void sync(), 1500);
        } else {
          setConnection("Saved work history");
        }
      } catch (err) {
        if (controller.signal.aborted) {
          return;
        }
        setError(formatUserFacingError(err, "Could not follow progress"));
        setConnection("Reconnecting…");
        timer = setTimeout(() => void sync(), 5000);
      }
    }

    setDetail(null);
    setTimelineState(emptyRunTimelineState());
    setAssistantStream(emptyAssistantStream());
    void sync();

    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [runId]);

  const value = useMemo(
    () => ({
      runId,
      detail,
      timeline,
      assistantStream,
      lastEventId,
      browserPreviewGeneration,
      workspaceRefreshGeneration,
      lastBrowserToolError,
      connection,
      error,
      pendingHumanIntervention,
    }),
    [
      runId,
      detail,
      timeline,
      assistantStream,
      lastEventId,
      browserPreviewGeneration,
      workspaceRefreshGeneration,
      lastBrowserToolError,
      connection,
      error,
      pendingHumanIntervention,
    ],
  );

  return <ActiveRunContext.Provider value={value}>{children}</ActiveRunContext.Provider>;
}

export function useActiveRun(): ActiveRunState {
  const ctx = useContext(ActiveRunContext);
  if (!ctx) {
    return {
      runId: null,
      detail: null,
      timeline: [],
      assistantStream: emptyAssistantStream(),
      lastEventId: null,
      browserPreviewGeneration: 0,
      workspaceRefreshGeneration: 0,
      lastBrowserToolError: null,
      connection: null,
      error: null,
      pendingHumanIntervention: null,
    };
  }
  return ctx;
}

export function useOptionalActiveRun(): ActiveRunState | null {
  return useContext(ActiveRunContext);
}

export function useRequireActiveRun(): ActiveRunState {
  const ctx = useContext(ActiveRunContext);
  if (!ctx) {
    throw new Error("useRequireActiveRun must be used within ActiveRunProvider");
  }
  return ctx;
}
