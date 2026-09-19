import { describe, expect, it } from "vitest";
import {
  desktopOnboardingRedirect,
  resolveDesktopOnboardingStep,
} from "./desktop-onboarding";

describe("resolveDesktopOnboardingStep", () => {
  it("ignores non-desktop shells", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: false,
        hasSession: false,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: null,
      }),
    ).toBeNull();
  });

  it("steers signed-out users to sign-in", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: true,
        hasSession: false,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: null,
      }),
    ).toBe("sign-in");
  });

  it("steers signed-in users without a paired Mac to computers", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 2,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: {
          phase: "disconnected",
          paired: false,
          pairingInProgress: false,
          paused: false,
          nodeId: null,
          computerId: null,
          userCode: null,
        },
      }),
    ).toBe("connect-this-mac");
  });

  it("opens quick start when there are no bots", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: {
          phase: "live",
          paired: true,
          pairingInProgress: false,
          paused: false,
          nodeId: "n",
          computerId: "c",
          userCode: null,
        },
      }),
    ).toBe("quick-start");
  });

  it("does not quick-start while workspace is still loading", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: false,
        thisMacReady: true,
        thisMac: {
          phase: "live",
          paired: true,
          pairingInProgress: false,
          paused: false,
          nodeId: "n",
          computerId: "c",
          userCode: null,
        },
      }),
    ).toBe("workspace");
  });
});

describe("desktopOnboardingRedirect", () => {
  it("maps steps to routes", () => {
    expect(desktopOnboardingRedirect("sign-in")).toBe("/sign-in");
    expect(desktopOnboardingRedirect("connect-this-mac")).toBe("/app/computers");
    expect(desktopOnboardingRedirect("quick-start")).toBe("/app?create=1");
  });
});
