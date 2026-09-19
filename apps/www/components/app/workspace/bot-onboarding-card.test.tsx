// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotOnboardingCard } from "./bot-onboarding-card";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";

const fetchMock = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function notStarted() {
  return {
    botId: "bot_1",
    status: "not_started",
    questionsAsked: 0,
    maxQuestions: 3,
    revision: 0,
    answers: [],
    generationModel: DEFAULT_BOT_MODEL_ID,
  };
}

function questionState() {
  return {
    botId: "bot_1",
    status: "in_progress",
    questionsAsked: 0,
    maxQuestions: 3,
    revision: 2,
    generationModel: DEFAULT_BOT_MODEL_ID,
    answers: [],
    currentQuestion: {
      id: "scope",
      prompt: "What should Scout own?",
      options: [
        { id: "research", label: "Research briefs" },
        { id: "ops", label: "Launch ops" },
        { id: "other", label: "Something else…" },
      ],
      allowCustom: true,
      index: 1,
      maxQuestions: 3,
    },
  };
}

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
});

describe("BotOnboardingCard", () => {
  it("offers optional setup on an empty newly created Bot", async () => {
    fetchMock.mockResolvedValue(jsonResponse(200, notStarted()));
    render(
      <BotOnboardingCard
        botId="bot_1"
        botName="Scout"
        conversationEmpty
      />,
    );
    expect(await screen.findByText(/Scout is ready/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Skip for now" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tune this Bot" })).toBeTruthy();
  });

  it("answers a question including a custom Other value", async () => {
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (String(path).endsWith("/onboarding") && init?.method !== "POST") {
        return jsonResponse(200, questionState());
      }
      if (String(path).endsWith("/answer")) {
        expect(JSON.parse(String(init?.body))).toMatchObject({
          questionId: "scope",
          customText: "Own weekly launch notes",
        });
        return jsonResponse(200, {
          ...questionState(),
          status: "ready_to_apply",
          questionsAsked: 1,
          currentQuestion: null,
          draft: {
            summary: "Scout owns launch notes.",
            owns: "Weekly launch notes",
            workingStyle: "Draft quietly, then check before sending.",
            instructions: "Write weekly launch notes.",
            context: "Audience is the founding team.",
            suggestedFirstTasks: ["Draft this week's note"],
            suggestedCapabilities: [],
          },
        });
      }
      throw new Error(`unexpected ${path}`);
    });

    render(
      <BotOnboardingCard botId="bot_1" botName="Scout" conversationEmpty autoStart={false} />,
    );
    fireEvent.click(await screen.findByRole("radio", { name: "Something else…" }));
    fireEvent.change(screen.getByLabelText("Custom answer"), {
      target: { value: "Own weekly launch notes" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Apply setup" })).toBeTruthy();
    });
    expect(screen.getByText("What Scout owns")).toBeTruthy();
    expect(screen.getByText("How Scout should work")).toBeTruthy();
    expect(screen.getByText("Context Scout should remember")).toBeTruthy();
    expect(screen.getByText("Skip for now")).toBeTruthy();
  });

  it("shows a quieter finish-setup affordance after real work", async () => {
    fetchMock.mockResolvedValue(jsonResponse(200, notStarted()));
    render(
      <BotOnboardingCard
        botId="bot_1"
        botName="Scout"
        conversationEmpty={false}
      />,
    );
    expect(await screen.findByRole("button", { name: "Finish setting up Scout" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Tune this Bot" })).toBeNull();
  });

  it("hides empty-work prompts until the user skips setup", async () => {
    fetchMock.mockImplementation(async (path: string, init?: RequestInit) => {
      if (String(path).endsWith("/onboarding") && init?.method !== "POST") {
        return jsonResponse(200, notStarted());
      }
      if (String(path).endsWith("/dismiss")) {
        return jsonResponse(200, { ...notStarted(), status: "dismissed", revision: 1 });
      }
      throw new Error(`unexpected ${path}`);
    });

    render(
      <BotOnboardingCard botId="bot_1" botName="Scout" conversationEmpty>
        <p>What should Scout work on?</p>
      </BotOnboardingCard>,
    );

    expect(await screen.findByText(/Scout is ready/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Tune this Bot" })).toBeTruthy();
    expect(screen.queryByText("What should Scout work on?")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Skip for now" }));
    await waitFor(() => {
      expect(screen.getByText("What should Scout work on?")).toBeTruthy();
    });
    expect(screen.queryByText(/Scout is ready/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Tune this Bot" })).toBeNull();
  });

  it("does not overlay setup when work is running", async () => {
    fetchMock.mockResolvedValue(jsonResponse(200, questionState()));
    render(
      <BotOnboardingCard
        botId="bot_1"
        botName="Scout"
        conversationEmpty
        workTakesPriority
      />,
    );
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalled();
    });
    expect(screen.queryByText("What should Scout own?")).toBeNull();
  });
});
