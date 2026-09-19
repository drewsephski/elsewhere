"use client";

import { desktopCompanion } from "@/lib/desktop-companion";
import type { ThisMacStatusSnapshot } from "@/lib/this-mac-status";
import { useCallback, useEffect, useState } from "react";

const POLL_MS = 4000;

export function useThisMacStatus(enabled = desktopCompanion.isAvailable()) {
  const [status, setStatus] = useState<ThisMacStatusSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(enabled);

  const refresh = useCallback(async () => {
    if (!enabled) {
      setStatus(null);
      setLoading(false);
      return;
    }
    try {
      const next = await desktopCompanion.getThisMacStatus();
      setStatus(next);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not read This Mac status");
    } finally {
      setLoading(false);
    }
  }, [enabled]);

  useEffect(() => {
    void refresh();
    if (!enabled) {
      return;
    }
    const timer = window.setInterval(() => void refresh(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [enabled, refresh]);

  return { status, error, loading, refresh };
}
