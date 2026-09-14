"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { RunDetail } from "@/lib/api-types";
import { ResultsPanel } from "./results-panel";
import { activityText, workStatus } from "@/lib/work-events";
import {
  ApprovalCard,
  type ApprovalRequestedPayload,
  type ApprovalTerminalState,
} from "./approval-card";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  Frame,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import {
  Timeline,
  TimelineContent,
  TimelineHeader,
  TimelineIndicator,
  TimelineItem,
  TimelineSeparator,
  TimelineTitle,
} from "@/components/reui/timeline";
import { Button } from "@/components/ui/button";
import { ListTree } from "lucide-react";

type Activity = {
  id: string;
  text?: string;
  approval?: ApprovalRequestedPayload;
  decision?: ApprovalTerminalState;
};

const active = (status: string) => status === "queued" || status === "running";

function statusBadgeVariant(status: string | undefined) {
  if (!status) return "secondary" as const;
  if (status === "completed") return "success-light" as const;
  if (status === "failed" || status === "cancelled") return "destructive-light" as const;
  if (status === "interrupted") return "warning-light" as const;
  return "primary-light" as const;
}

function timelineTitle(item: Activity) {
  if (item.approval) {
    return item.decision
      ? `Approval ${item.decision}`
      : "Approval required";
  }
  const text = item.text ?? "";
  if (text.length <= 72) return text || "Update";
  return `${text.slice(0, 72)}…`;
}

export function WorkDetail({ runId }: { runId: string }) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [timeline, setTimeline] = useState<Activity[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState("Connecting to your work…");
  const [stopping, setStopping] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    let lastEventId: string | undefined;
    let timer: ReturnType<typeof setTimeout>;
    setDetail(null);
    setTimeline([]);
    setError(null);
    async function sync() {
      try {
        const response = await cloudHostFetch(`/v1/runs/${runId}`, {
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error(
            response.status === 404 ? "Work not found" : "Could not load this work",
          );
        }
        const current: RunDetail = await response.json();
        if (controller.signal.aborted) return;
        setDetail(current);
        setConnection(
          active(current.status)
            ? "Following progress. You can leave this page."
            : "Saved work history",
        );
        setError(null);
        await cloudHostEventStream(`/v1/runs/${runId}/events`, {
          signal: controller.signal,
          lastEventId,
          onEvent(event) {
            if (event.id) lastEventId = event.id;
            const payload: Record<string, unknown> = JSON.parse(event.data);
            if (event.event === "stream_error") throw new Error("Progress connection interrupted");
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
                previous.some((item) => item.id === id) ? previous : [...previous, { id, approval }],
              );
            } else if (event.event === "approval_resolved") {
              const decision = payload.decision;
              if (["approved", "denied", "cancelled", "expired"].includes(String(decision))) {
                setTimeline((previous) =>
                  previous.map((item) =>
                    item.approval?.approvalId === payload.approvalId
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
                    : [...previous.slice(-199), { id, text }],
                );
              }
            }
          },
        });
        if (controller.signal.aborted) return;
        const refreshed = await cloudHostFetch(`/v1/runs/${runId}`, {
          signal: controller.signal,
        });
        if (!refreshed.ok) throw new Error("Could not refresh work status");
        const result: RunDetail = await refreshed.json();
        setDetail(result);
        if (active(result.status)) timer = setTimeout(() => void sync(), 1500);
        else setConnection("Saved work history");
      } catch (err) {
        if (controller.signal.aborted) return;
        setError(err instanceof Error ? err.message : "Could not follow progress");
        setConnection("Reconnecting. Your work continues on the server.");
        timer = setTimeout(() => void sync(), 5000);
      }
    }
    void sync();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [runId]);

  const timelineStep = useMemo(
    () => (timeline.length > 0 ? timeline.length : 1),
    [timeline.length],
  );

  async function stop() {
    setStopping(true);
    try {
      const response = await cloudHostFetch(`/v1/runs/${runId}/cancel`, { method: "POST" });
      if (!response.ok) throw new Error("Could not stop work. Try again.");
      setDetail(await response.json());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not stop work");
    } finally {
      setStopping(false);
    }
  }

  return (
    <section className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0 max-w-3xl">
          <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
            Assignment
          </p>
          <h1 className="mt-2 whitespace-pre-wrap break-words text-xl font-semibold">
            {detail?.task ?? "Delegated work"}
          </h1>
          <p className="mt-3 text-sm text-muted-foreground" role="status">{connection}</p>
        </div>
        <div className="flex items-center gap-3">
          <Badge variant={statusBadgeVariant(detail?.status)}>
            {detail ? workStatus(detail.status) : "Loading…"}
          </Badge>
          {detail && active(detail.status) ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={stopping}
              onClick={() => void stop()}
            >
              {stopping ? "Stopping…" : "Stop work"}
            </Button>
          ) : null}
        </div>
      </div>

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Something went wrong</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {detail?.status === "interrupted" ? (
        <Alert variant="warning">
          <AlertTitle>Work interrupted</AlertTitle>
          <AlertDescription>
            Actions already taken have not been repeated. Review the progress below, then{" "}
            <Link href={`/app/bots/${detail.botId}`} className="underline">
              give your bot a follow-up
            </Link>{" "}
            to continue safely.
          </AlertDescription>
        </Alert>
      ) : null}

      {detail?.status === "failed" ? (
        <Alert variant="destructive">
          <AlertTitle>Could not finish</AlertTitle>
          <AlertDescription>
            Your bot could not finish this assignment. Review its progress and connection, then
            send a follow-up.
          </AlertDescription>
        </Alert>
      ) : null}

      {detail?.assistantResult ? (
        <Frame spacing="sm">
          <FrameHeader>
            <FrameTitle>{active(detail.status) ? "Work so far" : "Result"}</FrameTitle>
          </FrameHeader>
          <FramePanel>
            <p className="whitespace-pre-wrap break-words text-sm leading-7">
              {detail.assistantResult}
            </p>
          </FramePanel>
        </Frame>
      ) : null}

      <ResultsPanel runId={runId} />

      <Frame spacing="sm">
        <FrameHeader>
          <FrameTitle>Progress</FrameTitle>
        </FrameHeader>
        <FramePanel>
          {timeline.length > 0 ? (
            <Timeline value={timelineStep} className="w-full">
              {timeline.map((item, index) => (
                <TimelineItem key={item.id} step={index + 1}>
                  <TimelineHeader>
                    <TimelineTitle>{timelineTitle(item)}</TimelineTitle>
                  </TimelineHeader>
                  <TimelineIndicator />
                  <TimelineSeparator />
                  <TimelineContent>
                    {item.approval ? (
                      <ApprovalCard payload={item.approval} externalStatus={item.decision} />
                    ) : (
                      <p className="text-muted-foreground">{item.text}</p>
                    )}
                  </TimelineContent>
                </TimelineItem>
              ))}
            </Timeline>
          ) : (
            <WorkspaceEmptyState
              title="Waiting for activity"
              description="Your bot’s steps and approval requests will appear here as work runs."
              icon={<ListTree aria-hidden />}
            />
          )}
        </FramePanel>
      </Frame>

      {detail ? (
        <Link
          href={`/app/bots/${detail.botId}`}
          className="inline-block text-sm underline underline-offset-4"
        >
          Back to your bot
        </Link>
      ) : null}
    </section>
  );
}
