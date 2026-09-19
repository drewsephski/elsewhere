// @vitest-environment happy-dom

import type { GroupListItem } from "@/lib/api-types";
import type { WorkspaceBotPresence } from "@/lib/workspace-types";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BotListSidebar } from "./bot-list-sidebar";

vi.mock("@/components/product-logo", () => ({
  ProductLogo: () => <span data-testid="product-logo" />,
}));

const bot: WorkspaceBotPresence = {
  id: "bot_1",
  name: "Scout",
  computerName: null,
  presence: "ready",
  workId: null,
  task: null,
};

const group: GroupListItem = {
  id: "group_1",
  name: "Launch crew",
  updatedAt: "2026-01-01T00:00:00.000Z",
  queuedRuns: 0,
  workingRuns: 0,
  participants: [
    {
      botId: "bot_1",
      name: "Scout",
      avatarId: "atlas",
      ordinal: 0,
      joinedAt: "2026-01-01T00:00:00.000Z",
    },
  ],
};

function renderSidebar(overrides?: {
  onRenameBot?: (botId: string, name: string) => Promise<void>;
  onRenameGroup?: (groupId: string, name: string) => Promise<void>;
}) {
  const onRenameBot = overrides?.onRenameBot ?? vi.fn(async () => undefined);
  const onRenameGroup = overrides?.onRenameGroup ?? vi.fn(async () => undefined);
  render(
    <MemoryRouter>
      <BotListSidebar
        bots={[bot]}
        groups={[group]}
        selectedBotId={bot.id}
        selectedGroupId={null}
        runActivityAt={{}}
        onCreateBot={vi.fn()}
        onCreateGroup={vi.fn()}
        onRenameBot={onRenameBot}
        onRenameGroup={onRenameGroup}
        footer={null}
      />
    </MemoryRouter>,
  );
  return { onRenameBot, onRenameGroup };
}

afterEach(() => {
  cleanup();
});

describe("BotListSidebar", () => {
  it("does not start renaming a bot or group from a single click on the name", () => {
    renderSidebar();

    fireEvent.click(screen.getByLabelText("Rename Scout"));
    fireEvent.click(screen.getByLabelText("Rename Launch crew"));

    expect(screen.queryByRole("textbox", { name: "Rename Scout" })).toBeNull();
    expect(screen.queryByRole("textbox", { name: "Rename Launch crew" })).toBeNull();
  });

  it("starts renaming a bot on double-click and commits the new name", async () => {
    const { onRenameBot } = renderSidebar();

    fireEvent.doubleClick(screen.getByLabelText("Rename Scout"));
    const input = screen.getByRole("textbox", { name: "Rename Scout" });
    fireEvent.change(input, { target: { value: "Scout v2" } });
    fireEvent.blur(input);

    await waitFor(() => {
      expect(onRenameBot).toHaveBeenCalledWith("bot_1", "Scout v2");
    });
  });

  it("starts renaming a group on double-click and commits the new name", async () => {
    const { onRenameGroup } = renderSidebar();

    fireEvent.doubleClick(screen.getByLabelText("Rename Launch crew"));
    const input = screen.getByRole("textbox", { name: "Rename Launch crew" });
    fireEvent.change(input, { target: { value: "Launch desk" } });
    fireEvent.blur(input);

    await waitFor(() => {
      expect(onRenameGroup).toHaveBeenCalledWith("group_1", "Launch desk");
    });
  });

  it("places the sliding selection highlight on the selected bot", () => {
    renderSidebar();
    const selected = screen.getByRole("link", { current: "page" });
    expect(selected.getAttribute("href")).toBe("/app/bots/bot_1");
    expect(selected.querySelector("[data-sidebar-selection]")).toBeTruthy();
    expect(
      screen.getByRole("link", { name: /Launch crew/ }).querySelector("[data-sidebar-selection]"),
    ).toBeNull();
  });

  it("animates a working bot that is not selected", () => {
    const idle: WorkspaceBotPresence = { ...bot, id: "bot_idle", name: "Idle" };
    const working: WorkspaceBotPresence = {
      ...bot,
      id: "bot_working",
      name: "Worker",
      presence: "working",
    };

    render(
      <MemoryRouter>
        <BotListSidebar
          bots={[idle, working]}
          groups={[]}
          selectedBotId={idle.id}
          selectedGroupId={null}
          runActivityAt={{}}
          onCreateBot={vi.fn()}
          footer={null}
        />
      </MemoryRouter>,
    );

    const workingAvatars = screen.getAllByRole("img", { name: "Worker avatar" });
    expect(workingAvatars.length).toBeGreaterThan(0);
    for (const avatar of workingAvatars) {
      expect(avatar.getAttribute("data-working")).toBe("on");
    }

    for (const avatar of screen.getAllByRole("img", { name: "Idle avatar" })) {
      expect(avatar.getAttribute("data-working")).toBe("off");
    }
  });
});
