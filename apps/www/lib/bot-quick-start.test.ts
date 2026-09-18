// @vitest-environment happy-dom

import type { ComputerSummary } from "@/lib/api-types";
import { CloudApiError } from "@/lib/cloud-api-error";
import { cloudHostFetch } from "@/lib/cloud-api";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  consumeQuickStartDraft,
  DEFAULT_BOT_ENGINE,
  DEFAULT_BOT_INSTRUCTIONS,
  DEFAULT_CLOUD_COMPUTER_NAME,
  isCodexAuthRequired,
  isUsableComputer,
  rememberQuickStartDraft,
  selectUsableComputer,
  startFirstBotWork,
  type QuickStartSession,
} from "./bot-quick-start";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(),
}));

const fetchMock = vi.mocked(cloudHostFetch);

function computer(overrides: Partial<ComputerSummary> & { id: string }): ComputerSummary {
  return {
    displayName: overrides.displayName ?? overrides.id,
    provider: "fly_sprite",
    state: "pending",
    lastUsedAt: null,
    providerMetadata: { provisioned: false },
    ...overrides,
  };
}

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function requestPath(path: string): string {
  return path;
}

afterEach(() => {
  fetchMock.mockReset();
  sessionStorage.clear();
});

describe("selectUsableComputer", () => {
  it("returns null when nothing is usable", () => {
    expect(selectUsableComputer([])).toBeNull();
    expect(
      selectUsableComputer([
        computer({
          id: "mac_offline",
          provider: "local_mac",
          providerMetadata: { provisioned: true, connected: false },
        }),
        computer({ id: "archived", state: "archived" }),
      ]),
    ).toBeNull();
  });

  it("prefers a cloud computer over a connected Mac", () => {
    const cloud = computer({ id: "cloud_1", lastUsedAt: "2026-01-01T00:00:00.000Z" });
    const mac = computer({
      id: "mac_1",
      provider: "local_mac",
      lastUsedAt: "2026-09-01T00:00:00.000Z",
      providerMetadata: { provisioned: true, connected: true },
    });
    expect(selectUsableComputer([mac, cloud])?.id).toBe("cloud_1");
  });

  it("reuses the most recently used cloud computer, including pending ones", () => {
    const older = computer({
      id: "cloud_old",
      state: "active",
      lastUsedAt: "2026-01-01T00:00:00.000Z",
      providerMetadata: { provisioned: true },
    });
    const newer = computer({
      id: "cloud_new",
      state: "pending",
      lastUsedAt: "2026-09-01T00:00:00.000Z",
    });
    expect(selectUsableComputer([older, newer])?.id).toBe("cloud_new");
    expect(isUsableComputer(newer)).toBe(true);
  });

  it("falls back to a connected Mac when no cloud computer exists", () => {
    const mac = computer({
      id: "mac_1",
      provider: "local_mac",
      providerMetadata: { provisioned: true, connected: true },
    });
    expect(selectUsableComputer([mac])?.id).toBe("mac_1");
  });
});

describe("isCodexAuthRequired", () => {
  it("detects ChatGPT connection failures without treating every error as auth", () => {
    expect(
      isCodexAuthRequired(
        new CloudApiError("Connect ChatGPT to run this Bot.", { status: 409 }),
      ),
    ).toBe(true);
    expect(
      isCodexAuthRequired(new CloudApiError("Not authenticated", { status: 401 })),
    ).toBe(true);
    expect(isCodexAuthRequired(new Error("Could not create a computer"))).toBe(false);
  });
});

describe("quick start draft", () => {
  it("returns the stored task for the matching Bot and clears it", () => {
    rememberQuickStartDraft("bot_1", "Write the brief");
    expect(consumeQuickStartDraft("bot_1")).toBe("Write the brief");
    expect(consumeQuickStartDraft("bot_1")).toBeNull();
  });

  it("leaves a draft for a different Bot untouched", () => {
    rememberQuickStartDraft("bot_1", "Write the brief");
    expect(consumeQuickStartDraft("bot_2")).toBeNull();
    expect(consumeQuickStartDraft("bot_1")).toBe("Write the brief");
  });
});

describe("startFirstBotWork", () => {
  it("reuses an existing cloud computer, creates a Bot with defaults, and starts a run", async () => {
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, [computer({ id: "comp_existing" })]);
      }
      if (path === "/v1/bots") {
        const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
        expect(body).toMatchObject({
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          computerId: "comp_existing",
          model: DEFAULT_BOT_MODEL_ID,
          enginePreference: DEFAULT_BOT_ENGINE,
          avatarId: DEFAULT_BOT_AVATAR_ID,
        });
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_existing",
          enginePreference: DEFAULT_BOT_ENGINE,
        });
      }
      if (path === "/v1/runs") {
        expect(init?.headers).toMatchObject({ "Idempotency-Key": expect.any(String) });
        expect(JSON.parse(String(init?.body))).toEqual({
          botId: "bot_1",
          message: "Write a competitor brief",
        });
        return jsonResponse(202, {
          runId: "run_1",
          requestId: "req_1",
          conversationId: "conv_1",
          computerId: "comp_existing",
          model: DEFAULT_BOT_MODEL_ID,
          status: "queued",
        });
      }
      throw new Error(`unexpected ${requestPath(path)}`);
    });

    const session: QuickStartSession = {};
    const outcome = await startFirstBotWork(
      { name: "Scout", task: "Write a competitor brief" },
      session,
    );
    expect(outcome).toEqual({
      status: "started",
      botId: "bot_1",
      computerId: "comp_existing",
      runId: "run_1",
      conversationId: "conv_1",
    });
    expect(session.botId).toBe("bot_1");
    expect(session.computerId).toBe("comp_existing");
    expect(fetchMock.mock.calls.some(([path, init]) => path === "/v1/computers" && init?.method === "POST")).toBe(
      false,
    );
  });

  it("creates the default cloud computer when none is usable", async () => {
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, [
          computer({
            id: "mac_offline",
            provider: "local_mac",
            providerMetadata: { provisioned: true, connected: false },
          }),
        ]);
      }
      if (path === "/v1/computers" && init?.method === "POST") {
        expect(JSON.parse(String(init.body))).toEqual({
          displayName: DEFAULT_CLOUD_COMPUTER_NAME,
        });
        return jsonResponse(200, computer({ id: "comp_new" }));
      }
      if (path === "/v1/bots") {
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_new",
          enginePreference: DEFAULT_BOT_ENGINE,
        });
      }
      if (path === "/v1/runs") {
        return jsonResponse(202, {
          runId: "run_1",
          requestId: "req_1",
          conversationId: "conv_1",
          computerId: "comp_new",
          model: DEFAULT_BOT_MODEL_ID,
          status: "queued",
        });
      }
      throw new Error(`unexpected ${path}`);
    });

    const outcome = await startFirstBotWork({ name: "Scout", task: "Look around" });
    expect(outcome.status).toBe("started");
    if (outcome.status === "started") {
      expect(outcome.computerId).toBe("comp_new");
    }
  });

  it("does not create a second computer or Bot when a recoverable step is retried", async () => {
    let computerCreates = 0;
    let botCreates = 0;
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, []);
      }
      if (path === "/v1/computers" && init?.method === "POST") {
        computerCreates += 1;
        return jsonResponse(200, computer({ id: "comp_new" }));
      }
      if (path === "/v1/bots") {
        botCreates += 1;
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_new",
          enginePreference: DEFAULT_BOT_ENGINE,
        });
      }
      if (path === "/v1/runs") {
        return jsonResponse(503, { error: "Runner is restarting" });
      }
      throw new Error(`unexpected ${path}`);
    });

    const session: QuickStartSession = {};
    const first = await startFirstBotWork({ name: "Scout", task: "Write a brief" }, session);
    expect(first.status).toBe("bot_ready_run_failed");
    const second = await startFirstBotWork({ name: "Scout", task: "Write a brief" }, session);
    expect(second.status).toBe("bot_ready_run_failed");
    expect(computerCreates).toBe(1);
    expect(botCreates).toBe(1);
    expect(session.idempotencyKey).toBeTruthy();
    const runCalls = fetchMock.mock.calls.filter(([path]) => path === "/v1/runs");
    expect(runCalls).toHaveLength(2);
    expect(runCalls[0]?.[1]?.headers).toEqual(runCalls[1]?.[1]?.headers);
  });

  it("keeps the pending draft on Codex auth errors instead of treating the Bot as ready", async () => {
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, [computer({ id: "comp_1" })]);
      }
      if (path === "/v1/bots") {
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_1",
          enginePreference: DEFAULT_BOT_ENGINE,
        });
      }
      if (path === "/v1/runs") {
        return jsonResponse(409, { error: "Connect ChatGPT to run this Bot." });
      }
      throw new Error(`unexpected ${path}`);
    });

    const outcome = await startFirstBotWork({ name: "Scout", task: "Write a brief" });
    expect(outcome).toMatchObject({
      status: "needs_codex",
      botId: "bot_1",
      computerId: "comp_1",
    });
  });

  it("preserves created resources when computer or Bot creation fails", async () => {
    fetchMock.mockImplementation(async (path: string) => {
      if (path === "/v1/computers") {
        return jsonResponse(500, { error: "Could not load computers" });
      }
      throw new Error(`unexpected ${path}`);
    });
    const failed = await startFirstBotWork({ name: "Scout", task: "Write a brief" });
    expect(failed.status).toBe("failed");
    if (failed.status === "failed") {
      expect(failed.error).toContain("Could not load computers");
      expect(failed.botId).toBeUndefined();
    }
  });
});
