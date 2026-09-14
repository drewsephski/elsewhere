"use client";

import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { RunDetail } from "@/lib/api-types";
import { activityText } from "@/lib/work-events";
import type { ApprovalRequestedPayload, ApprovalTerminalState } from "@/components/app/approval-card";
import { useEffect, useState } from "react";

export type RunActivityItem =
  | { id: string; kind: "text"; text: string }
  | { id: string; kind: "approval"; approval: ApprovalRequestedPayload; decision?: ApprovalTerminalState };

function runIsActive(status: string): boolean {
  return status === "queued" || status === "running";
}

export function useRunEventStream(runId: string | null) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [timeline, setTimeline] = useState<RunActivityItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState<string | null>(null);

  useEffect(() => {
    if (!runId) {
      setDetail(null);
      setTimeline([]);
      setError(null);
      setConnection(null);
      return;
    }

    const controller = new AbortController();
    let lastEventId: string | undefined;
    let timer: ReturnType<typeof setTimeout>;

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
          runIsActive(current.status)
            ? "Following progress"
            : "Saved work history",
        );
        setError(null);

        await cloudHostEventStream(`/v1/runs/${runId}/events`, {
          signal: controller.signal,
          lastEventId,
          onEvent(event) {
            if (event.id) {
              lastEventId = event.id;
            }
            const payload: Record<string, unknown> = JSON.parse(event.data);
            if (event.event === "stream_error") {
              throw new Error("Progress connection interrupted");
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
              if (text) {
                setTimeline((previous) =>
                  previous.some((item) => item.id === id)
                    ? previous
                    : [...previous.slice(-199), { id, kind: "text", text }],
                );
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
    void sync();

    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [runId]);

  return { detail, timeline, error, connection };
}
