import type { ApprovalRequestedPayload, ApprovalTerminalState } from "@/components/app/approval-card";
import type { UserQuestionPayload } from "@/components/app/user-question-card";
import { userQuestionFromPayload } from "@/components/app/user-question-card";
import { activityLineFromEvent } from "@/lib/run-activity";
import type { PendingHumanIntervention } from "@/lib/human-intervention";
import {
  isSubagentEvent,
  subagentActivityFromPayload,
} from "@/lib/subagent-events";
import type { RunActivityItem, ToolActivity } from "@/contexts/active-run-context";
import {
  parseConnectorNeed,
  parseConnectorNeedStatus,
} from "@/lib/connector-need";

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

function toolNameFromPayload(payload: Record<string, unknown>): string {
  return String(payload.tool ?? payload.name ?? "").trim();
}

function upsertToolItem(items: RunActivityItem[], id: string, tool: ToolActivity): RunActivityItem[] {
  const existing = items.findIndex((item) => item.id === id || (item.kind === "tool" && item.id === id));
  if (existing >= 0) {
    return items.map((item, index) =>
      index === existing && item.kind === "tool" ? { ...item, tool: { ...item.tool, ...tool } } : item,
    );
  }
  return [...items.slice(-199), { id, kind: "tool", tool }];
}

function isConnectorAccessErrorCode(code: unknown): boolean {
  return code === "not_connected" || code === "reconnect_required";
}

function isConnectorAccessMiss(
  items: RunActivityItem[],
  payload: Record<string, unknown>,
  toolName: string,
): boolean {
  if (isConnectorAccessErrorCode(payload.errorCode)) {
    return true;
  }
  return (
    toolName.startsWith("github_") &&
    items.some(
      (item) => item.kind === "connector" && item.need.status.phase === "pending",
    )
  );
}

function toolItemId(
  items: RunActivityItem[],
  payload: Record<string, unknown>,
  toolName: string,
): string {
  if (typeof payload.toolInvocationId === "string" && payload.toolInvocationId) {
    return `tool:${payload.toolInvocationId}`;
  }
  const open = [...items]
    .reverse()
    .find((item) => item.kind === "tool" && item.tool.toolName === toolName && item.tool.state === "running");
  if (open) {
    return open.id;
  }
  return `tool:${toolName}:${items.length}`;
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

  if (streamEvent.event === "connector_needed") {
    const need = parseConnectorNeed(runId, payload);
    if (!need) {
      return state;
    }
    if (items.some((item) => item.kind === "connector" && item.need.needId === need.needId)) {
      return state;
    }
    return {
      items: [...items, { id, kind: "connector", need }],
      pendingHumanIntervention,
    };
  }

  if (streamEvent.event === "connector_needed_resolved") {
    const needId = typeof payload.needId === "string" ? payload.needId : "";
    const status = parseConnectorNeedStatus(payload);
    if (!needId || status.phase !== "resolved") {
      return state;
    }
    return {
      items: items.map((item) =>
        item.kind === "connector" && item.need.needId === needId
          ? { ...item, need: { ...item.need, status } }
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

  const toolName = toolNameFromPayload(payload);
  if (
    toolName &&
    (streamEvent.event === "tool_started" ||
      streamEvent.event === "tool_result" ||
      streamEvent.event.startsWith("tool_"))
  ) {
    if (
      streamEvent.event === "tool_result" &&
      payload.ok === false &&
      isConnectorAccessMiss(items, payload, toolName)
    ) {
      const toolId = toolItemId(items, payload, toolName);
      return {
        items: items.filter(
          (item) =>
            !(item.kind === "tool" && item.id === toolId && item.tool.state === "running"),
        ),
        pendingHumanIntervention,
      };
    }
    const line = activityLineFromEvent(streamEvent.event, payload);
    const toolState: ToolActivity["state"] =
      streamEvent.event === "tool_result" ? (payload.ok === false ? "error" : "complete") : "running";
    const toolId = toolItemId(items, payload, toolName);
    return {
      items: upsertToolItem(items, toolId, {
        toolName,
        label: line?.headline || toolName.replace(/_/g, " "),
        state: toolState,
        technical: line?.technical,
      }),
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
