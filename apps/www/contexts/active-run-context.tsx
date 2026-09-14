"use client";

import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { RunDetail } from "@/lib/api-types";
import {
  applyAssistantDelta,
  emptyAssistantStream,
  type AssistantStreamState,
} from "@/lib/assistant-stream";
import { activityText } from "@/lib/work-events";
import type { ApprovalRequestedPayload, ApprovalTerminalState } from "@/components/app/approval-card";
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type RunActivityItem =
  | { id: string; kind: "text"; text: string }
  | { id: string; kind: "approval"; approval: ApprovalRequestedPayload; decision?: ApprovalTerminalState };

export type ActiveRunState = {
  runId: string | null;
  detail: RunDetail | null;
  timeline: RunActivityItem[];
  assistantStream: AssistantStreamState;
  lastEventId: string | null;
  browserPreviewGeneration: number;
  connection: string | null;
  error: string | null;
};

const ActiveRunContext = createContext<ActiveRunState | null>(null);

function runIsActive(status: string): boolean {
  return status === "queued" || status === "running";
}

function isBrowserToolEvent(eventType: string, payload: Record<string, unknown>): boolean {
  if (eventType === "tool_result" || eventType === "tool_call") {
    const tool = String(payload.tool ?? payload.name ?? "").toLowerCase();
    return tool.includes("browser");
  }
  const text = activityText(eventType, payload);
  if (!text) {
    return false;
  }
  return text.toLowerCase().includes("browser");
}

export function ActiveRunProvider({
  runId,
  children,
}: {
  runId: string | null;
  children: ReactNode;
}) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [timeline, setTimeline] = useState<RunActivityItem[]>([]);
  const [assistantStream, setAssistantStream] = useState<AssistantStreamState>(emptyAssistantStream);
  const [lastEventId, setLastEventId] = useState<string | null>(null);
  const [browserPreviewGeneration, setBrowserPreviewGeneration] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState<string | null>(null);

  useEffect(() => {
    if (!runId) {
      setDetail(null);
      setTimeline([]);
      setAssistantStream(emptyAssistantStream());
      setLastEventId(null);
      setBrowserPreviewGeneration(0);
      setError(null);
      setConnection(null);
      return;
    }

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
        const response = await cloudHostFetch(`/v1/runs/${runId}`, {
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
        setConnection(
          runIsActive(current.status) ? "Following progress" : "Saved work history",
        );
        setError(null);

        await cloudHostEventStream(`/v1/runs/${runId}/events`, {
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
            const id = event.id ?? `terminal-${runId}`;
            if (
              event.event === "approval_requested" &&
              typeof payload.approvalId === "string" &&
              typeof payload.summary === "string"
            ) {
              const approval: ApprovalRequestedPayload = {
                approvalId: payload.approvalId,
                summary: payload.summary,
                tool: String(payload.tool ?? ""),
                operationKind: String(payload.operationKind ?? ""),
              };
              setTimeline((previous) =>
                previous.some((item) => item.id === id)
                  ? previous
                  : [...previous, { id, kind: "approval", approval }],
              );
            } else if (event.event === "approval_resolved") {
              const decision = payload.decision;
              if (
                ["approved", "denied", "cancelled", "expired"].includes(String(decision))
              ) {
                setTimeline((previous) =>
                  previous.map((item) =>
                    item.kind === "approval" &&
                    item.approval.approvalId === payload.approvalId
                      ? { ...item, decision: decision as ApprovalTerminalState }
                      : item,
                  ),
                );
              }
            } else {
              const text = activityText(event.event, payload);
              if (text && isNew) {
                setTimeline((previous) =>
                  previous.some((item) => item.id === id)
                    ? previous
                    : [...previous.slice(-199), { id, kind: "text", text }],
                );
              }
              if (isBrowserToolEvent(event.event, payload)) {
                setBrowserPreviewGeneration((value) => value + 1);
              }
            }
          },
        });

        if (controller.signal.aborted) {
          return;
        }

        const refreshed = await cloudHostFetch(`/v1/runs/${runId}`, {
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
        setError(err instanceof Error ? err.message : "Could not follow progress");
        setConnection("Reconnecting…");
        timer = setTimeout(() => void sync(), 5000);
      }
    }

    setDetail(null);
    setTimeline([]);
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
      connection,
      error,
    }),
    [
      runId,
      detail,
      timeline,
      assistantStream,
      lastEventId,
      browserPreviewGeneration,
      connection,
      error,
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
      connection: null,
      error: null,
    };
  }
  return ctx;
}

export function useRequireActiveRun(): ActiveRunState {
  const ctx = useContext(ActiveRunContext);
  if (!ctx) {
    throw new Error("useRequireActiveRun must be used within ActiveRunProvider");
  }
  return ctx;
}
