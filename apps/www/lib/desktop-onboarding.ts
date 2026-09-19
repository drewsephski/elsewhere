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

export interface DesktopOnboardingOverlayOptions {
  /** User tapped Connect this Mac in the current UI session. */
  pairingFlowActive: boolean;
}

/**
 * Whether the first-run This Mac overlay should be visible.
 * Pairing flow stays open via `pairingFlowActive`, not via `phase === live`.
 */
export function shouldShowDesktopOnboardingOverlay(
  ctx: DesktopOnboardingContext,
  options: DesktopOnboardingOverlayOptions,
): boolean {
  if (!ctx.isDesktopShell || !ctx.hasSession || !ctx.thisMacReady) {
    return false;
  }
  const mac = ctx.thisMac;
  if (mac?.onboardingDismissed) {
    return false;
  }
  if (options.pairingFlowActive) {
    return true;
  }
  return shouldShowThisMacOnboarding(ctx);
}

/** Automatic first-run presentation only (not active pairing flow). */
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
  if (!mac || mac.onboardingDismissed || mac.paired || mac.pairingInProgress) {
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
