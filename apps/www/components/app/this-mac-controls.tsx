"use client";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { desktopCompanion } from "@/lib/desktop-companion";
import { thisMacPhaseLabel, thisMacPhaseTone } from "@/lib/this-mac-status";
import { useThisMacStatus } from "@/hooks/use-this-mac-status";
import { cn } from "cn";
import { useCallback, useState } from "react";

interface ThisMacControlsProps {
  layout?: "inline" | "stack";
  className?: string;
}

export function ThisMacControls({ layout = "inline", className }: ThisMacControlsProps) {
  const { status, error, refresh } = useThisMacStatus();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const handlePair = useCallback(async () => {
    setBusy(true);
    setActionError(null);
    try {
      await desktopCompanion.startThisMacPairing();
      await refresh();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Pairing failed");
    } finally {
      setBusy(false);
    }
  }, [refresh]);

  const handlePauseToggle = useCallback(async () => {
    if (!status) {
      return;
    }
    setBusy(true);
    setActionError(null);
    try {
      await desktopCompanion.setThisMacPaused(!status.paused);
      await refresh();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : "Could not update pause state");
    } finally {
      setBusy(false);
    }
  }, [refresh, status]);

  if (!desktopCompanion.isAvailable()) {
    return null;
  }

  const phase = status?.phase ?? "disconnected";
  const tone = thisMacPhaseTone(phase);
  const needsConnect = !status?.paired || phase === "reauth" || phase === "disconnected";

  return (
    <div
      className={cn(
        layout === "stack" ? "flex flex-col items-stretch gap-2" : "flex flex-wrap items-center gap-2",
        className,
      )}
    >
      <Badge
        variant="secondary"
        className={cn(
          tone === "success" && "bg-emerald-600/90 text-white",
          tone === "warning" && "bg-amber-500/90 text-black",
          tone === "destructive" && "bg-destructive/90 text-destructive-foreground",
        )}
      >
        This Mac · {thisMacPhaseLabel(phase)}
      </Badge>
      {needsConnect ? (
        <Button type="button" size="sm" disabled={busy} onClick={() => void handlePair()}>
          {busy ? "Waiting for approval…" : phase === "reauth" ? "Reconnect This Mac" : "Use this Mac"}
        </Button>
      ) : (
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy}
          onClick={() => void handlePauseToggle()}
        >
          {status?.paused ? "Resume This Mac" : "Pause This Mac"}
        </Button>
      )}
      {(actionError ?? error) ? (
        <p className="text-xs text-destructive" role="alert">
          {actionError ?? error}
        </p>
      ) : null}
    </div>
  );
}
