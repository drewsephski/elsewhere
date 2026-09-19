"use client";

import { thisMacPhaseLabel, thisMacPhaseTone } from "@/lib/this-mac-status";
import { useThisMacStatus } from "@/hooks/use-this-mac-status";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { cn } from "cn";

interface DesktopTitlebarProps {
  className?: string;
}

function TitlebarStatusPill() {
  const { status } = useThisMacStatus();
  if (!status) {
    return null;
  }
  const tone = thisMacPhaseTone(status.phase);
  return (
    <span
      className={cn(
        "pointer-events-none select-none rounded-full px-2 py-0.5 text-[10px] font-medium tracking-wide",
        tone === "success" && "bg-emerald-500/20 text-emerald-200",
        tone === "warning" && "bg-amber-500/20 text-amber-100",
        tone === "destructive" && "bg-red-500/20 text-red-200",
        tone === "muted" && "bg-muted text-muted-foreground",
        tone === "default" && "bg-muted/60 text-muted-foreground",
      )}
    >
      {thisMacPhaseLabel(status.phase)}
    </span>
  );
}

/**
 * macOS overlay title bar drag strip (traffic-light safe area) + This Mac status.
 * Only rendered in the Tauri desktop shell.
 */
export function DesktopTitlebar({ className }: DesktopTitlebarProps) {
  if (!isTauriRuntime()) {
    return null;
  }

  return (
    <div
      data-tauri-drag-region
      className={cn(
        "tauri-titlebar relative flex shrink-0 items-end justify-center pb-1",
        className,
      )}
    >
      <TitlebarStatusPill />
    </div>
  );
}
