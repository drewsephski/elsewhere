// @vitest-environment happy-dom

import type { BotSummary } from "@/lib/api-types";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SettingsDialog } from "./settings-dialog";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(async (path: string) => {
    if (String(path).includes("permission-policies")) {
      return {
        ok: true,
        json: async () => ({
          actions: [
            {
              action: "workspace_write",
              label: "Write files",
              group: "files",
              groupLabel: "Files",
              decision: "ask",
              source: "default",
              inherited: true,
              inheritedDecision: "ask",
              overridable: true,
            },
          ],
        }),
      };
    }
    return { ok: true, json: async () => [] };
  }),
}));

vi.mock("@/hooks/use-provider-status", () => ({
  useProviderStatus: () => ({
    status: { chatgptPlanType: "Plus" },
    challenge: null,
    busy: false,
    error: null,
    connected: true,
    checking: false,
    checkFailed: false,
    providerUnavailable: false,
    canConnect: true,
    connectBlocked: false,
    load: vi.fn(),
    handleConnect: vi.fn(),
    cancelLogin: vi.fn(),
  }),
}));

const bot: BotSummary = {
  id: "bot_1",
  name: "Researcher",
  instructions: "Research, compare sources, and deliver a concise result.",
  model: "codex",
  computerId: "comp_1",
  enginePreference: "codex",
};

function renderDialog(
  section: "general" | "permissions" | "skills" | "workspace" | "chatgpt" | "advanced" = "general",
  onSectionChange = vi.fn(),
) {
  return render(
    <MemoryRouter>
      <SettingsDialog
        open
        onOpenChange={vi.fn()}
        section={section}
        onSectionChange={onSectionChange}
        bot={bot}
        onBotSaved={vi.fn()}
      />
    </MemoryRouter>,
  );
}

describe("SettingsDialog", () => {
  afterEach(() => {
    cleanup();
  });
  it("opens on General with bot identity fields and an explicit save", async () => {
    renderDialog("general");
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByLabelText("Name")).toBeTruthy();
    expect(screen.getByLabelText("Role and instructions")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeTruthy();
  });

  it("fills name and instructions from a newly selected avatar", () => {
    renderDialog("general");
    fireEvent.click(screen.getByRole("radio", { name: /Writer/ }));
    expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Writer");
    expect(
      (screen.getByLabelText("Role and instructions") as HTMLTextAreaElement).value,
    ).toContain("writing specialist");
  });

  it("keeps a custom name when the already selected avatar is clicked again", () => {
    renderDialog("general");
    fireEvent.click(screen.getByRole("radio", { name: /Chief of Staff/ }));
    expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Researcher");
    expect(
      (screen.getByLabelText("Role and instructions") as HTMLTextAreaElement).value,
    ).toBe("Research, compare sources, and deliver a concise result.");
  });

  it("notifies the parent when switching sections", () => {
    const onSectionChange = vi.fn();
    renderDialog("general", onSectionChange);
    fireEvent.click(screen.getByRole("tab", { name: "Permissions" }));
    expect(onSectionChange).toHaveBeenCalledWith("permissions");
  });

  it("renders permission radios on the Permissions panel", async () => {
    renderDialog("permissions");
    expect(await screen.findByRole("radiogroup", { name: "Write files permission" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Allow" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Use account default" })).toBeNull();
  });

  it("renders Skills, Workspace, ChatGPT, and Advanced surfaces", async () => {
    renderDialog("skills");
    expect(await screen.findByText("No skills attached yet.")).toBeTruthy();
    expect(screen.getByText("Manage catalog")).toBeTruthy();
    cleanup();

    renderDialog("workspace");
    expect(
      screen.getByText("Defaults for all Bots. Individual Bot settings can override these."),
    ).toBeTruthy();
    cleanup();

    renderDialog("chatgpt");
    expect(screen.getByText(/Connected/)).toBeTruthy();
    cleanup();

    renderDialog("advanced");
    fireEvent.click(screen.getByRole("button", { name: "Delete Bot" }));
    expect(screen.getByText("Delete Researcher?")).toBeTruthy();
  });
});
