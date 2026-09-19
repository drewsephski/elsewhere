import { describe, expect, it } from "vitest";
import {
  desktopOnboardingRedirect,
  resolveDesktopOnboardingStep,
  shouldShowDesktopOnboardingOverlay,
  shouldShowThisMacOnboarding,
} from "./desktop-onboarding";

const baseMac = {
  phase: "disconnected" as const,
  paired: false,
  pairingInProgress: false,
  paused: false,
  onboardingDismissed: false,
  deviceName: "Drew's MacBook Air",
  nodeId: null,
  computerId: null,
  userCode: null,
};

const baseCtx = {
  isDesktopShell: true,
  hasSession: true,
  pathname: "/app",
  botCount: 0,
  workspaceReady: true,
  thisMacReady: true,
  thisMac: baseMac,
};

describe("shouldShowThisMacOnboarding", () => {
  it("does not show for browser", () => {
    expect(
      shouldShowThisMacOnboarding({
        ...baseCtx,
        isDesktopShell: false,
      }),
    ).toBe(false);
  });

  it("does not interrupt users who already have bots", () => {
    expect(
      shouldShowThisMacOnboarding({
        ...baseCtx,
        botCount: 2,
      }),
    ).toBe(false);
  });

  it("shows for first-run unpaired desktop users", () => {
    expect(shouldShowThisMacOnboarding(baseCtx)).toBe(true);
  });

  it("respects onboarding dismissed", () => {
    expect(
      shouldShowThisMacOnboarding({
        ...baseCtx,
        thisMac: { ...baseMac, onboardingDismissed: true },
      }),
    ).toBe(false);
  });

  it("does not show for reconnecting previously paired mac", () => {
    expect(
      shouldShowThisMacOnboarding({
        ...baseCtx,
        thisMac: {
          ...baseMac,
          paired: true,
          phase: "reconnecting",
        },
      }),
    ).toBe(false);
  });
});

describe("shouldShowDesktopOnboardingOverlay", () => {
  it("fresh first run: unpaired, no bots, not dismissed → overlay", () => {
    expect(
      shouldShowDesktopOnboardingOverlay(baseCtx, { pairingFlowActive: false }),
    ).toBe(true);
  });

  it("active pairing: pairingFlowActive keeps overlay through live", () => {
    const liveMac = {
      ...baseMac,
      phase: "live" as const,
      paired: true,
      computerId: "c1",
    };
    expect(
      shouldShowDesktopOnboardingOverlay(
        { ...baseCtx, thisMac: liveMac },
        { pairingFlowActive: true },
      ),
    ).toBe(true);
    expect(
      shouldShowDesktopOnboardingOverlay(
        { ...baseCtx, thisMac: liveMac },
        { pairingFlowActive: false },
      ),
    ).toBe(false);
  });

  it("app restart: paired/live + onboarding dismissed → overlay hidden", () => {
    expect(
      shouldShowDesktopOnboardingOverlay(
        {
          ...baseCtx,
          thisMac: {
            ...baseMac,
            phase: "live",
            paired: true,
            computerId: "c1",
            onboardingDismissed: true,
          },
        },
        { pairingFlowActive: false },
      ),
    ).toBe(false);
  });

  it("existing user: live mac + bots > 0 → overlay hidden", () => {
    expect(
      shouldShowDesktopOnboardingOverlay(
        {
          ...baseCtx,
          botCount: 3,
          thisMac: {
            ...baseMac,
            phase: "live",
            paired: true,
            computerId: "c1",
          },
        },
        { pairingFlowActive: false },
      ),
    ).toBe(false);
  });

  it("not now: dismissed suppresses automatic overlay", () => {
    expect(
      shouldShowDesktopOnboardingOverlay(
        {
          ...baseCtx,
          thisMac: { ...baseMac, onboardingDismissed: true },
        },
        { pairingFlowActive: false },
      ),
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
        ...baseCtx,
        hasSession: false,
        thisMac: null,
      }),
    ).toBe("sign-in");
  });

  it("uses overlay step instead of computers redirect", () => {
    expect(resolveDesktopOnboardingStep(baseCtx)).toBe("this-mac-overlay");
  });

  it("opens quick start when there are no bots and mac is live", () => {
    expect(
      resolveDesktopOnboardingStep({
        ...baseCtx,
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
