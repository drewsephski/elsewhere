import type { ThisMacStatusSnapshot } from "@/lib/this-mac-status";

export type DesktopOnboardingStep =
  | "workspace"
  | "sign-in"
  | "this-mac-overlay"
  | "quick-start";

export interface DesktopOnboardingContext {
  isDesktopShell: boolean;
  hasSession: boolean;
  pathname: string;
  botCount: number;
  workspaceReady: boolean;
  thisMacReady: boolean;
  thisMac: ThisMacStatusSnapshot | null;
}

/** First-run This Mac overlay — not the generic Computers admin page. */
export function shouldShowThisMacOnboarding(
  ctx: DesktopOnboardingContext,
): boolean {
  if (!ctx.isDesktopShell) {
    return false;
  }
  if (!ctx.hasSession || !ctx.thisMacReady) {
    return false;
  }
  if (ctx.botCount > 0) {
    return false;
  }
  const mac = ctx.thisMac;
  if (!mac || mac.onboardingSkipped || mac.paired || mac.pairingInProgress) {
    return false;
  }
  if (mac.phase === "reauth" || mac.phase === "reconnecting") {
    return false;
  }
  return mac.phase === "disconnected";
}

/**
 * State-driven desktop entry for the bundled dev shell (React Router redirects).
 * Hosted production uses overlays in {@link DesktopExperienceLayer}.
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
  if (shouldShowThisMacOnboarding(ctx)) {
    return "this-mac-overlay";
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
    case "this-mac-overlay":
      return null;
    case "quick-start":
      return "/app?create=1";
    default:
      return null;
  }
}
