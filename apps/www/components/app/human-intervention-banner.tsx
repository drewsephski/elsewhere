"use client";

import { NeedsYouCard } from "@/components/app/needs-you-card";
import { Button } from "@/components/ui/button";
import {
  humanInterventionReasonLabel,
  type PendingHumanIntervention,
} from "@/lib/human-intervention";
import { takeBrowserControl } from "@/lib/browser-control";
import { useState } from "react";

interface HumanInterventionBannerProps {
  pending: PendingHumanIntervention;
  onTakeControlComplete?: () => void;
}

function primaryActionLabel(reason: string): string {
  if (reason === "login" || reason === "credentials") {
    return "Sign in in browser";
  }
  if (reason === "captcha" || reason === "two_factor" || reason === "passkey") {
    return "Take control";
  }
  return "Take control";
}

export function HumanInterventionBanner({
  pending,
  onTakeControlComplete,
}: HumanInterventionBannerProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleTakeControl() {
    setBusy(true);
    setError(null);
    try {
      await takeBrowserControl(pending.computerId);
      onTakeControlComplete?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not take control");
    } finally {
      setBusy(false);
    }
  }

  const reasonLabel = humanInterventionReasonLabel(pending.reason);

  return (
    <NeedsYouCard
      title="Your bot needs you in the browser"
      reason={`${reasonLabel} — ${pending.message}`}
      continuation="Use Watch in the sidebar to follow along. Choose Return to bot when you are done so work can continue."
      actions={
        <>
          <Button type="button" size="sm" disabled={busy} onClick={() => void handleTakeControl()}>
            {busy ? "Opening…" : primaryActionLabel(pending.reason)}
          </Button>
          {error ? (
            <p className="w-full text-xs text-destructive" role="alert">{error}</p>
          ) : null}
        </>
      }
    />
  );
}
