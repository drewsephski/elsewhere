// @vitest-environment happy-dom

import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CreateBotDialog } from "./create-bot-dialog";
import { botConversationHref } from "@/lib/bot-onboarding";

const push = vi.fn();
const onOutcome = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push,
    replace: vi.fn(),
    refresh: vi.fn(),
  }),
}));

vi.mock("./create-bot-form", () => ({
  CreateBotForm: ({
    onOutcome: handleOutcome,
  }: {
    onOutcome: (outcome: {
      status: string;
      botId: string;
      computerId?: string;
      error?: string;
      task?: string;
    }) => void;
  }) => {
    onOutcome.mockImplementation(handleOutcome);
    return <button type="button" onClick={() => handleOutcome({ status: "created", botId: "bot_1", computerId: "c1" })}>emit-created</button>;
  },
}));

afterEach(() => {
  cleanup();
  push.mockReset();
});

describe("CreateBotDialog", () => {
  it("routes a created Bot into optional setup without starting a run", () => {
    const { getByRole } = render(<CreateBotDialog open onClose={vi.fn()} />);
    fireEvent.click(getByRole("button", { name: "emit-created" }));
    expect(push).toHaveBeenCalledWith(botConversationHref("bot_1", { setup: true }));
  });
});
