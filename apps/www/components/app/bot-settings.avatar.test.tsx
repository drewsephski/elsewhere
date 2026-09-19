// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotGeneralSettings } from "./bot-settings";

const fetchMock = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    refresh: vi.fn(),
  }),
}));

const bot: BotSummary = {
  id: "bot_1",
  name: "Launch",
  instructions: "Own the launch checklist and keep it current.",
  model: "gpt-5.6-luna",
  computerId: "comp_1",
  enginePreference: "codex",
  avatarId: "berry-puff",
};

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("BotGeneralSettings avatar", () => {
  it("saves a new avatar without replacing a custom name or instructions", async () => {
    fetchMock.mockImplementation(async (path: string) => {
      if (typeof path === "string" && path.endsWith("/onboarding")) {
        return new Response(JSON.stringify({ status: "completed" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(
        JSON.stringify({
          ...bot,
          avatarId: "amber-puff",
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    });
    const onSaved = vi.fn();
    render(<BotGeneralSettings bot={bot} onSaved={onSaved} />);

    fireEvent.click(screen.getByRole("radio", { name: /Researcher/i }));
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalled();
    });
    const patch = fetchMock.mock.calls.find(([path]) => path === "/v1/bots/bot_1");
    expect(patch).toBeTruthy();
    const body = JSON.parse(String((patch?.[1] as RequestInit | undefined)?.body));
    expect(body).toMatchObject({
      name: "Launch",
      instructions: "Own the launch checklist and keep it current.",
      avatarId: "amber-puff",
    });
    expect(screen.getByLabelText("Name")).toHaveProperty("value", "Launch");
    expect(screen.getByLabelText("Role and instructions")).toHaveProperty(
      "value",
      "Own the launch checklist and keep it current.",
    );
  });
});
