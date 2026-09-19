import type { ThisMacPhase, ThisMacStatusSnapshot } from "@/lib/this-mac-status";
import { isTauriRuntime } from "@/lib/tauri-runtime";

/** Narrow desktop IPC surface — pairing, pause, and status only (no bot/chat DB). */
export type { ThisMacPhase, ThisMacStatusSnapshot };

type RawThisMacStatus = {
  phase: string;
  paired: boolean;
  pairingInProgress: boolean;
  paused: boolean;
  onboardingSkipped: boolean;
  deviceName: string;
  nodeId: string | null;
  computerId: string | null;
  userCode: string | null;
};

const PHASE_MAP: Record<string, ThisMacPhase> = {
  disconnected: "disconnected",
  connecting: "connecting",
  connected: "connected",
  live: "live",
  offline: "offline",
  reconnecting: "reconnecting",
  reauth: "reauth",
  paused: "paused",
};

function normalizePhase(phase: string): ThisMacPhase {
  const key = phase.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();
  return PHASE_MAP[key] ?? PHASE_MAP[phase.toLowerCase()] ?? "disconnected";
}

function normalizeStatus(raw: RawThisMacStatus): ThisMacStatusSnapshot {
  return {
    phase: normalizePhase(raw.phase),
    paired: raw.paired,
    pairingInProgress: raw.pairingInProgress,
    paused: raw.paused,
    onboardingSkipped: raw.onboardingSkipped,
    deviceName: raw.deviceName?.trim() || "This Mac",
    nodeId: raw.nodeId,
    computerId: raw.computerId,
    userCode: raw.userCode,
  };
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export const desktopCompanion = {
  isAvailable(): boolean {
    return isTauriRuntime();
  },

  async getThisMacStatus(): Promise<ThisMacStatusSnapshot> {
    const raw = await invokeDesktop<RawThisMacStatus>("get_this_mac_status");
    return normalizeStatus(raw);
  },

  async setThisMacPaused(paused: boolean): Promise<ThisMacStatusSnapshot> {
    const raw = await invokeDesktop<RawThisMacStatus>("set_this_mac_paused", { paused });
    return normalizeStatus(raw);
  },

  async startThisMacPairing(): Promise<ThisMacStatusSnapshot> {
    const raw = await invokeDesktop<RawThisMacStatus>("start_this_mac_pairing");
    return normalizeStatus(raw);
  },

  async setThisMacOnboardingSkipped(skipped: boolean): Promise<ThisMacStatusSnapshot> {
    const raw = await invokeDesktop<RawThisMacStatus>("set_this_mac_onboarding_skipped", {
      skipped,
    });
    return normalizeStatus(raw);
  },
};
