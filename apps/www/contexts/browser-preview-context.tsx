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

export interface BrowserPreviewPipPosition {
  x: number;
  y: number;
}

interface BrowserPreviewContextValue {
  computerId: string | null;
  enabled: boolean;
  frame: BrowserPreviewFrame | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  navigateBrowser: (url: string) => Promise<void>;
  pipOpen: boolean;
  openPip: () => void;
  closePip: () => void;
  pipDismissed: boolean;
  dismissPipForSession: () => void;
  resetPipDismissal: () => void;
  pipPosition: BrowserPreviewPipPosition | null;
  setPipPosition: (position: BrowserPreviewPipPosition | null) => void;
  dockPip: () => void;
}

const BrowserPreviewContext = createContext<BrowserPreviewContextValue | null>(null);

export function BrowserPreviewProvider({
  computerId,
  enabled,
  sessionKey,
  refreshGeneration = 0,
  children,
}: {
  computerId: string | null;
  enabled: boolean;
  /** Changes when a new run starts so PiP can auto-appear again after dismiss. */
  sessionKey?: string | null;
  /** Bumped when browser tool activity is observed on the active run SSE stream. */
  refreshGeneration?: number;
  children: ReactNode;
}) {
  const { frame, loading, error, refresh, navigateBrowser } = useBrowserPreview(
    computerId,
    enabled,
    refreshGeneration,
  );
  const [pipOpen, setPipOpen] = useState(false);
  const [pipDismissed, setPipDismissed] = useState(false);
  const [pipPosition, setPipPosition] = useState<BrowserPreviewPipPosition | null>(null);
  const lastSessionKey = useRef<string | null | undefined>(sessionKey);

  useEffect(() => {
    if (lastSessionKey.current !== sessionKey) {
      lastSessionKey.current = sessionKey;
      setPipDismissed(false);
      setPipOpen(false);
      setPipPosition(null);
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

  const dockPip = useCallback(() => {
    setPipOpen(false);
    setPipPosition(null);
  }, []);

  const value = useMemo(
    () => ({
      computerId,
      enabled,
      frame,
      loading,
      error,
      refresh,
      navigateBrowser,
      pipOpen,
      openPip,
      closePip,
      pipDismissed,
      dismissPipForSession,
      resetPipDismissal,
      pipPosition,
      setPipPosition,
      dockPip,
    }),
    [
      computerId,
      enabled,
      frame,
      loading,
      error,
      refresh,
      navigateBrowser,
      pipOpen,
      openPip,
      closePip,
      pipDismissed,
      dismissPipForSession,
      resetPipDismissal,
      pipPosition,
      dockPip,
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
