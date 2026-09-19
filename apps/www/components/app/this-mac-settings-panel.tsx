"use client";

import { ThisMacControls } from "@/components/app/this-mac-controls";
import { desktopCompanion } from "@/lib/desktop-companion";

export function ThisMacSettingsPanel() {
  if (!desktopCompanion.isAvailable()) {
    return (
      <p className="text-[13px] text-muted-foreground">
        This Mac settings are only available in the Elsewhere desktop app.
      </p>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-[13px] leading-relaxed text-muted-foreground">
        Pause stops new work on this Mac and releases the live host connection. Your device
        credential stays in the Keychain until you disconnect.
      </p>
      <ThisMacControls layout="stack" />
    </div>
  );
}
