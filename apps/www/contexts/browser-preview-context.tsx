"use client";

import {
  useBrowserPreview,
  type BrowserPreviewFrame,
} from "@/hooks/use-browser-preview";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

interface BrowserPreviewContextValue {
  computerId: string | null;
  enabled: boolean;
  frame: BrowserPreviewFrame | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  pipOpen: boolean;
  openPip: () => void;
  closePip: () => void;
  pipDismissed: boolean;
  dismissPipForSession: () => void;
  resetPipDismissal: () => void;
}

const BrowserPreviewContext = createContext<BrowserPreviewContextValue | null>(null);

export function BrowserPreviewProvider({
  computerId,
  enabled,
  sessionKey,
  children,
}: {
  computerId: string | null;
  enabled: boolean;
  /** Changes when a new run starts so PiP can auto-appear again after dismiss. */
  sessionKey?: string | null;
  children: ReactNode;
}) {
  const { frame, loading, error, refresh } = useBrowserPreview(computerId, enabled);
  const [pipOpen, setPipOpen] = useState(false);
  const [pipDismissed, setPipDismissed] = useState(false);
  const lastSessionKey = useRef<string | null | undefined>(sessionKey);

  useEffect(() => {
    if (lastSessionKey.current !== sessionKey) {
      lastSessionKey.current = sessionKey;
      setPipDismissed(false);
      setPipOpen(false);
    }
  }, [sessionKey]);

  useEffect(() => {
    if (!enabled) {
      setPipOpen(false);
    }
  }, [enabled]);

  const openPip = useCallback(() => {
    setPipDismissed(false);
    setPipOpen(true);
  }, []);

  const closePip = useCallback(() => {
    setPipOpen(false);
  }, []);

  const dismissPipForSession = useCallback(() => {
    setPipDismissed(true);
    setPipOpen(false);
  }, []);

  const resetPipDismissal = useCallback(() => {
    setPipDismissed(false);
  }, []);

  const value = useMemo(
    () => ({
      computerId,
      enabled,
      frame,
      loading,
      error,
      refresh,
      pipOpen,
      openPip,
      closePip,
      pipDismissed,
      dismissPipForSession,
      resetPipDismissal,
    }),
    [
      computerId,
      enabled,
      frame,
      loading,
      error,
      refresh,
      pipOpen,
      openPip,
      closePip,
      pipDismissed,
      dismissPipForSession,
      resetPipDismissal,
    ],
  );

  return (
    <BrowserPreviewContext.Provider value={value}>{children}</BrowserPreviewContext.Provider>
  );
}

export function useBrowserPreviewContext(): BrowserPreviewContextValue {
  const ctx = useContext(BrowserPreviewContext);
  if (!ctx) {
    throw new Error("useBrowserPreviewContext must be used within BrowserPreviewProvider");
  }
  return ctx;
}

export function useOptionalBrowserPreviewContext(): BrowserPreviewContextValue | null {
  return useContext(BrowserPreviewContext);
}
