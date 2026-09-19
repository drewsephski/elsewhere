import { describe, expect, it } from "vitest";
import { appRoutes } from "./app-routes";
import { cloudShellAuthenticatedRoutes } from "./cloud-shell-routes";

/** Map canonical app hrefs to nested CloudShell route segments (under `/app`). */
const APP_ROUTE_TO_SEGMENT: Record<string, string> = {
  [appRoutes.workspace]: "index",
  [appRoutes.computers]: "computers",
  [appRoutes.routines]: "routines",
  [appRoutes.approvals]: "approvals",
  [appRoutes.work]: "work",
  [appRoutes.results]: "results",
  [appRoutes.skills]: "skills",
  [appRoutes.connectors]: "connectors",
  [appRoutes.channels]: "channels",
  [appRoutes.settings]: "settings",
};

describe("cloudShellAuthenticatedRoutes", () => {
  it("covers primary management routes from appRoutes", () => {
    const segments = new Set(
      cloudShellAuthenticatedRoutes.map((route) => route.path),
    );
    for (const href of Object.keys(APP_ROUTE_TO_SEGMENT)) {
      const segment = APP_ROUTE_TO_SEGMENT[href];
      expect(segments.has(segment)).toBe(true);
    }
  });

  it("includes OAuth callback segments for connectors and channels", () => {
    const paths = cloudShellAuthenticatedRoutes.map((route) => route.path);
    expect(paths).toContain("connectors/github/callback");
    expect(paths).toContain("connectors/mcp/callback");
    expect(paths).toContain("channels/slack/callback");
  });
});
