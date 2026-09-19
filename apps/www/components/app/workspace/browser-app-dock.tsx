"use client";

import { MacOSDock, type MacOSDockSize } from "@/components/ui/mac-os-dock";
import {
  BROWSER_DOCK_ICON_APPS,
  browserDockAppById,
  dockAppIdForUrl,
} from "@/lib/browser-dock";
import { cn } from "cn";
import { useState } from "react";

interface BrowserAppDockProps {
  url: string | null | undefined;
  enabled: boolean;
  humanActive: boolean;
  controlLoading?: boolean;
  size?: MacOSDockSize;
  onTakeControl: () => Promise<void> | void;
  onNavigate: (url: string) => Promise<void> | void;
  onClose: () => Promise<void> | void;
  className?: string;
}

export function BrowserAppDock({
  url,
  enabled,
  humanActive,
  controlLoading = false,
  size = "mini",
  onTakeControl,
  onNavigate,
  onClose,
  className,
}: BrowserAppDockProps) {
  const [busyAppId, setBusyAppId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeAppId = dockAppIdForUrl(url ?? null);
  const disabled = !enabled || controlLoading;

  async function handleAppClick(appId: string) {
    if (disabled || busyAppId) {
      return;
    }
    setError(null);
    setBusyAppId(appId);
    try {
      if (!humanActive) {
        await onTakeControl();
      }
      if (activeAppId === appId) {
        await onClose();
        return;
      }
      const app = browserDockAppById(appId);
      if (!app) {
        return;
      }
      await onNavigate(app.url);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not open that app");
    } finally {
      setBusyAppId(null);
    }
  }

  return (
    <div
      className={cn("pointer-events-none absolute inset-x-0 bottom-2 z-[5] flex justify-center px-2", className)}
      data-browser-app-dock=""
    >
      <div
        className="pointer-events-auto w-full max-w-[280px]"
        onClick={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
      >
        <MacOSDock
          apps={BROWSER_DOCK_ICON_APPS}
          onAppClick={(appId) => void handleAppClick(appId)}
          openApps={activeAppId ? [activeAppId] : []}
          busyApps={busyAppId ? [busyAppId] : []}
          disabled={disabled}
          size={size}
        />
        {error ? (
          <p className="mt-1 text-center text-[10px] text-red-300" role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}
