// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProviderStatusCard } from "./provider-status-card";

const providerState = vi.hoisted(() => ({
  status: {
    chatgptConnected: false,
    chatgptConnectionState: "not_connected" as const,
    codexInstalled: true,
    connectionDetail: null,
    chatgptPlanType: null,
    preferredEngine: "codex",
    apiFallbackConfigured: false,
    defaultModel: "gpt-5.6-luna",
    codexLoginAllowed: false,
  },
  challenge: null,
  busy: false,
  error: null,
  connected: false,
  checking: false,
  checkFailed: false,
  providerUnavailable: false,
  canConnect: false,
  connectBlocked: true,
  load: vi.fn(),
  handleConnect: vi.fn(),
  cancelLogin: vi.fn(),
}));

vi.mock("@/hooks/use-provider-status", () => ({
  useProviderStatus: () => providerState,
}));

afterEach(() => {
  cleanup();
});

describe("ProviderStatusCard", () => {
  it("uses product copy when ChatGPT sign-in is disabled", () => {
    render(<ProviderStatusCard />);
    expect(
      screen.getByText("ChatGPT sign-in is not enabled on this Elsewhere deployment."),
    ).toBeTruthy();
    expect(screen.queryByText(/ELSEWHERE_ALLOW_CODEX_LOGIN/)).toBeNull();
  });
});
