"use client";

import { DesktopThisMacOnboarding } from "@/components/app/desktop-this-mac-onboarding";
import { shouldShowThisMacOnboarding } from "@/lib/desktop-onboarding";
import { desktopCompanion } from "@/lib/desktop-companion";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { useThisMacStatus } from "@/hooks/use-this-mac-status";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { useCallback, useMemo, useState } from "react";

/**
 * Desktop-only runtime layer (hosted Next + Vite dev shell). Browser: no-op.
 */
export function DesktopExperienceLayer() {
  const enabled = desktopCompanion.isAvailable();
  const { status, loading, refresh } = useThisMacStatus(enabled);
  const { data: workspace, phase: workspacePhase } = useWorkspaceOverview();
  const [onboardingDismissed, setOnboardingDismissed] = useState(false);

  const workspaceReady =
    workspacePhase === "ready" || workspacePhase === "stale";
  const botCount = workspace?.bots.length ?? 0;

  const showOnboarding = useMemo(() => {
    if (!isTauriRuntime() || onboardingDismissed) {
      return false;
    }
    if (status?.onboardingSkipped) {
      return false;
    }
    if (status?.pairingInProgress || status?.phase === "live" || status?.phase === "connected") {
      return true;
    }
    return shouldShowThisMacOnboarding({
      isDesktopShell: true,
      hasSession: true,
      pathname: typeof window !== "undefined" ? window.location.pathname : "/app",
      botCount,
      workspaceReady,
      thisMacReady: !loading,
      thisMac: status,
    });
  }, [
    botCount,
    loading,
    onboardingDismissed,
    status,
    workspaceReady,
  ]);

  const handleFinished = useCallback(() => {
    setOnboardingDismissed(true);
    void refresh();
  }, [refresh]);

  if (!enabled) {
    return null;
  }

  return (
    <DesktopThisMacOnboarding
      open={showOnboarding}
      status={status}
      onRefreshStatus={() => void refresh()}
      onFinished={handleFinished}
      botCount={botCount}
    />
  );
}
