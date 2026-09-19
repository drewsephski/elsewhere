"use client";

import {
  desktopOnboardingRedirect,
  resolveDesktopOnboardingStep,
} from "@/lib/desktop-onboarding";
import { desktopCompanion } from "@/lib/desktop-companion";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { useThisMacStatus } from "@/hooks/use-this-mac-status";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";

interface DesktopShellBootstrapProps {
  hasSession: boolean;
}

/**
 * Bundled Vite dev shell: sign-in + quick-start redirects only.
 * This Mac onboarding is an overlay via {@link DesktopExperienceLayer}.
 */
export function DesktopShellBootstrap({ hasSession }: DesktopShellBootstrapProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const { status: thisMac, loading: thisMacLoading } = useThisMacStatus(
    desktopCompanion.isAvailable(),
  );
  const { data: workspace, phase: workspacePhase } = useWorkspaceOverview();

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }
    const workspaceReady =
      workspacePhase === "ready" || workspacePhase === "stale";
    const step = resolveDesktopOnboardingStep({
      isDesktopShell: true,
      hasSession,
      pathname: location.pathname,
      botCount: workspace?.bots.length ?? 0,
      workspaceReady,
      thisMacReady: !thisMacLoading,
      thisMac,
    });
    if (!step || step === "workspace" || step === "this-mac-overlay") {
      return;
    }
    const target = desktopOnboardingRedirect(step);
    if (!target) {
      return;
    }
    const [path, query] = target.split("?");
    if (location.pathname === path && (!query || location.search.includes(query.split("=")[0] ?? ""))) {
      return;
    }
    navigate(target, { replace: true });
  }, [
    hasSession,
    location.pathname,
    location.search,
    navigate,
    thisMac,
    thisMacLoading,
    workspace?.bots.length,
    workspacePhase,
  ]);

  return null;
}
