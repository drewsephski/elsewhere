"use client";

import { Button } from "@/components/ui/button";
import {
  humanInterventionReasonLabel,
  type PendingHumanIntervention,
} from "@/lib/human-intervention";
import { takeBrowserControl } from "@/lib/browser-control";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { HandHelping } from "lucide-react";
import { useState } from "react";

interface HumanInterventionBannerProps {
  pending: PendingHumanIntervention;
  onTakeControlComplete?: () => void;
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

  return (
    <Alert variant="warning" className="border-amber-500/40 bg-amber-500/10">
      <HandHelping className="size-4" aria-hidden />
      <AlertTitle>Bot needs you</AlertTitle>
      <AlertDescription className="space-y-3">
        <p>
          <span className="font-medium">{humanInterventionReasonLabel(pending.reason)}</span>
          {" — "}
          {pending.message}
        </p>
        <p className="text-muted-foreground text-sm">
          Take control to complete this step in the browser, then use Return control to bot when
          finished.
        </p>
        {error ? <p className="text-destructive text-sm">{error}</p> : null}
        <Button type="button" size="sm" disabled={busy} onClick={() => void handleTakeControl()}>
          {busy ? "Taking control…" : "Take control"}
        </Button>
      </AlertDescription>
    </Alert>
  );
}
