"use client";

import { isTauriRuntime } from "@/lib/tauri-runtime";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { useEffect, useRef } from "react";

function mayNotify(): boolean {
  return (
    isTauriRuntime() &&
    typeof Notification !== "undefined" &&
    Notification.permission !== "denied"
  );
}

async function ensurePermission(): Promise<boolean> {
  if (!mayNotify()) {
    return false;
  }
  if (Notification.permission === "granted") {
    return true;
  }
  const result = await Notification.requestPermission();
  return result === "granted";
}

/**
 * Lightweight native notifications for approval backlog and failed work (desktop only).
 */
export function useDesktopNativeNotifications(enabled = isTauriRuntime()) {
  const { data: workspace } = useWorkspaceOverview();
  const lastApprovalCount = useRef<number | null>(null);
  const lastAttentionBot = useRef<string | null>(null);

  useEffect(() => {
    if (!enabled || !workspace) {
      return;
    }
    let cancelled = false;
    const snapshot = workspace;
    async function run() {
      const ok = await ensurePermission();
      if (!ok || cancelled || !snapshot) {
        return;
      }
      const approvals = snapshot.counts.approvals;
      if (
        lastApprovalCount.current !== null &&
        approvals > lastApprovalCount.current
      ) {
        new Notification("Approval needed", {
          body: "A bot is waiting for your decision in Elsewhere.",
          tag: "elsewhere-approval",
        });
      }
      lastApprovalCount.current = approvals;

      const needsYou = snapshot.bots.find(
        (bot) => bot.presence === "waiting_approval" || bot.presence === "needs_attention",
      );
      if (needsYou && needsYou.id !== lastAttentionBot.current) {
        lastAttentionBot.current = needsYou.id;
        new Notification("Your bot needs you", {
          body: needsYou.task?.slice(0, 120) || `${needsYou.name} needs attention.`,
          tag: `elsewhere-attention-${needsYou.id}`,
        });
      }
    }
    void run();
    return () => {
      cancelled = true;
    };
  }, [enabled, workspace]);
}
