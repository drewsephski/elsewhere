// @vitest-environment happy-dom

import type { ComputerSummary } from "@/lib/api-types";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkspaceQuickStart } from "./workspace-quick-start";
import { consumeQuickStartDraft, DEFAULT_BOT_INSTRUCTIONS } from "@/lib/bot-quick-start";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";

const fetchMock = vi.fn();
const push = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push,
    replace: vi.fn(),
    refresh: vi.fn(),
  }),
}));

const providerState = {
  status: {
    chatgptConnected: true,
    chatgptConnectionState: "connected" as const,
    codexInstalled: true,
    connectionDetail: null,
    chatgptPlanType: "Plus",
    preferredEngine: "codex",
    apiFallbackConfigured: false,
    defaultModel: DEFAULT_BOT_MODEL_ID,
    codexLoginAllowed: true,
  },
  challenge: null as { loginId: string; authUrl: string; userCode: string } | null,
  busy: false,
  error: null as string | null,
  connected: true,
  checking: false,
  checkFailed: false,
  providerUnavailable: false,
  canConnect: false,
  connectBlocked: false,
  load: vi.fn(),
  handleConnect: vi.fn(),
  cancelLogin: vi.fn(),
};

vi.mock("@/hooks/use-provider-status", () => ({
  useProviderStatus: () => providerState,
}));

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function existingComputer(): ComputerSummary {
  return {
    id: "comp_1",
    displayName: "Workspace",
    provider: "fly_sprite",
    state: "pending",
    lastUsedAt: null,
    providerMetadata: { provisioned: false },
  };
}

function mockHappyPath() {
  fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
    if (path === "/v1/computers" && init?.method !== "POST") {
      return jsonResponse(200, [existingComputer()]);
    }
    if (path === "/v1/bots") {
      return jsonResponse(200, {
        id: "bot_1",
        name: "Scout",
        instructions: DEFAULT_BOT_INSTRUCTIONS,
        model: DEFAULT_BOT_MODEL_ID,
        computerId: "comp_1",
        enginePreference: "codex",
      });
    }
    if (path === "/v1/runs") {
      return jsonResponse(202, {
        runId: "run_1",
        requestId: "req_1",
        conversationId: "conv_1",
        computerId: "comp_1",
        model: DEFAULT_BOT_MODEL_ID,
        status: "queued",
      });
    }
    throw new Error(`unexpected ${path}`);
  });
}

function fillAndSubmit() {
  fireEvent.change(screen.getByLabelText("Bot name"), {
    target: { value: "Scout" },
  });
  fireEvent.change(screen.getByLabelText("What should this Bot work on?"), {
    target: { value: "Write a competitor brief" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Start working" }));
}

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
  push.mockReset();
  sessionStorage.clear();
  providerState.connected = true;
  providerState.checking = false;
  providerState.checkFailed = false;
  providerState.providerUnavailable = false;
  providerState.challenge = null;
  providerState.status = {
    ...providerState.status,
    chatgptConnected: true,
    chatgptConnectionState: "connected",
  };
});

describe("WorkspaceQuickStart", () => {
  it("shows name, first-task composer, and Start working without infrastructure fields", () => {
    render(
      <MemoryRouter>
        <WorkspaceQuickStart />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText("Bot name")).toBeTruthy();
    expect(screen.getByLabelText("What should this Bot work on?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start working" })).toBeTruthy();
    expect(screen.queryByLabelText("Model")).toBeNull();
    expect(screen.queryByLabelText("Computer")).toBeNull();
    expect(screen.queryByLabelText("Role and instructions")).toBeNull();
    expect(screen.getByText(/Uses your Codex allowance/)).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Connect ChatGPT" })).toBeNull();
  });

  it("keeps the Quick Start form when ChatGPT is disconnected", () => {
    providerState.connected = false;
    providerState.status = {
      ...providerState.status,
      chatgptConnected: false,
      chatgptConnectionState: "connected" as const,
    };
    providerState.canConnect = true;

    render(
      <MemoryRouter>
        <WorkspaceQuickStart />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Connect ChatGPT" })).toBeTruthy();
    expect(screen.getByLabelText("Bot name")).toBeTruthy();
    expect(screen.getByLabelText("What should this Bot work on?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start working" })).toHaveProperty(
      "disabled",
      true,
    );
  });

  it("starts work through the existing Bot/run APIs and navigates to the conversation", async () => {
    mockHappyPath();
    const onStarted = vi.fn();
    render(
      <MemoryRouter>
        <WorkspaceQuickStart onStarted={onStarted} />
      </MemoryRouter>,
    );

    fillAndSubmit();

    await waitFor(() => {
      expect(push).toHaveBeenCalledWith("/app/bots/bot_1");
    });
    expect(onStarted).toHaveBeenCalled();
    const botBody = JSON.parse(
      String(fetchMock.mock.calls.find(([path]) => path === "/v1/bots")?.[1]?.body),
    );
    expect(botBody.model).toBe(DEFAULT_BOT_MODEL_ID);
    expect(botBody.enginePreference).toBe("codex");
    expect(botBody.computerId).toBe("comp_1");
    expect(
      fetchMock.mock.calls.some(([path]) => path === "/v1/runs"),
    ).toBe(true);
  });

  it("preserves the draft and surfaces an error when setup fails", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse(500, { error: "Could not load computers" }),
    );
    render(
      <MemoryRouter>
        <WorkspaceQuickStart />
      </MemoryRouter>,
    );

    fillAndSubmit();

    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain("Could not load computers");
    });
    expect(push).not.toHaveBeenCalled();
    expect((screen.getByLabelText("Bot name") as HTMLInputElement).value).toBe(
      "Scout",
    );
    expect(
      (screen.getByLabelText("What should this Bot work on?") as HTMLTextAreaElement)
        .value,
    ).toBe("Write a competitor brief");
  });

  it("navigates to the created Bot and leaves the task recoverable if the run fails", async () => {
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, [existingComputer()]);
      }
      if (path === "/v1/bots") {
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_1",
          enginePreference: "codex",
        });
      }
      if (path === "/v1/runs") {
        return jsonResponse(503, { error: "Runner is restarting" });
      }
      throw new Error(`unexpected ${path}`);
    });

    render(
      <MemoryRouter>
        <WorkspaceQuickStart />
      </MemoryRouter>,
    );

    fillAndSubmit();

    await waitFor(() => {
      expect(push).toHaveBeenCalledWith("/app/bots/bot_1");
    });
    expect(consumeQuickStartDraft("bot_1")).toBe("Write a competitor brief");
  });

  it("blocks duplicate submits while work is starting", async () => {
    let finishRun: ((value: Response) => void) | undefined;
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, [existingComputer()]);
      }
      if (path === "/v1/bots") {
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          instructions: DEFAULT_BOT_INSTRUCTIONS,
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_1",
          enginePreference: "codex",
        });
      }
      if (path === "/v1/runs") {
        return await new Promise<Response>((resolve) => {
          finishRun = resolve;
        });
      }
      throw new Error(`unexpected ${path}`);
    });

    render(
      <MemoryRouter>
        <WorkspaceQuickStart />
      </MemoryRouter>,
    );

    fillAndSubmit();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Starting|Setting up|Creating/ })).toHaveProperty(
        "disabled",
        true,
      );
    });
    fireEvent.click(screen.getByRole("button", { name: /Starting|Setting up|Creating/ }));
    expect(fetchMock.mock.calls.filter(([path]) => path === "/v1/bots")).toHaveLength(1);

    finishRun?.(
      jsonResponse(202, {
        runId: "run_1",
        requestId: "req_1",
        conversationId: "conv_1",
        computerId: "comp_1",
        model: DEFAULT_BOT_MODEL_ID,
        status: "queued",
      }),
    );
    await waitFor(() => {
      expect(push).toHaveBeenCalledWith("/app/bots/bot_1");
    });
  });
});
