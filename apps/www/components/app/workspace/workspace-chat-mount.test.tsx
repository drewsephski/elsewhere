// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WorkspaceAuthenticatedFrame } from "./workspace-authenticated-frame";
import BotWorkspacePage from "@/app/app/bots/[id]/page";

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

vi.mock("@/hooks/use-workspace-overview", () => ({
  useWorkspaceOverview: () => ({
    data: {
      bots: [bot],
      runnerReady: true,
    },
    error: null,
    phase: "ready",
    refresh: vi.fn(),
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
  });
});
