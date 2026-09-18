"use client";

import { isTauriRuntime } from "@/lib/tauri-runtime";
import { cn } from "cn";

interface DesktopTitlebarProps {
  className?: string;
}

/**
 * Invisible drag strip for macOS overlay title bars (traffic-light safe area).
 * Only rendered in the Tauri desktop shell.
 */
export function DesktopTitlebar({ className }: DesktopTitlebarProps) {
  if (!isTauriRuntime()) {
    return null;
  }

  return (
    <div
      data-tauri-drag-region
      className={cn("tauri-titlebar shrink-0", className)}
      aria-hidden
    />
  );
}
