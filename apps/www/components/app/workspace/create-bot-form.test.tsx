// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CreateBotForm } from "./create-bot-form";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";

const fetchMock = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

const providerState = {
  connected: true,
  checking: false,
  challenge: null,
  checkFailed: false,
  providerUnavailable: false,
};

vi.mock("@/hooks/use-provider-status", () => ({
  useProviderStatus: () => providerState,
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
  }: {
    children: React.ReactNode;
    href: string;
  }) => <a href={href}>{children}</a>,
}));

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("CreateBotForm", () => {
  it("shows avatars without opening Advanced", () => {
    render(<CreateBotForm onOutcome={vi.fn()} showProviderCard={false} />);
    expect(screen.getByRole("radiogroup", { name: "Choose bot avatar" })).toBeTruthy();
  });

  it("hides first task, model, and computer until Advanced is opened", async () => {
    fetchMock.mockResolvedValue(jsonResponse(200, []));
    render(<CreateBotForm onOutcome={vi.fn()} showProviderCard={false} />);
    expect(screen.queryByLabelText("First task (optional)")).toBeNull();
    expect(screen.queryByLabelText("Model")).toBeNull();
    expect(screen.queryByLabelText("Computer")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));
    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toBeTruthy();
    });
    expect(screen.getByLabelText("First task (optional)")).toBeTruthy();
    expect(screen.getByLabelText("Model")).toBeTruthy();
    expect(screen.getByLabelText("Computer")).toBeTruthy();
  });

  it("keeps a required first task visible without opening Advanced", () => {
    render(
      <CreateBotForm onOutcome={vi.fn()} showProviderCard={false} requireTask />,
    );
    expect(screen.getByLabelText("What should this Bot work on?")).toBeTruthy();
    expect(screen.queryByLabelText("Model")).toBeNull();
    expect(screen.queryByLabelText("Computer")).toBeNull();
  });

  it("creates a bot without a run when no task is provided", async () => {
    const onOutcome = vi.fn();
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (path === "/v1/computers" && init?.method !== "POST") {
        return jsonResponse(200, [
          {
            id: "comp_1",
            displayName: "Workspace",
            provider: "fly_sprite",
            state: "pending",
            lastUsedAt: null,
            providerMetadata: { provisioned: false },
          },
        ]);
      }
      if (path === "/v1/bots") {
        return jsonResponse(200, {
          id: "bot_1",
          name: "Scout",
          model: DEFAULT_BOT_MODEL_ID,
          computerId: "comp_1",
        });
      }
      throw new Error(`unexpected ${path}`);
    });

    render(<CreateBotForm onOutcome={onOutcome} showProviderCard={false} />);
    fireEvent.change(screen.getByLabelText("Bot name"), {
      target: { value: "Scout" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create bot" }));

    await waitFor(() => {
      expect(onOutcome).toHaveBeenCalledWith({
        status: "created",
        botId: "bot_1",
        computerId: "comp_1",
      });
    });
    const botPost = fetchMock.mock.calls.find(([path]) => path === "/v1/bots");
    expect(botPost).toBeTruthy();
    const body = JSON.parse(String((botPost?.[1] as RequestInit | undefined)?.body));
    expect(typeof body.avatarId).toBe("string");
    expect(body.avatarId.length).toBeGreaterThan(0);
    expect(fetchMock.mock.calls.some(([path]) => path === "/v1/runs")).toBe(false);
  });
});
