import type { ApprovalRequestedPayload, ApprovalTerminalState } from "@/components/app/approval-card";
import type { UserQuestionPayload } from "@/components/app/user-question-card";
import { userQuestionFromPayload } from "@/components/app/user-question-card";
import { activityLineFromEvent } from "@/lib/run-activity";
import type { PendingHumanIntervention } from "@/lib/human-intervention";
import {
  isSubagentEvent,
  subagentActivityFromPayload,
  type SubagentActivity,
} from "@/lib/subagent-events";
import type { RunActivityItem } from "@/contexts/active-run-context";

export interface RunTimelineState {
  items: RunActivityItem[];
  pendingHumanIntervention: PendingHumanIntervention | null;
}

export function emptyRunTimelineState(): RunTimelineState {
  return { items: [], pendingHumanIntervention: null };
}

export interface RunStreamEvent {
  id?: string;
  event: string;
  data: string;
}

function parsePayload(data: string): Record<string, unknown> {
  try {
    return JSON.parse(data) as Record<string, unknown>;
  } catch {
    return {};
  }
}

function approvalFromPayload(payload: Record<string, unknown>): ApprovalRequestedPayload | null {
  if (typeof payload.approvalId !== "string" || typeof payload.summary !== "string") {
    return null;
  }
  return {
    approvalId: payload.approvalId,
    summary: payload.summary,
    tool: String(payload.tool ?? ""),
    operationKind: String(payload.operationKind ?? ""),
    botId: typeof payload.botId === "string" ? payload.botId : undefined,
    botName: typeof payload.botName === "string" ? payload.botName : undefined,
    policyOverridable:
      typeof payload.policyOverridable === "boolean" ? payload.policyOverridable : undefined,
    policyActionLabel:
      typeof payload.policyActionLabel === "string" ? payload.policyActionLabel : undefined,
    connectedAppName:
      typeof payload.connectedAppName === "string" ? payload.connectedAppName : undefined,
    connectedToolName:
      typeof payload.connectedToolName === "string" ? payload.connectedToolName : undefined,
    argumentSummary:
      payload.argumentSummary && typeof payload.argumentSummary === "object"
        ? (payload.argumentSummary as Record<string, unknown>)
        : undefined,
  };
}

function shouldSkipDuplicateText(items: RunActivityItem[], headline: string): boolean {
  const last = items[items.length - 1];
  return last?.kind === "text" && last.text === headline;
}

export function applyRunStreamEvent(
  state: RunTimelineState,
  runId: string,
  streamEvent: RunStreamEvent,
  options?: { skipIfSeenId?: (id: string) => boolean },
): RunTimelineState {
  const payload = parsePayload(streamEvent.data);
  const id = streamEvent.id ?? `evt-${streamEvent.event}-${state.items.length}`;
  const isNew = options?.skipIfSeenId ? options.skipIfSeenId(id) : true;

  if (streamEvent.event === "stream_error") {
    return state;
  }

  if (streamEvent.event === "assistant_delta") {
    return state;
  }

  let { items, pendingHumanIntervention } = state;

  if (
    streamEvent.event === "human_intervention_requested" &&
    typeof payload.interventionId === "string"
  ) {
    pendingHumanIntervention = {
      id: payload.interventionId,
      runId,
      computerId: String(payload.computerId ?? ""),
      reason: String(payload.reason ?? "other"),
      message: String(payload.message ?? "The Bot needs your help"),
      requestedAt: new Date().toISOString(),
    };
    return { items, pendingHumanIntervention };
  }

  if (streamEvent.event === "human_intervention_resolved") {
    return { items, pendingHumanIntervention: null };
  }

  if (streamEvent.event === "approval_requested") {
    const approval = approvalFromPayload(payload);
    if (!approval) {
      return state;
    }
    if (items.some((item) => item.id === id)) {
      return state;
    }
    return {
      items: [...items, { id, kind: "approval", approval }],
      pendingHumanIntervention,
    };
  }

  if (streamEvent.event === "approval_resolved") {
    const decision = payload.decision;
    if (!["approved", "denied", "cancelled", "expired"].includes(String(decision))) {
      return state;
    }
    return {
      items: items.map((item) =>
        item.kind === "approval" && item.approval.approvalId === payload.approvalId
          ? { ...item, decision: decision as ApprovalTerminalState }
          : item,
      ),
      pendingHumanIntervention,
    };
  }

  if (streamEvent.event === "user_question_requested") {
    const question = userQuestionFromPayload(runId, payload);
    if (!question) {
      return state;
    }
    if (
      items.some(
        (item) => item.kind === "question" && item.question.questionId === question.questionId,
      )
    ) {
      return state;
    }
    return {
      items: [...items, { id, kind: "question", question }],
      pendingHumanIntervention,
    };
  }

  if (streamEvent.event === "user_question_answered") {
    const selectedIndex =
      typeof payload.selectedIndex === "number" ? payload.selectedIndex : null;
    return {
      items: items.map((item) =>
        item.kind === "question" && item.question.questionId === payload.questionId
          ? {
              ...item,
              question: {
                ...item.question,
                selectedIndex,
                status: "answered",
              },
            }
          : item,
      ),
      pendingHumanIntervention,
    };
  }

  if (streamEvent.event === "user_question_cancelled") {
    return {
      items: items.map((item) =>
        item.kind === "question" && item.question.questionId === payload.questionId
          ? {
              ...item,
              question: { ...item.question, status: "cancelled" },
            }
          : item,
      ),
      pendingHumanIntervention,
    };
  }

  if (isSubagentEvent(streamEvent.event)) {
    const activity = subagentActivityFromPayload(payload);
    if (!activity) {
      return state;
    }
    const existing = items.findIndex(
      (item) => item.kind === "subagent" && item.subagent.subagentId === activity.subagentId,
    );
    if (existing >= 0) {
      return {
        items: items.map((item, index) =>
          index === existing && item.kind === "subagent"
            ? { ...item, subagent: { ...item.subagent, ...activity } }
            : item,
        ),
        pendingHumanIntervention,
      };
    }
    if (!isNew) {
      return state;
    }
    return {
      items: [...items.slice(-199), { id, kind: "subagent", subagent: activity }],
      pendingHumanIntervention,
    };
  }

  const line = activityLineFromEvent(streamEvent.event, payload);
  if (!line || !isNew) {
    return state;
  }
  if (items.some((item) => item.id === id)) {
    return state;
  }
  if (shouldSkipDuplicateText(items, line.headline)) {
    return state;
  }
  return {
    items: [
      ...items.slice(-199),
      { id, kind: "text", text: line.headline, technical: line.technical },
    ],
    pendingHumanIntervention,
  };
}

export function replayRunStreamEvents(
  runId: string,
  events: RunStreamEvent[],
): RunTimelineState {
  const seen = new Set<string>();
  let state = emptyRunTimelineState();
  for (const event of events) {
    state = applyRunStreamEvent(state, runId, event, {
      skipIfSeenId: (eventId) => {
        if (seen.has(eventId)) {
          return false;
        }
        seen.add(eventId);
        return true;
      },
    });
  }
  return state;
}
