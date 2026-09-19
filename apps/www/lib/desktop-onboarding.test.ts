import { describe, expect, it } from "vitest";
import {
  desktopOnboardingRedirect,
  resolveDesktopOnboardingStep,
  shouldShowThisMacOnboarding,
} from "./desktop-onboarding";

const baseMac = {
  phase: "disconnected" as const,
  paired: false,
  pairingInProgress: false,
  paused: false,
  onboardingSkipped: false,
  deviceName: "Drew's MacBook Air",
  nodeId: null,
  computerId: null,
  userCode: null,
};

describe("shouldShowThisMacOnboarding", () => {
  it("does not show for browser", () => {
    expect(
      shouldShowThisMacOnboarding({
        isDesktopShell: false,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: baseMac,
      }),
    ).toBe(false);
  });

  it("does not interrupt users who already have bots", () => {
    expect(
      shouldShowThisMacOnboarding({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 2,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: baseMac,
      }),
    ).toBe(false);
  });

  it("shows for first-run unpaired desktop users", () => {
    expect(
      shouldShowThisMacOnboarding({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: baseMac,
      }),
    ).toBe(true);
  });

  it("respects onboarding skipped", () => {
    expect(
      shouldShowThisMacOnboarding({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: { ...baseMac, onboardingSkipped: true },
      }),
    ).toBe(false);
  });
});

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

  it("uses overlay step instead of computers redirect", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: baseMac,
      }),
    ).toBe("this-mac-overlay");
  });

  it("opens quick start when there are no bots and mac is live", () => {
    expect(
      resolveDesktopOnboardingStep({
        isDesktopShell: true,
        hasSession: true,
        pathname: "/app",
        botCount: 0,
        workspaceReady: true,
        thisMacReady: true,
        thisMac: {
          ...baseMac,
          phase: "live",
          paired: true,
          computerId: "c",
        },
      }),
    ).toBe("quick-start");
  });
});

describe("desktopOnboardingRedirect", () => {
  it("maps steps to routes", () => {
    expect(desktopOnboardingRedirect("sign-in")).toBe("/sign-in");
    expect(desktopOnboardingRedirect("this-mac-overlay")).toBeNull();
    expect(desktopOnboardingRedirect("quick-start")).toBe("/app?create=1");
  });
});
