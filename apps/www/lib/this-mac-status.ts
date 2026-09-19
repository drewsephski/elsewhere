/** UI-facing connection phases for This Mac (desktop companion). */
export type ThisMacPhase =
  | "disconnected"
  | "connecting"
  | "connected"
  | "live"
  | "offline"
  | "reconnecting"
  | "reauth"
  | "paused";

export interface ThisMacStatusSnapshot {
  phase: ThisMacPhase;
  paired: boolean;
  pairingInProgress: boolean;
  paused: boolean;
  /** First-run overlay suppressed (Not now or successful Continue). */
  onboardingDismissed: boolean;
  deviceName: string;
  nodeId: string | null;
  computerId: string | null;
  userCode: string | null;
}

export function thisMacPhaseLabel(phase: ThisMacPhase): string {
  switch (phase) {
    case "disconnected":
      return "Not connected";
    case "connecting":
      return "Connecting…";
    case "connected":
      return "Paired — connecting";
    case "live":
      return "Live";
    case "offline":
      return "Offline";
    case "reconnecting":
      return "Reconnecting…";
    case "reauth":
      return "Reconnect required";
    case "paused":
      return "Paused";
    default:
      return "This Mac";
  }
}

export function thisMacPhaseTone(
  phase: ThisMacPhase,
): "default" | "success" | "warning" | "destructive" | "muted" {
  switch (phase) {
    case "live":
    case "connected":
      return "success";
    case "connecting":
    case "reconnecting":
      return "warning";
    case "reauth":
    case "offline":
      return "destructive";
    case "paused":
      return "muted";
    default:
      return "default";
  }
}

export function isThisMacLiveForQuickStart(
  status: ThisMacStatusSnapshot | null | undefined,
): boolean {
  return Boolean(
    status &&
      status.phase === "live" &&
      !status.paused &&
      status.computerId,
  );
}
