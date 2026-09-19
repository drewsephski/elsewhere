"use client";

import { desktopNotificationsEnabled } from "@/lib/desktop-notifications-pref";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { appRoutes } from "@/lib/app-routes";
import { useEffect, useRef } from "react";

function mayNotify(): boolean {
  return (
    isTauriRuntime() &&
    desktopNotificationsEnabled() &&
    typeof Notification !== "undefined" &&
    Notification.permission === "granted"
  );
}

function focusApprovals(): void {
  if (typeof window === "undefined") {
    return;
  }
  window.focus();
  const path = appRoutes.approvals;
  if (window.location.pathname !== path) {
    window.location.assign(path);
  }
}

/**
 * Desktop-only notifications for approval backlog increases (no permission prompts here).
 */
export function useDesktopNativeNotifications(enabled = isTauriRuntime()) {
  const { data: workspace, phase } = useWorkspaceOverview();
  const lastApprovalCount = useRef<number | null>(null);
  const initialized = useRef(false);

  useEffect(() => {
    if (!enabled || !workspace || phase !== "ready") {
      return;
    }
    if (!mayNotify()) {
      lastApprovalCount.current = workspace.counts.approvals;
      initialized.current = true;
      return;
    }

    const approvals = workspace.counts.approvals;
    if (!initialized.current) {
      lastApprovalCount.current = approvals;
      initialized.current = true;
      return;
    }

    if (
      lastApprovalCount.current !== null &&
      approvals > lastApprovalCount.current
    ) {
      const notification = new Notification("Approval needed", {
        body: "A bot is waiting for your decision in Elsewhere.",
        tag: "elsewhere-approval",
      });
      notification.onclick = () => {
        notification.close();
        focusApprovals();
      };
    }
    lastApprovalCount.current = approvals;
  }, [enabled, phase, workspace]);
}
