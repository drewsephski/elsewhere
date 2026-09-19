"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { ProductThemeScope } from "@/components/app/product-theme-scope";
import { Button } from "@/components/ui/button";
import { desktopCompanion } from "@/lib/desktop-companion";
import { JOB_BOT_IMAGES } from "@/lib/bot-avatars";
import type { ThisMacStatusSnapshot } from "@/lib/this-mac-status";
import { cn } from "cn";
import { useCallback, useState } from "react";

interface DesktopThisMacOnboardingProps {
  open: boolean;
  status: ThisMacStatusSnapshot | null;
  onRefreshStatus: () => void;
  onFinished: () => void;
  botCount: number;
}

function pairingHeadline(status: ThisMacStatusSnapshot | null): string {
  if (!status) {
    return "Preparing this Mac…";
  }
  if (status.pairingInProgress && status.phase === "connecting") {
    return "Preparing this Mac…";
  }
  if (status.userCode) {
    return "Approve the connection in your browser";
  }
  if (status.phase === "connected" || status.phase === "reconnecting") {
    return "Paired — connecting…";
  }
  if (status.phase === "live") {
    const name = status.deviceName || "This Mac";
    return `${name} is ready`;
  }
  return "Preparing this Mac…";
}

function pairingSubcopy(status: ThisMacStatusSnapshot | null): string | null {
  if (!status) {
    return null;
  }
  if (status.userCode) {
    return "Waiting for you to confirm in the browser window.";
  }
  if (status.phase === "live") {
    return null;
  }
  if (status.pairingInProgress || status.phase === "connected") {
    return null;
  }
  return null;
}

export function DesktopThisMacOnboarding({
  open,
  status,
  onRefreshStatus,
  onFinished,
  botCount,
}: DesktopThisMacOnboardingProps) {
  const [busy, setBusy] = useState(false);
  const [flowActive, setFlowActive] = useState(false);

  const showConnectActions =
    !flowActive &&
    (!status || (status.phase === "disconnected" && !status.pairingInProgress));

  const showContinue = status?.phase === "live";

  const handleConnect = useCallback(async () => {
    setBusy(true);
    setFlowActive(true);
    try {
      await desktopCompanion.startThisMacPairing();
      onRefreshStatus();
    } finally {
      setBusy(false);
    }
  }, [onRefreshStatus]);

  const handleNotNow = useCallback(async () => {
    setBusy(true);
    try {
      await desktopCompanion.setThisMacOnboardingSkipped(true);
      onFinished();
    } finally {
      setBusy(false);
    }
  }, [onFinished]);

  const handleContinue = useCallback(() => {
    onFinished();
    if (botCount === 0) {
      const params = new URLSearchParams(window.location.search);
      params.set("create", "1");
      const query = params.toString();
      window.location.assign(query ? `/app?${query}` : "/app?create=1");
    }
  }, [botCount, onFinished]);

  if (!open) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-[200] flex flex-col items-center justify-center app-shell-bg px-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby="desktop-this-mac-title"
    >
      <ProductThemeScope />
      <div className="surface-panel flex w-full max-w-lg flex-col items-center px-8 py-10 text-center">
        <div className="relative mb-8 flex h-28 w-full max-w-sm items-end justify-center">
          <BotCreatureAvatar
            name="Engineer"
            src={JOB_BOT_IMAGES.engineer}
            size="xl"
            animated
            className={cn("absolute left-[8%] z-10 -rotate-6")}
          />
          <BotCreatureAvatar
            name="Researcher"
            src={JOB_BOT_IMAGES.researcher}
            size="2xl"
            animated
            className="relative z-20"
          />
          <BotCreatureAvatar
            name="Chief of Staff"
            src={JOB_BOT_IMAGES.chiefOfStaff}
            size="xl"
            animated
            className={cn("absolute right-[8%] z-10 rotate-6")}
          />
        </div>

        {showConnectActions ? (
          <>
            <h1
              id="desktop-this-mac-title"
              className="text-2xl font-semibold tracking-tight text-foreground"
            >
              Use this Mac
            </h1>
            <p className="mt-3 max-w-md text-sm leading-6 text-muted-foreground">
              Let Bots you assign use this Mac through your existing Files, Terminal, and
              Browser permissions.
            </p>
            <div className="mt-8 flex w-full flex-col gap-3 sm:flex-row sm:justify-center">
              <Button
                size="lg"
                className="w-full sm:w-auto"
                disabled={busy}
                onClick={() => void handleConnect()}
              >
                Connect this Mac
              </Button>
              <Button
                size="lg"
                variant="outline"
                className="w-full sm:w-auto"
                disabled={busy}
                onClick={() => void handleNotNow()}
              >
                Not now
              </Button>
            </div>
            <p className="mt-6 max-w-sm text-xs leading-5 text-muted-foreground">
              You stay in control. Actions still follow your Elsewhere approval settings.
            </p>
          </>
        ) : (
          <>
            <h1 className="text-xl font-semibold tracking-tight text-foreground">
              {pairingHeadline(status)}
            </h1>
            {pairingSubcopy(status) ? (
              <p className="mt-2 text-sm text-muted-foreground">{pairingSubcopy(status)}</p>
            ) : null}
            {status?.phase === "live" ? (
              <p className="mt-3 flex items-center justify-center gap-2 text-sm font-medium text-foreground">
                <span className="size-2 rounded-full bg-emerald-500" aria-hidden />
                Live
              </p>
            ) : (
              <p className="mt-6 text-sm text-muted-foreground" aria-live="polite">
                {status?.pairingInProgress || status?.phase === "connected"
                  ? "This can take a moment."
                  : null}
              </p>
            )}
            {showContinue ? (
              <Button size="lg" className="mt-8" onClick={handleContinue}>
                Continue
              </Button>
            ) : null}
          </>
        )}
      </div>
    </div>
  );
}
