// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ConnectorNeedCard } from "./connector-need-card";
import type { ConnectorNeed } from "@/lib/connector-need";
import { botChatReturnTo } from "@/lib/connector-need";

const startGithubConnectorOAuth = vi.fn();

vi.mock("@/lib/github-oauth", () => ({
  startGithubConnectorOAuth: (...args: unknown[]) => startGithubConnectorOAuth(...args),
}));

const returnTo = botChatReturnTo({ botId: "bot_1", conversationId: "c1" });

function need(overrides: Partial<ConnectorNeed> = {}): ConnectorNeed {
  return {
    needId: "cneed_1",
    provider: "github",
    reason: { kind: "disconnected" },
    runId: "run_1",
    botId: "bot_1",
    toolName: "github_list_repositories",
    requestedAt: "2026-09-19T17:00:00.000Z",
    status: { phase: "pending" },
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
  startGithubConnectorOAuth.mockReset();
});

describe("ConnectorNeedCard", () => {
  it("shows a Connect CTA while pending", () => {
    render(<ConnectorNeedCard need={need()} returnTo={returnTo} />);
    expect(screen.getByText("Connect GitHub?")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Connect GitHub/ })).toBeTruthy();
    expect(screen.getByText("Your bot continues after you connect.")).toBeTruthy();
  });

  it("hides Connect on a resolved card", () => {
    render(
      <ConnectorNeedCard
        need={need({ status: { phase: "resolved", resolution: "connected" } })}
        returnTo={returnTo}
      />,
    );
    expect(screen.queryByRole("button", { name: /Connect GitHub/ })).toBeNull();
  });

  it("uses distinct copy for reconnect versus unauthorized repo", () => {
    const { rerender } = render(
      <ConnectorNeedCard
        need={need({ reason: { kind: "reconnect_required" } })}
        returnTo={returnTo}
      />,
    );
    expect(screen.getByText("Reconnect GitHub?")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Reconnect GitHub/ })).toBeTruthy();

    rerender(
      <ConnectorNeedCard
        need={need({
          reason: { kind: "unauthorized_repo", owner: "acme", repo: "elsewhere" },
        })}
        returnTo={returnTo}
      />,
    );
    expect(screen.getByText("Add this repository?")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Add acme on GitHub/ })).toBeTruthy();
  });

  it("starts OAuth with returnTo", () => {
    startGithubConnectorOAuth.mockResolvedValue({
      authorizeUrl: "https://github.com/apps/elsewhere/installations/new",
      state: "s",
      expiresAt: "2026-09-19T17:10:00.000Z",
    });
    render(<ConnectorNeedCard need={need()} returnTo={returnTo} />);
    fireEvent.click(screen.getByRole("button", { name: /Connect GitHub/ }));
    expect(startGithubConnectorOAuth).toHaveBeenCalledWith({ returnTo });
  });
});
