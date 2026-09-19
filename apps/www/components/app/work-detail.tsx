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
import { SubagentCard } from "@/components/app/subagent-card";
import {
  isSubagentEvent,
  subagentActivityFromPayload,
  subagentHeadline,
  type SubagentActivity,
} from "@/lib/subagent-events";
import {
  ApprovalCard,
  type ApprovalRequestedPayload,
  type ApprovalTerminalState,
} from "./approval-card";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
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
import { UserQuestionCard, type UserQuestionPayload, userQuestionFromPayload } from "@/components/app/user-question-card";
import { ConnectorNeedCard } from "@/components/app/connector-need-card";
import {
  botChatReturnTo,
  parseConnectorNeed,
  parseConnectorNeedStatus,
  type ConnectorNeed,
} from "@/lib/connector-need";
import { MessageAttachmentList } from "@/components/app/workspace/message-attachments";
import {
  fetchHumanInterventionStatus,
  type PendingHumanIntervention,
} from "@/lib/human-intervention";
import {
  BrowserPreviewProvider,
  useOptionalBrowserPreviewContext,
} from "@/contexts/browser-preview-context";
import { BrowserPreviewView } from "@/components/app/workspace/browser-preview-view";
import { HumanInterventionBanner } from "./human-intervention-banner";
import { ArrowLeft } from "@/components/icons/lucide";
import { cn } from "cn";

type Activity = {
  id: string;
  text?: string;
  approval?: ApprovalRequestedPayload;
  decision?: ApprovalTerminalState;
  subagent?: SubagentActivity;
  question?: UserQuestionPayload;
  need?: ConnectorNeed;
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
  if (item.need) {
    return item.need.status.phase === "resolved" ? "GitHub connected" : "GitHub needed";
  }
  if (item.question) {
    return item.question.status === "answered" ? "Choice saved" : "Needs a choice";
  }
  if (item.subagent) {
    return subagentHeadline(item.subagent);
  }
  if (item.approval) {
    return item.decision
      ? `Approval ${item.decision}`
      : "Approval required";
  }
  return item.text?.trim() || "Update";
}

function hasInteractiveTimelineItem(item: Activity) {
  return Boolean(item.approval || item.subagent || item.question || item.need);
}

function WorkBrowserColumn({ show }: { show: boolean }) {
  const ctx = useOptionalBrowserPreviewContext();
  const hasImage = Boolean(ctx?.frame?.imageDataUrl);
  if (!show && !hasImage) {
    return null;
  }

  return (
    <aside className="min-w-0 lg:col-start-2 lg:row-start-1 lg:row-span-3 lg:sticky lg:top-4">
      <BrowserPreviewView variant="work" className="w-full" />
    </aside>
  );
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
  const [savingSkill, setSavingSkill] = useState(false);
  const [pendingHumanIntervention, setPendingHumanIntervention] =
    useState<PendingHumanIntervention | null>(null);

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
    void fetchHumanInterventionStatus(runId)
      .then((status) => {
        if (!controller.signal.aborted) {
          setPendingHumanIntervention(status.pending);
        }
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
              event.event === "human_intervention_requested" &&
              typeof payload.interventionId === "string"
            ) {
              setDetail((previous) => {
                const computerId = String(
                  payload.computerId ?? previous?.computerId ?? "",
                );
                setPendingHumanIntervention({
                  id: payload.interventionId as string,
                  runId,
                  computerId,
                  reason: String(payload.reason ?? "other"),
                  message: String(payload.message ?? "The Bot needs your help"),
                  requestedAt: new Date().toISOString(),
                });
                return previous;
              });
            } else if (event.event === "human_intervention_resolved") {
              setPendingHumanIntervention(null);
            } else if (
              event.event === "approval_requested" &&
              typeof payload.approvalId === "string" &&
              typeof payload.summary === "string"
            ) {
              const approval: ApprovalRequestedPayload = {
                approvalId: payload.approvalId,
                summary: payload.summary,
                tool: String(payload.tool ?? ""),
                operationKind: String(payload.operationKind ?? ""),
                botId: typeof payload.botId === "string" ? payload.botId : undefined,
                botName: typeof payload.botName === "string" ? payload.botName : undefined,
                policyOverridable:
                  typeof payload.policyOverridable === "boolean"
                    ? payload.policyOverridable
                    : undefined,
                policyActionLabel:
                  typeof payload.policyActionLabel === "string"
                    ? payload.policyActionLabel
                    : undefined,
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
            } else if (event.event === "user_question_requested") {
              const question = userQuestionFromPayload(runId, payload);
              if (question) {
                setTimeline((previous) =>
                  previous.some((item) => item.question?.questionId === question.questionId)
                    ? previous
                    : [...previous, { id, question }],
                );
              }
            } else if (event.event === "user_question_answered") {
              const selectedIndex =
                typeof payload.selectedIndex === "number" ? payload.selectedIndex : null;
              setTimeline((previous) =>
                previous.map((item) => {
                  if (!item.question || item.question.questionId !== payload.questionId) {
                    return item;
                  }
                  return {
                    ...item,
                    question: {
                      ...item.question,
                      selectedIndex,
                      status: "answered",
                    },
                  };
                }),
              );
            } else if (event.event === "connector_needed") {
              const need = parseConnectorNeed(runId, payload);
              if (need) {
                setTimeline((previous) =>
                  previous.some((item) => item.need?.needId === need.needId)
                    ? previous
                    : [...previous, { id, need }],
                );
              }
            } else if (event.event === "connector_needed_resolved") {
              const status = parseConnectorNeedStatus(payload);
              const needId = typeof payload.needId === "string" ? payload.needId : "";
              if (needId && status.phase === "resolved") {
                setTimeline((previous) =>
                  previous.map((item) => {
                    if (!item.need || item.need.needId !== needId) {
                      return item;
                    }
                    return { ...item, need: { ...item.need, status } };
                  }),
                );
              }
            } else if (isSubagentEvent(event.event)) {
              const activity = subagentActivityFromPayload(payload);
              if (activity) {
                setTimeline((previous) => {
                  const existing = previous.findIndex(
                    (item) => item.subagent?.subagentId === activity.subagentId,
                  );
                  if (existing >= 0) {
                    return previous.map((item, index) =>
                      index === existing
                        ? { ...item, subagent: { ...item.subagent, ...activity } }
                        : item,
                    );
                  }
                  return [...previous.slice(-199), { id, subagent: activity }];
                });
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
      setError(err instanceof Error ? err.message : "Could not archive work");
      setDeleting(false);
    }
  }

  async function handleSaveAsSkill() {
    if (savingSkill || !detail || detail.status !== "completed") return;
    setSavingSkill(true);
    setError(null);
    try {
      const draftResponse = await cloudHostFetch(`/v1/runs/${runId}/skill-draft`, {
        method: "POST",
      });
      const draftBody = await draftResponse.json();
      if (!draftResponse.ok) {
        throw new Error(
          typeof draftBody.error === "string"
            ? draftBody.error
            : "Could not generate a skill draft from this run",
        );
      }
      const slug =
        typeof draftBody.parsedName === "string" ? draftBody.parsedName.trim() : "";
      if (!slug) {
        throw new Error("Skill draft is missing a valid name in SKILL.md frontmatter");
      }
      const createResponse = await cloudHostFetch("/v1/skills", {
        method: "POST",
        body: JSON.stringify({
          slug,
          skillMd: draftBody.skillMd,
          files: draftBody.files ?? [],
        }),
      });
      const created = await createResponse.json();
      if (!createResponse.ok) {
        const message =
          typeof created.error === "string"
            ? created.error
            : "Could not publish skill draft";
        if (
          createResponse.status === 409 ||
          message.toLowerCase().includes("duplicate") ||
          message.toLowerCase().includes("unique")
        ) {
          throw new Error(
            `A skill named "${slug}" already exists. Rename the skill in the draft or delete the existing skill before saving.`,
          );
        }
        throw new Error(message);
      }
      router.push(`/app/skills/${created.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save skill");
    } finally {
      setSavingSkill(false);
    }
  }

  const previewEnabled = Boolean(detail?.computerId && detail && active(detail.status));
  const showBrowser = previewEnabled || Boolean(pendingHumanIntervention);
  const canDelete = canArchiveWorkRun(detail?.status) && !deleting;
  const assignment = detail?.task?.trim();
  const hasOutcome = Boolean(
    pendingHumanIntervention ||
      error ||
      detail?.status === "interrupted" ||
      detail?.status === "failed" ||
      detail?.assistantResult,
  );

  return (
    <BrowserPreviewProvider
      computerId={detail?.computerId ?? null}
      enabled={previewEnabled}
      sessionKey={runId}
    >
      <article className="flex min-w-0 flex-col gap-8">
        <p className="sr-only" role="status">
          {connection}
        </p>

        <header className="flex min-w-0 flex-col gap-4">
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
            <Link
              href="/app/work"
              className="inline-flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <ArrowLeft className="size-3.5" aria-hidden />
              All work
            </Link>
            {detail ? (
              <Link
                href={`/app/bots/${detail.botId}`}
                className="text-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              >
                Open chat
              </Link>
            ) : null}
          </div>

          <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
            <div className="min-w-0 flex-1 space-y-3">
              <h1 className="max-w-3xl text-base font-semibold leading-snug tracking-tight text-pretty sm:text-lg">
                {assignment ||
                  (detail ? "Untitled assignment" : "Loading assignment…")}
              </h1>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={statusBadgeVariant(detail?.status)}>
                  {detail ? workStatus(detail.status) : "Loading…"}
                </Badge>
                {detail?.originLabel ? (
                  <Badge variant="secondary">Started from {detail.originLabel}</Badge>
                ) : null}
              </div>
              {detail?.attachments && detail.attachments.length > 0 ? (
                <MessageAttachmentList attachments={detail.attachments} align="start" />
              ) : null}
            </div>
            <div className="flex shrink-0 flex-wrap items-center gap-2">
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
              {detail?.status === "completed" ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={savingSkill}
                  onClick={() => void handleSaveAsSkill()}
                >
                  {savingSkill ? "Preparing…" : "Save as skill"}
                </Button>
              ) : null}
              {canDelete ? (
                <MessageDeleteButton
                  intent="archive"
                  onDelete={handleDeleteMessage}
                  className="h-7 text-muted-foreground hover:text-foreground"
                />
              ) : null}
            </div>
          </div>
        </header>

        {delegations.length > 0 ? (
          <section className="space-y-3">
            <h2 className="text-sm font-medium">Bot handoffs</h2>
            {delegations.map((delegation) => (
              <DelegationCard key={delegation.id} delegation={delegation} />
            ))}
          </section>
        ) : null}

        {detail?.memories && detail.memories.length > 0 ? (
          <details className="max-w-3xl text-sm">
            <summary className="cursor-pointer text-muted-foreground transition-colors hover:text-foreground">
              Used {detail.memories.length}{" "}
              {detail.memories.length === 1 ? "memory" : "memories"}
            </summary>
            <ul className="mt-2 space-y-1 text-muted-foreground">
              {detail.memories.map((memory, index) => (
                <li key={`${memory.kind}-${index}`}>{memory.content}</li>
              ))}
            </ul>
          </details>
        ) : null}

        <div
          className={cn(
            "grid min-w-0 gap-8",
            "lg:grid-cols-1 lg:items-start lg:has-[aside]:grid-cols-[minmax(0,1fr)_minmax(18rem,22rem)]",
          )}
        >
          {hasOutcome ? (
            <div className="flex min-w-0 flex-col gap-8 lg:col-start-1">
              {pendingHumanIntervention ? (
                <HumanInterventionBanner pending={pendingHumanIntervention} />
              ) : null}

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
                    Actions already taken have not been repeated. Review progress below, then send a
                    follow-up from chat.
                  </AlertDescription>
                </Alert>
              ) : null}

              {detail?.status === "failed" ? (
                <Alert variant="destructive">
                  <AlertTitle>Could not finish</AlertTitle>
                  <AlertDescription>
                    Review progress below, then send a follow-up from chat.
                  </AlertDescription>
                </Alert>
              ) : null}

              {detail?.assistantResult ? (
                <section className="max-w-3xl space-y-3">
                  <h2 className="text-sm font-medium">
                    {active(detail.status) ? "Work so far" : "Result"}
                  </h2>
                  <MarkdownContent text={detail.assistantResult} />
                </section>
              ) : null}
            </div>
          ) : null}

          <WorkBrowserColumn show={showBrowser} />

          <div className="flex min-w-0 flex-col gap-8 lg:col-start-1">
            <ResultsPanel runId={runId} variant="compact" />

            <section className="space-y-3">
              <h2 className="text-sm font-medium">Progress</h2>
              {timeline.length > 0 ? (
                <Timeline value={timelineStep} className="w-full">
                  {timeline.map((item, index) => (
                    <TimelineItem
                      key={item.id}
                      step={index + 1}
                      className="group-data-[orientation=vertical]/timeline:not-last:pb-3"
                    >
                      <TimelineHeader>
                        <TimelineTitle className="font-normal leading-snug">
                          {timelineTitle(item)}
                        </TimelineTitle>
                      </TimelineHeader>
                      <TimelineIndicator className="size-2.5 border-border group-data-completed/timeline-item:border-muted-foreground" />
                      <TimelineSeparator className="bg-border" />
                      {hasInteractiveTimelineItem(item) ? (
                        <TimelineContent>
                          {item.approval ? (
                            <ApprovalCard
                              payload={item.approval}
                              externalStatus={item.decision}
                            />
                          ) : item.subagent ? (
                            <SubagentCard activity={item.subagent} />
                          ) : item.question ? (
                            <UserQuestionCard question={item.question} />
                          ) : item.need && detail ? (
                            <ConnectorNeedCard
                              need={item.need}
                              returnTo={botChatReturnTo({
                                botId: detail.botId,
                                conversationId: detail.conversationId,
                              })}
                            />
                          ) : null}
                        </TimelineContent>
                      ) : null}
                    </TimelineItem>
                  ))}
                </Timeline>
              ) : (
                <p className="text-sm text-muted-foreground">
                  {detail && active(detail.status)
                    ? "Steps will appear here as your bot works."
                    : "No activity recorded."}
                </p>
              )}
            </section>
          </div>
        </div>
      </article>
    </BrowserPreviewProvider>
  );
}
