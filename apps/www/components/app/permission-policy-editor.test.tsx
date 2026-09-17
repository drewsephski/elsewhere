// @vitest-environment happy-dom

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PermissionPolicyEditor } from "./permission-policy-editor";
import { cloudHostFetch } from "@/lib/cloud-api";

vi.mock("@/lib/cloud-api", () => ({
  cloudHostFetch: vi.fn(),
}));

const catalog = {
  actions: [
    {
      action: "workspace_write",
      label: "Write files",
      group: "files",
      groupLabel: "Files",
      decision: "ask",
      source: "bot",
      inherited: false,
      inheritedDecision: "ask",
      overridable: true,
    },
  ],
};

describe("PermissionPolicyEditor", () => {
  it("sends Allow / Ask / Deny and can reset to the account default", async () => {
    vi.mocked(cloudHostFetch).mockImplementation(async (path, init) => {
      if (!init?.method) {
        return { ok: true, json: async () => catalog } as Response;
      }
      const body = JSON.parse(String(init.body)) as {
        policies: Array<{ action: string; decision: string | null }>;
      };
      return {
        ok: true,
        json: async () => ({
          actions: [
            {
              ...catalog.actions[0],
              decision: body.policies[0]?.decision ?? "ask",
              inherited: body.policies[0]?.decision === null,
            },
          ],
        }),
      } as Response;
    });

    render(
      <PermissionPolicyEditor endpoint="/v1/bots/bot_1/permission-policies" mode="bot" />,
    );

    const deny = await screen.findByRole("radio", { name: "Deny" });
    fireEvent.click(deny);
    await waitFor(() => {
      expect(vi.mocked(cloudHostFetch)).toHaveBeenCalledWith(
        "/v1/bots/bot_1/permission-policies",
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify({
            policies: [{ action: "workspace_write", decision: "deny" }],
          }),
        }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Use account default" }));
    await waitFor(() => {
      expect(vi.mocked(cloudHostFetch)).toHaveBeenCalledWith(
        "/v1/bots/bot_1/permission-policies",
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify({
            policies: [{ action: "workspace_write", decision: null }],
          }),
        }),
      );
    });
  });
});
