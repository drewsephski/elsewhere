// @vitest-environment happy-dom

import type { GroupConversationDetail, TranscriptMessage } from "@/lib/api-types";
import type { WorkspaceBotPresence } from "@/lib/workspace-types";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { GroupConversationView } from "./group-conversation-view";

const fetchMock = vi.fn();
const toastError = vi.fn();

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: (...args: unknown[]) => fetchMock(...args),
}));

vi.mock("sonner", () => ({
  toast: { error: (...args: unknown[]) => toastError(...args) },
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

const group: GroupConversationDetail = {
  id: "group_1",
  name: "Launch crew",
  conversationType: "group",
  updatedAt: "2026-01-01T00:00:00.000Z",
  participants: [
    {
      botId: "bot_1",
      name: "Scout",
      avatarId: "berry-puff",
      ordinal: 0,
      joinedAt: "2026-01-01T00:00:00.000Z",
    },
    {
      botId: "bot_2",
      name: "Writer",
      avatarId: "rose-sprout",
      ordinal: 1,
      joinedAt: "2026-01-01T00:00:00.000Z",
    },
  ],
};

const message: TranscriptMessage = {
  id: "msg_1",
  sequence: 1,
  role: "user",
  status: "complete",
  authorKind: "human",
  body: "Hello",
  createdAt: "2026-01-01T00:00:00.000Z",
  routing: { mode: "auto", status: "failed" },
};

const extraBot: WorkspaceBotPresence = {
  id: "bot_3",
  name: "Analyst",
  computerName: null,
  presence: "ready",
  workId: null,
  task: null,
};

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

afterEach(() => {
  cleanup();
  fetchMock.mockReset();
  toastError.mockReset();
});

describe("GroupConversationView mutations", () => {
  it("toasts a network rejection when adding a participant and re-enables the control", async () => {
    fetchMock.mockImplementation(async (path: string) => {
      if (path === "/v1/conversations/group_1") {
        return jsonResponse(200, group);
      }
      if (path === "/v1/conversations/group_1/messages") {
        return jsonResponse(200, []);
      }
      if (path === "/v1/conversations/group_1/participants") {
        throw new Error("network down");
      }
      throw new Error(`unexpected ${path}`);
    });

    render(
      <GroupConversationView
        groupId="group_1"
        bots={[
          {
            id: "bot_1",
            name: "Scout",
            computerName: null,
            presence: "ready",
            workId: null,
            task: null,
          },
          extraBot,
        ]}
      />,
    );

    await screen.findByText("Launch crew");
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Analyst/ }));

    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith("Network down");
    });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("menuitem", { name: /Analyst/ })).not.toHaveProperty(
      "disabled",
      true,
    );
  });

  it("toasts a network rejection when retrying routing", async () => {
    fetchMock.mockImplementation(async (path: string) => {
      if (path === "/v1/conversations/group_1") {
        return jsonResponse(200, group);
      }
      if (path === "/v1/conversations/group_1/messages") {
        return jsonResponse(200, [message]);
      }
      if (path.endsWith("/route/retry")) {
        throw new Error("network down");
      }
      throw new Error(`unexpected ${path}`);
    });

    render(
      <GroupConversationView
        groupId="group_1"
        bots={[
          {
            id: "bot_1",
            name: "Scout",
            computerName: null,
            presence: "ready",
            workId: null,
            task: null,
          },
        ]}
      />,
    );

    const retry = await screen.findByRole("button", { name: "Retry" });
    fireEvent.click(retry);
    await waitFor(() => {
      expect(toastError).toHaveBeenCalledWith("Network down");
    });
    expect(screen.getByRole("button", { name: "Retry" })).not.toHaveProperty("disabled", true);
  });
});
