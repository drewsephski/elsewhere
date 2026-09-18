// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { rememberQuickStartDraft } from "@/lib/bot-quick-start";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkspaceAuthenticatedFrame } from "./workspace-authenticated-frame";
import BotWorkspacePage from "@/app/app/bots/[id]/page";
import AppHomePage from "@/app/app/page";

vi.mock("@/components/app/product-theme-scope", () => ({
  ProductThemeScope: () => null,
}));

vi.mock("@/components/ui/sonner", () => ({
  Toaster: () => null,
}));

vi.mock("./floating-browser-preview", () => ({
  FloatingBrowserPreview: () => null,
}));

const bot: BotSummary = {
  id: "bot_1",
  name: "Scout",
  instructions: "Helpful assistant",
  model: "codex",
  computerId: null,
  enginePreference: "codex",
};

const overviewState = vi.hoisted(() => ({
  data: {
    bots: [
      {
        id: "bot_1",
        name: "Scout",
        computerName: null,
        presence: "ready",
        workId: null,
        task: null,
      },
    ],
    runnerReady: true,
  },
  error: null as string | null,
  phase: "ready" as const,
  refresh: vi.fn(),
}));

vi.mock("@/hooks/use-workspace-overview", () => ({
  useWorkspaceOverview: () => overviewState,
}));

vi.mock("@/hooks/use-provider-status", () => ({
  useProviderStatus: () => ({
    status: {
      chatgptConnected: true,
      chatgptConnectionState: "connected",
      codexInstalled: true,
      connectionDetail: null,
      chatgptPlanType: "Plus",
      preferredEngine: "codex",
      apiFallbackConfigured: false,
      defaultModel: "gpt-5.6-luna",
      codexLoginAllowed: true,
    },
    challenge: null,
    busy: false,
    error: null,
    connected: true,
    checking: false,
    checkFailed: false,
    providerUnavailable: false,
    canConnect: false,
    connectBlocked: false,
    load: vi.fn(),
    handleConnect: vi.fn(),
    cancelLogin: vi.fn(),
  }),
}));

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(async (path: string) => {
    if (path.startsWith("/v1/bots/")) {
      return {
        ok: true,
        json: async () => bot,
      };
    }
    if (path.includes("/conversations")) {
      return {
        ok: true,
        json: async () => ({ conversationId: "conv_1", runs: [] }),
      };
    }
    if (path === "/v1/conversations/groups") {
      return { ok: true, json: async () => [] };
    }
    if (path === "/v1/runs?limit=50") {
      return { ok: true, json: async () => [] };
    }
    return { ok: true, json: async () => ({}) };
  }),
}));

vi.mock("@/contexts/active-run-context", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/contexts/active-run-context")>();
  return {
    ...actual,
    useActiveRun: () => ({
      connection: null,
      timeline: [],
      assistantStream: { answerText: "", commentaryText: "", streaming: false },
      pendingHumanIntervention: null,
      browserPreviewGeneration: 0,
      reset: vi.fn(),
    }),
  };
});

afterEach(() => {
  cleanup();
  sessionStorage.clear();
  overviewState.data = {
    bots: [
      {
        id: "bot_1",
        name: "Scout",
        computerName: null,
        presence: "ready",
        workId: null,
        task: null,
      },
    ],
    runnerReady: true,
  };
  overviewState.phase = "ready";
});

describe("workspace chat mount", () => {
  it("mounts BotConversationView via WorkspaceShell when the bot page outlet is empty", async () => {
    render(
      <MemoryRouter initialEntries={["/app/bots/bot_1"]}>
        <Routes>
          <Route
            path="/app/bots/:id"
            element={
              <WorkspaceAuthenticatedFrame userEmail="user@example.com">
                <BotWorkspacePage />
              </WorkspaceAuthenticatedFrame>
            }
          />
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Message" })).toBeTruthy();
    });
    expect(screen.getByRole("button", { name: "Collapse sidebar" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Hide details" })).toBeNull();
    expect(await screen.findByLabelText("Model")).toBeTruthy();
    const starter = await screen.findByRole("button", { name: "What can you take on?" });
    fireEvent.click(starter);
    expect((screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value).toContain(
      "Look at your instructions and this computer",
    );
  });

  it("restores a recoverable first-task draft in the conversation composer", async () => {
    rememberQuickStartDraft("bot_1", "Write a competitor brief");
    render(
      <MemoryRouter initialEntries={["/app/bots/bot_1"]}>
        <Routes>
          <Route
            path="/app/bots/:id"
            element={
              <WorkspaceAuthenticatedFrame userEmail="user@example.com">
                <BotWorkspacePage />
              </WorkspaceAuthenticatedFrame>
            }
          />
        </Routes>
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(
        (screen.getByRole("textbox", { name: "Message" }) as HTMLTextAreaElement).value,
      ).toBe("Write a competitor brief");
    });
  });

  it("shows Quick Start on /app when the account has no Bots", async () => {
    overviewState.data = { bots: [], runnerReady: true };
    render(
      <MemoryRouter initialEntries={["/app"]}>
        <Routes>
          <Route
            path="/app"
            element={
              <WorkspaceAuthenticatedFrame userEmail="user@example.com">
                <AppHomePage />
              </WorkspaceAuthenticatedFrame>
            }
          />
        </Routes>
      </MemoryRouter>,
    );

    expect(await screen.findByRole("heading", { name: "Meet your first teammate" })).toBeTruthy();
    expect(screen.getByLabelText("Bot name")).toBeTruthy();
    expect(screen.getByLabelText("What should this Bot work on?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start working" })).toBeTruthy();
    expect(screen.queryByLabelText("Model")).toBeNull();
    expect(screen.queryByLabelText("Computer")).toBeNull();
  });
});
