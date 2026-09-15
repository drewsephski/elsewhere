"use client";

import {
  fetchBrowserControlState,
  heartbeatBrowserControl,
  returnBrowserControl,
  takeBrowserControl,
  type BrowserControlState,
} from "@/lib/browser-control";
import { useCallback, useEffect, useState } from "react";

const CONTROL_STATE_POLL_MS = 15_000;
const HUMAN_LEASE_HEARTBEAT_MS = 30_000;

export function useBrowserHumanControl(computerId: string | null, enabled: boolean) {
  const [control, setControl] = useState<BrowserControlState | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const humanActive = Boolean(control?.youHaveControl);

  const refresh = useCallback(async () => {
    if (!computerId || !enabled) {
      setControl(null);
      return;
    }
    try {
      const next = await fetchBrowserControlState(computerId);
      setControl(next);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Control state unavailable");
    }
  }, [computerId, enabled]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!computerId || !enabled) {
      return;
    }
    const timer = window.setInterval(() => {
      void refresh();
    }, CONTROL_STATE_POLL_MS);
    return () => window.clearInterval(timer);
  }, [computerId, enabled, refresh]);

  useEffect(() => {
    if (!computerId || !enabled || !humanActive) {
      return;
    }

    let cancelled = false;

    async function sendHeartbeat() {
      if (cancelled || !computerId) {
        return;
      }
      try {
        const next = await heartbeatBrowserControl(computerId);
        if (!cancelled) {
          setControl(next);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Control lease heartbeat failed");
          void refresh();
        }
      }
    }

    void sendHeartbeat();
    const timer = window.setInterval(() => {
      void sendHeartbeat();
    }, HUMAN_LEASE_HEARTBEAT_MS);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [computerId, enabled, humanActive, refresh]);

  const handleTakeControl = useCallback(async () => {
    if (!computerId) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const next = await takeBrowserControl(computerId);
      setControl(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not take control");
    } finally {
      setLoading(false);
    }
  }, [computerId]);

  const handleReturnControl = useCallback(async () => {
    if (!computerId) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const next = await returnBrowserControl(computerId);
      setControl(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not return control");
    } finally {
      setLoading(false);
    }
  }, [computerId]);

  return {
    control,
    loading,
    error,
    refresh,
    takeControl: handleTakeControl,
    returnControl: handleReturnControl,
    humanActive,
  };
}
