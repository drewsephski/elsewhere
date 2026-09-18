// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ConnectorsManager } from "./connectors-manager";

const bot: BotSummary = {
  id: "bot_1",
  name: "Scout",
  instructions: "Research",
  model: "codex",
  computerId: "comp_1",
  enginePreference: "codex",
};

vi.mock("@/lib/cloud-api", () => ({
  cloudHostErrorMessage: vi.fn(async () => "error"),
  cloudHostFetch: vi.fn(async (path: string) => {
    if (path === "/v1/connectors/github") {
      return {
        ok: true,
        json: async () => ({
          provider: "github",
          status: "disconnected",
          metadata: {},
          updatedAt: "2026-01-01T00:00:00.000Z",
        }),
      };
    }
    if (path === "/v1/connectors/installs") {
      return {
        ok: true,
        json: async () => [
          {
            id: "install_1",
            kind: "mcp",
            displayName: "Linear MCP",
            endpointUrl: "https://mcp.linear.app",
            status: "connected",
            enabled: true,
            toolCount: 3,
            sampleTools: ["list_issues"],
            createdAt: "2026-01-01T00:00:00.000Z",
            updatedAt: "2026-01-01T00:00:00.000Z",
          },
        ],
      };
    }
    if (path === "/v1/channels") {
      return { ok: true, json: async () => [] };
    }
    if (path === "/v1/bots") {
      return { ok: true, json: async () => [bot] };
    }
    return { ok: false, json: async () => ({}) };
  }),
}));

afterEach(() => {
  cleanup();
});

describe("ConnectorsManager", () => {
  it("renders compact branded cards for GitHub, Slack, MCP, and OpenAPI", async () => {
    render(<ConnectorsManager />);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "GitHub" })).toBeTruthy();
    });

    expect(screen.getByRole("heading", { name: "Slack" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "MCP Server" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "OpenAPI" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Connect Slack" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Linear MCP" })).toBeTruthy();

    const apps = screen.getByRole("heading", { name: "Apps" }).closest("section");
    expect(apps?.querySelector("ul")?.className).toContain("sm:grid-cols-2");
  });
});
