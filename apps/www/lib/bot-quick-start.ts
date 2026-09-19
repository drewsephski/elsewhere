import type { BotSummary, ComputerSummary, CreateRunResponse } from "@/lib/api-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { cloudApiErrorFromResponse, isCloudApiError } from "@/lib/cloud-api-error";
import { isLocalMacProvider } from "@/lib/computer-kind";
import { isThisMacLiveForQuickStart } from "@/lib/this-mac-status";
import type { ThisMacStatusSnapshot } from "@/lib/this-mac-status";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";
import { formatUserFacingError } from "@/lib/format-api-error";
import {
  readSessionStorage,
  removeSessionStorage,
  writeSessionStorage,
} from "@/lib/session-storage";

/** Generic first-Bot instructions used when the user does not pick a role. */
export const DEFAULT_BOT_INSTRUCTIONS =
  "Complete delegated work carefully, keep useful files on your computer, and explain your results clearly. Ask for approval before making changes.";

export const DEFAULT_CLOUD_COMPUTER_NAME = "Workspace";

export const DEFAULT_BOT_ENGINE = "codex";

const QUICK_START_DRAFT_KEY = "elsewhere:quick-start-draft";

export type QuickStartSession = {
  computerId?: string;
  botId?: string;
  idempotencyKey?: string;
  taskFingerprint?: string;
};

export type QuickStartOutcome =
  | {
      status: "started";
      botId: string;
      computerId: string;
      runId: string;
      conversationId: string;
    }
  | {
      status: "needs_codex";
      error: string;
      botId?: string;
      computerId?: string;
    }
  | {
      status: "bot_ready_run_failed";
      error: string;
      botId: string;
      computerId: string;
      task: string;
    }
  | {
      status: "failed";
      error: string;
      botId?: string;
      computerId?: string;
    };

export type CreateBotOutcome =
  | { status: "created"; botId: string; computerId: string }
  | QuickStartOutcome;

export type CreateBotWorkInput = {
  name: string;
  instructions?: string;
  task?: string;
  computerId?: string;
  model?: string;
  avatarId?: string;
  thisMac?: ThisMacStatusSnapshot | null;
};

export function isUsableComputer(computer: ComputerSummary): boolean {
  if (computer.state === "archived") {
    return false;
  }
  if (isLocalMacProvider(computer.provider)) {
    return computer.providerMetadata.connected === true;
  }
  return true;
}

export interface SelectUsableComputerOptions {
  /** When set and live, prefer this cloud computer id for This Mac. */
  thisMac?: ThisMacStatusSnapshot | null;
}

export function selectUsableComputer(
  computers: ComputerSummary[],
  options?: SelectUsableComputerOptions,
): ComputerSummary | null {
  const usable = computers.filter(isUsableComputer);
  if (!usable.length) {
    return null;
  }
  const liveThisMac = isThisMacLiveForQuickStart(options?.thisMac);
  if (liveThisMac && options?.thisMac?.computerId) {
    const match = usable.find((c) => c.id === options.thisMac?.computerId);
    if (match) {
      return match;
    }
  }
  if (liveThisMac) {
    const localMac = usable.filter((computer) =>
      isLocalMacProvider(computer.provider),
    );
    if (localMac.length) {
      const pool = localMac;
      return (
        [...pool].sort((a, b) => {
          const aTime = a.lastUsedAt ?? "";
          const bTime = b.lastUsedAt ?? "";
          return bTime.localeCompare(aTime);
        })[0] ?? null
      );
    }
  }
  const cloud = usable.filter((computer) => !isLocalMacProvider(computer.provider));
  const pool = cloud.length ? cloud : usable;
  return (
    [...pool].sort((a, b) => {
      const aTime = a.lastUsedAt ?? "";
      const bTime = b.lastUsedAt ?? "";
      return bTime.localeCompare(aTime);
    })[0] ?? null
  );
}

export function isCodexAuthRequired(error: unknown): boolean {
  if (isCloudApiError(error) && (error.status === 401 || error.code === "chatgpt_not_connected")) {
    return true;
  }
  const message = (error instanceof Error ? error.message : String(error)).toLowerCase();
  return (
    message.includes("not connected") ||
    message.includes("connect chatgpt") ||
    message.includes("sign in to chatgpt") ||
    message.includes("codex login")
  );
}

export function rememberQuickStartDraft(botId: string, message: string) {
  writeSessionStorage(
    QUICK_START_DRAFT_KEY,
    JSON.stringify({ botId, message }),
  );
}

export function consumeQuickStartDraft(botId: string): string | null {
  const stored = readSessionStorage(QUICK_START_DRAFT_KEY);
  if (!stored) {
    return null;
  }
  try {
    const parsed = JSON.parse(stored) as { botId?: unknown; message?: unknown };
    if (parsed.botId !== botId || typeof parsed.message !== "string") {
      return null;
    }
    removeSessionStorage(QUICK_START_DRAFT_KEY);
    return parsed.message;
  } catch {
    removeSessionStorage(QUICK_START_DRAFT_KEY);
    return null;
  }
}

function taskFingerprint(botId: string | undefined, task: string): string {
  return `${botId ?? ""}|${task}`;
}

async function readOkJson<T>(response: Response, fallback: string): Promise<T> {
  if (!response.ok) {
    throw await cloudApiErrorFromResponse(response, fallback);
  }
  return (await response.json()) as T;
}

export async function ensureQuickStartComputer(
  session: QuickStartSession,
  options?: SelectUsableComputerOptions,
): Promise<string> {
  if (session.computerId) {
    return session.computerId;
  }
  if (isThisMacLiveForQuickStart(options?.thisMac) && options?.thisMac?.computerId) {
    session.computerId = options.thisMac.computerId;
    return options.thisMac.computerId;
  }
  const listResponse = await cloudHostFetch("/v1/computers");
  const computers = await readOkJson<ComputerSummary[]>(
    listResponse,
    "Could not load computers",
  );
  const existing = selectUsableComputer(computers, options);
  if (existing) {
    session.computerId = existing.id;
    return existing.id;
  }
  const createResponse = await cloudHostFetch("/v1/computers", {
    method: "POST",
    body: JSON.stringify({ displayName: DEFAULT_CLOUD_COMPUTER_NAME }),
  });
  const created = await readOkJson<ComputerSummary>(
    createResponse,
    "Could not create a computer",
  );
  session.computerId = created.id;
  return created.id;
}

export async function ensureQuickStartBot(
  session: QuickStartSession,
  input: {
    name: string;
    computerId: string;
    instructions?: string;
    model?: string;
    avatarId?: string;
  },
): Promise<string> {
  if (session.botId) {
    return session.botId;
  }
  const instructions = input.instructions?.trim() || DEFAULT_BOT_INSTRUCTIONS;
  const response = await cloudHostFetch("/v1/bots", {
    method: "POST",
    body: JSON.stringify({
      name: input.name.trim(),
      instructions,
      computerId: input.computerId,
      model: input.model ?? DEFAULT_BOT_MODEL_ID,
      enginePreference: DEFAULT_BOT_ENGINE,
      avatarId: input.avatarId ?? DEFAULT_BOT_AVATAR_ID,
    }),
  });
  const created = await readOkJson<BotSummary>(response, "Could not create bot");
  session.botId = created.id;
  return created.id;
}

async function startBotRun(
  session: QuickStartSession,
  botId: string,
  computerId: string,
  task: string,
): Promise<QuickStartOutcome> {
  const fingerprint = taskFingerprint(botId, task);
  if (session.taskFingerprint !== fingerprint || !session.idempotencyKey) {
    session.idempotencyKey = crypto.randomUUID();
    session.taskFingerprint = fingerprint;
  }

  try {
    const runResponse = await cloudHostFetch("/v1/runs", {
      method: "POST",
      headers: { "Idempotency-Key": session.idempotencyKey },
      body: JSON.stringify({ botId, message: task }),
    });
    const created = await readOkJson<CreateRunResponse>(
      runResponse,
      "Could not start work",
    );
    return {
      status: "started",
      botId,
      computerId,
      runId: created.runId,
      conversationId: created.conversationId,
    };
  } catch (error) {
    const message = formatUserFacingError(error, "Could not start work");
    if (isCodexAuthRequired(error)) {
      return { status: "needs_codex", error: message, botId, computerId };
    }
    return {
      status: "bot_ready_run_failed",
      error: message,
      botId,
      computerId,
      task,
    };
  }
}

/** Shared create-bot path for quick start, New bot, and management surfaces. */
export async function createBotAndMaybeStartWork(
  input: CreateBotWorkInput,
  session: QuickStartSession = {},
): Promise<CreateBotOutcome> {
  const name = input.name.trim();
  const task = input.task?.trim() ?? "";
  const instructions = input.instructions?.trim() || DEFAULT_BOT_INSTRUCTIONS;

  if (!name) {
    return { status: "failed", error: "Name your Bot to get started.", ...session };
  }

  if (input.computerId) {
    session.computerId = input.computerId;
  }

  try {
    const computerId = await ensureQuickStartComputer(session, {
      thisMac: input.thisMac,
    });
    const botId = await ensureQuickStartBot(session, {
      name,
      computerId,
      instructions,
      model: input.model,
      avatarId: input.avatarId,
    });

    if (!task) {
      return { status: "created", botId, computerId };
    }

    const runOutcome = await startBotRun(session, botId, computerId, task);
    return runOutcome;
  } catch (error) {
    return {
      status: "failed",
      error: formatUserFacingError(error, "Could not create this Bot"),
      botId: session.botId,
      computerId: session.computerId,
    };
  }
}

export async function startFirstBotWork(
  input: { name: string; task: string },
  session: QuickStartSession = {},
): Promise<QuickStartOutcome> {
  const task = input.task.trim();
  if (!task) {
    return { status: "failed", error: "Tell this Bot what to work on.", ...session };
  }

  const outcome = await createBotAndMaybeStartWork(
    { name: input.name, task },
    session,
  );
  if (outcome.status === "created") {
    return {
      status: "failed",
      error: "Tell this Bot what to work on.",
      botId: outcome.botId,
      computerId: outcome.computerId,
    };
  }
  return outcome;
}
