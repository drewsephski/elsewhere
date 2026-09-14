"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { DelegationSummary, RunDetail } from "@/lib/api-types";
import { archiveWorkRun, canArchiveWorkRun } from "@/lib/archive-work-run";
import { DelegationCard } from "@/components/app/delegation-card";
import { MarkdownContent } from "@/components/app/markdown-content";
import { MessageDeleteButton } from "@/components/app/message-delete-button";
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
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { ListTree } from "lucide-react";
import { BrowserPreviewProvider } from "@/contexts/browser-preview-context";
import { BrowserPreviewView } from "@/components/app/workspace/browser-preview-view";

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
  const router = useRouter();
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [timeline, setTimeline] = useState<Activity[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState("Connecting to your work…");
  const [stopping, setStopping] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [delegations, setDelegations] = useState<DelegationSummary[]>([]);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/runs/${runId}/delegations`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) return;
        setDelegations(await response.json());
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, [runId]);

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

  async function handleDeleteMessage() {
    setDeleting(true);
    setError(null);
    try {
      await archiveWorkRun(runId);
      if (detail?.botId) {
        router.push(`/app/bots/${detail.botId}`);
        return;
      }
      router.push("/app");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delete message");
      setDeleting(false);
    }
  }

  const previewEnabled = Boolean(detail?.computerId && detail && active(detail.status));
  const canDelete = canArchiveWorkRun(detail?.status) && !deleting;

  return (
    <BrowserPreviewProvider
      computerId={detail?.computerId ?? null}
      enabled={previewEnabled}
      sessionKey={runId}
    >
    <section className="space-y-6">
      <div className="mx-auto flex max-w-2xl flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Your message
            </p>
            <p className="mt-1 text-xs text-muted-foreground" role="status">
              {connection}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Badge variant={statusBadgeVariant(detail?.status)}>
              {detail ? workStatus(detail.status) : "Loading…"}
            </Badge>
            {canDelete ? (
              <MessageDeleteButton
                onDelete={handleDeleteMessage}
                label="Delete"
                className="h-8 text-xs text-muted-foreground hover:text-destructive"
              />
            ) : null}
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

        {delegations.length > 0 ? (
          <div className="space-y-3">
            <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Bot handoffs
            </p>
            {delegations.map((delegation) => (
              <DelegationCard key={delegation.id} delegation={delegation} />
            ))}
          </div>
        ) : null}

        {detail?.task ? (
          <div className="flex flex-col items-end gap-1">
            <UserPromptBubble>{detail.task}</UserPromptBubble>
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            {detail ? "No assignment text saved for this work." : "Loading assignment…"}
          </p>
        )}
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
            <MarkdownContent text={detail.assistantResult} />
          </FramePanel>
        </Frame>
      ) : null}

      <ResultsPanel runId={runId} />

      {detail?.computerId ? (
        <div className="mx-auto max-w-2xl border-t border-border/50 pt-4">
          <BrowserPreviewView variant="work" />
        </div>
      ) : null}

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
    </BrowserPreviewProvider>
  );
}
