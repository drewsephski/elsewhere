import type { ThisMacStatusSnapshot } from "@/lib/this-mac-status";

export type DesktopOnboardingStep =
  | "workspace"
  | "sign-in"
  | "connect-this-mac"
  | "quick-start";

export interface DesktopOnboardingContext {
  isDesktopShell: boolean;
  hasSession: boolean;
  pathname: string;
  botCount: number;
  /** Avoid quick-start redirect while workspace overview is still loading. */
  workspaceReady: boolean;
  /** Avoid connect redirect while companion status is still loading. */
  thisMacReady: boolean;
  thisMac: ThisMacStatusSnapshot | null;
}

/**
 * State-driven desktop entry: returning users land in workspace; signed-out users
 * see auth; signed-in users without a paired Mac are steered to connect; empty
 * workspaces open quick start (preferring This Mac in the quick-start layer).
 */
export function resolveDesktopOnboardingStep(
  ctx: DesktopOnboardingContext,
): DesktopOnboardingStep | null {
  if (!ctx.isDesktopShell) {
    return null;
  }
  if (ctx.pathname.startsWith("/pair/mac")) {
    return null;
  }
  if (!ctx.hasSession) {
    if (ctx.pathname === "/sign-in" || ctx.pathname === "/sign-up") {
      return null;
    }
    return "sign-in";
  }
  if (!ctx.thisMacReady) {
    return "workspace";
  }
  const mac = ctx.thisMac;
  if (mac && !mac.paired && !mac.pairingInProgress) {
    if (ctx.pathname === "/app/computers") {
      return null;
    }
    return "connect-this-mac";
  }
  if (
    ctx.workspaceReady &&
    ctx.botCount === 0 &&
    ctx.pathname === "/app"
  ) {
    return "quick-start";
  }
  return "workspace";
}

export function desktopOnboardingRedirect(
  step: DesktopOnboardingStep,
): string | null {
  switch (step) {
    case "sign-in":
      return "/sign-in";
    case "connect-this-mac":
      return "/app/computers";
    case "quick-start":
      return "/app?create=1";
    default:
      return null;
  }
}
