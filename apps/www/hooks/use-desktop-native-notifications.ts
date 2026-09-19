"use client";

import { desktopNotificationsEnabled } from "@/lib/desktop-notifications-pref";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { useEffect, useRef } from "react";

async function sendNativeApprovalNotification(): Promise<void> {
  const { isPermissionGranted, requestPermission, sendNotification } = await import(
    "@tauri-apps/plugin-notification"
  );
  let granted = await isPermissionGranted();
  if (!granted) {
    const result = await requestPermission();
    granted = result === "granted";
  }
  if (!granted) {
    return;
  }
  sendNotification({
    title: "Approval needed",
    body: "A bot is waiting for your decision in Elsewhere.",
  });
}

/**
 * Desktop-only native notifications for approval backlog increases.
 */
export function useDesktopNativeNotifications(enabled = isTauriRuntime()) {
  const { data: workspace, phase } = useWorkspaceOverview();
  const lastApprovalCount = useRef<number | null>(null);
  const initialized = useRef(false);

  useEffect(() => {
    if (!enabled || !workspace || phase !== "ready") {
      return;
    }
    if (!desktopNotificationsEnabled()) {
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
      void sendNativeApprovalNotification().catch(() => {
        /* permission denied or plugin unavailable */
      });
    }
    lastApprovalCount.current = approvals;
  }, [enabled, phase, workspace]);
}
