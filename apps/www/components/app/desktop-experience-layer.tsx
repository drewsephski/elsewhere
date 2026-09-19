"use client";

import { DesktopThisMacOnboarding } from "@/components/app/desktop-this-mac-onboarding";
import { shouldShowDesktopOnboardingOverlay } from "@/lib/desktop-onboarding";
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
  const [pairingFlowActive, setPairingFlowActive] = useState(false);

  const workspaceReady =
    workspacePhase === "ready" || workspacePhase === "stale";
  const botCount = workspace?.bots.length ?? 0;

  const onboardingContext = useMemo(
    () => ({
      isDesktopShell: true,
      hasSession: true,
      pathname: typeof window !== "undefined" ? window.location.pathname : "/app",
      botCount,
      workspaceReady,
      thisMacReady: !loading,
      thisMac: status,
    }),
    [botCount, loading, status, workspaceReady],
  );

  const showOnboarding = useMemo(() => {
    if (!isTauriRuntime()) {
      return false;
    }
    return shouldShowDesktopOnboardingOverlay(onboardingContext, {
      pairingFlowActive,
    });
  }, [onboardingContext, pairingFlowActive]);

  const handlePairingStarted = useCallback(() => {
    setPairingFlowActive(true);
  }, []);

  const handlePersistDismissed = useCallback(async () => {
    await desktopCompanion.setThisMacOnboardingDismissed(true);
    setPairingFlowActive(false);
    await refresh();
  }, [refresh]);

  if (!enabled) {
    return null;
  }

  return (
    <DesktopThisMacOnboarding
      open={showOnboarding}
      pairingFlowActive={pairingFlowActive}
      status={status}
      onPairingStarted={handlePairingStarted}
      onRefreshStatus={() => void refresh()}
      onPersistDismissed={handlePersistDismissed}
      botCount={botCount}
    />
  );
}
