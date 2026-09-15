import { cloudHostFetch } from "@/lib/cloud-api";

export interface PendingHumanIntervention {
  id: string;
  runId: string;
  computerId: string;
  reason: string;
  message: string;
  requestedAt: string;
}

export interface HumanInterventionStatus {
  pending: PendingHumanIntervention | null;
}

export async function fetchHumanInterventionStatus(
  runId: string,
): Promise<HumanInterventionStatus> {
  const response = await cloudHostFetch(
    `/v1/runs/${encodeURIComponent(runId)}/human-intervention`,
  );
  if (!response.ok) {
    throw new Error("Could not load human intervention status");
  }
  return response.json() as Promise<HumanInterventionStatus>;
}

export function humanInterventionReasonLabel(reason: string): string {
  const labels: Record<string, string> = {
    login: "Sign-in required",
    captcha: "CAPTCHA required",
    two_factor: "Two-factor required",
    passkey: "Passkey required",
    credentials: "Credentials required",
    consent: "Consent required",
    other: "Your action required",
  };
  return labels[reason] ?? "Your action required";
}
