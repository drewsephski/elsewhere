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
 * State-driven desktop onboarding (returning users, auth, This Mac, quick start).
 * No-op in the browser.
 */
export function DesktopShellBootstrap({ hasSession }: DesktopShellBootstrapProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const { status: thisMac } = useThisMacStatus(desktopCompanion.isAvailable());
  const { data: workspace } = useWorkspaceOverview();

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }
    const step = resolveDesktopOnboardingStep({
      isDesktopShell: true,
      hasSession,
      pathname: location.pathname,
      botCount: workspace?.bots.length ?? 0,
      thisMac,
    });
    if (!step || step === "workspace") {
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
  }, [hasSession, location.pathname, location.search, navigate, thisMac, workspace?.bots.length]);

  return null;
}
