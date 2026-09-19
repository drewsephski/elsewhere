export type ConnectorProvider = "github";

export type ConnectorNeedReason =
  | { kind: "disconnected" }
  | { kind: "reconnect_required" }
  | { kind: "unauthorized_repo"; owner: string; repo: string }
  | { kind: "empty_authorization" }
  | { kind: "host_unconfigured" };

export type ConnectorNeedResolutionKind =
  | "connected"
  | "repo_authorized"
  | "dismissed"
  | "expired"
  | "cancelled";

export type ConnectorNeedStatus =
  | { phase: "pending" }
  | { phase: "resolved"; resolution: ConnectorNeedResolutionKind };

export type ConnectorNeed = {
  needId: string;
  provider: ConnectorProvider;
  reason: ConnectorNeedReason;
  runId: string;
  botId: string;
  toolName: string;
  requestedAt: string;
  status: ConnectorNeedStatus;
  blockedInvocationId?: string;
};

export type BotChatReturnTo = string & { readonly __brand: "BotChatReturnTo" };

export type GithubOAuthStartResponse = {
  authorizeUrl: string;
  state: string;
  expiresAt: string;
};

export type GithubOAuthCompleteResponse = {
  provider: "github";
  status: string;
  metadata: Record<string, unknown>;
  connectedAt: string | null;
  updatedAt: string;
  returnTo: string | null;
};

const RESOLUTION_KINDS = new Set<ConnectorNeedResolutionKind>([
  "connected",
  "repo_authorized",
  "dismissed",
  "expired",
  "cancelled",
]);

function asNonEmptyString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

export function parseConnectorNeedReason(value: unknown): ConnectorNeedReason | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const reason = value as Record<string, unknown>;
  const kind = asNonEmptyString(reason.kind);
  if (kind === "disconnected") {
    return { kind: "disconnected" };
  }
  if (kind === "reconnect_required") {
    return { kind: "reconnect_required" };
  }
  if (kind === "empty_authorization") {
    return { kind: "empty_authorization" };
  }
  if (kind === "host_unconfigured") {
    return { kind: "host_unconfigured" };
  }
  if (kind === "unauthorized_repo") {
    const owner = asNonEmptyString(reason.owner);
    const repo = asNonEmptyString(reason.repo);
    if (!owner || !repo) {
      return null;
    }
    return { kind: "unauthorized_repo", owner, repo };
  }
  return null;
}

export function parseConnectorNeedStatus(payload: Record<string, unknown>): ConnectorNeedStatus {
  const resolution = payload.resolution;
  if (typeof resolution === "string" && RESOLUTION_KINDS.has(resolution as ConnectorNeedResolutionKind)) {
    return { phase: "resolved", resolution: resolution as ConnectorNeedResolutionKind };
  }
  return { phase: "pending" };
}

export function parseConnectorNeed(
  runId: string,
  payload: Record<string, unknown>,
): ConnectorNeed | null {
  const needId = asNonEmptyString(payload.needId);
  const reason = parseConnectorNeedReason(payload.reason);
  const botId = asNonEmptyString(payload.botId);
  const toolName = asNonEmptyString(payload.toolName);
  const requestedAt = asNonEmptyString(payload.requestedAt);
  const provider = payload.provider === "github" ? "github" : null;
  const payloadRunId = asNonEmptyString(payload.runId) ?? runId;
  if (!needId || !reason || !botId || !toolName || !requestedAt || !provider) {
    return null;
  }
  const blockedInvocationId = asNonEmptyString(payload.blockedInvocationId) ?? undefined;
  return {
    needId,
    provider,
    reason,
    runId: payloadRunId,
    botId,
    toolName,
    requestedAt,
    status: parseConnectorNeedStatus(payload),
    blockedInvocationId,
  };
}

export function parseBotChatReturnTo(raw: string): BotChatReturnTo | null {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.includes("\0")) {
    return null;
  }
  if (trimmed.includes("://") || trimmed.startsWith("//")) {
    return null;
  }
  if (!trimmed.startsWith("/app/bots/")) {
    return null;
  }
  let url: URL;
  try {
    url = new URL(`https://elsewhere.invalid${trimmed}`);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" || url.hostname !== "elsewhere.invalid") {
    return null;
  }
  if (url.hash) {
    return null;
  }
  const botId = url.pathname.slice("/app/bots/".length);
  if (!botId || botId.includes("/") || botId.includes("..")) {
    return null;
  }
  if (![...botId].every((char) => /[A-Za-z0-9_-]/.test(char))) {
    return null;
  }
  const keys = [...new Set([...url.searchParams.keys()])].sort();
  if (keys.length === 0) {
    return `/app/bots/${botId}` as BotChatReturnTo;
  }
  if (keys.length === 1 && keys[0] === "conversation") {
    const conversation = url.searchParams.get("conversation") ?? "";
    if (
      !conversation ||
      conversation.includes("/") ||
      conversation.includes("..") ||
      ![...conversation].every((char) => /[A-Za-z0-9_-]/.test(char))
    ) {
      return null;
    }
    return `/app/bots/${botId}?conversation=${conversation}` as BotChatReturnTo;
  }
  return null;
}

export function botChatReturnTo(input: {
  botId: string;
  conversationId?: string | null;
}): BotChatReturnTo {
  const raw = input.conversationId
    ? `/app/bots/${input.botId}?conversation=${input.conversationId}`
    : `/app/bots/${input.botId}`;
  const parsed = parseBotChatReturnTo(raw);
  if (!parsed) {
    throw new Error("invalid bot chat return path");
  }
  return parsed;
}

export function presentationForConnectorNeed(need: ConnectorNeed): {
  title: string;
  reason: string;
  actionLabel: string;
  continuation: string;
} {
  const continuation = "Your bot continues after you connect.";
  switch (need.reason.kind) {
    case "disconnected":
      return {
        title: "Connect GitHub?",
        reason: "This bot needs GitHub to work on repositories.",
        actionLabel: "Connect GitHub",
        continuation,
      };
    case "reconnect_required":
      return {
        title: "Reconnect GitHub?",
        reason: "GitHub is linked but this account must sign in again.",
        actionLabel: "Reconnect GitHub",
        continuation,
      };
    case "unauthorized_repo":
      return {
        title: "Add this repository?",
        reason: `GitHub is connected, but ${need.reason.owner}/${need.reason.repo} is not in the authorized installation.`,
        actionLabel: `Add ${need.reason.owner} on GitHub`,
        continuation,
      };
    case "empty_authorization":
      return {
        title: "Grant repository access?",
        reason: "GitHub is connected, but no repositories are authorized yet.",
        actionLabel: "Add repositories",
        continuation,
      };
    case "host_unconfigured":
      return {
        title: "GitHub isn't available",
        reason: "This host hasn't been set up to connect GitHub yet.",
        actionLabel: "Connect GitHub",
        continuation: "The bot cannot use GitHub until an operator enables it.",
      };
  }
}
