"use client";

import { ThisMacControls } from "@/components/app/this-mac-controls";
import { Label } from "@/components/ui/label";
import { desktopCompanion } from "@/lib/desktop-companion";
import {
  desktopNotificationsEnabled,
  setDesktopNotificationsEnabled,
} from "@/lib/desktop-notifications-pref";
import { useState } from "react";

export function ThisMacSettingsPanel() {
  const [notify, setNotify] = useState(() => desktopNotificationsEnabled());
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
      <div className="flex items-center justify-between gap-3 border-t border-border pt-4">
        <div>
          <Label htmlFor="desktop-notify" className="text-[13px]">
            Desktop notifications
          </Label>
          <p className="text-[12px] text-muted-foreground">
            Alert when new approvals need you (requires macOS notification permission).
          </p>
        </div>
        <input
          id="desktop-notify"
          type="checkbox"
          checked={notify}
          onChange={(event) => {
            const next = event.target.checked;
            setNotify(next);
            setDesktopNotificationsEnabled(next);
            if (next && typeof Notification !== "undefined" && Notification.permission === "default") {
              void Notification.requestPermission();
            }
          }}
          className="size-4 accent-primary"
        />
      </div>
    </div>
  );
}
