"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { useCallback, useEffect, useRef, useState } from "react";

export interface BrowserPreviewFrame {
  available: boolean;
  url: string | null;
  title: string | null;
  contentType: string | null;
  imageDataUrl: string | null;
  capturedAt: string | null;
}

interface BrowserPreviewJson {
  available: boolean;
  url?: string | null;
  title?: string | null;
  contentType?: string | null;
  imageBase64?: string | null;
  capturedAt?: string | null;
}

const POLL_MS = 2_000;

export function useBrowserPreview(computerId: string | null, enabled: boolean) {
  const [frame, setFrame] = useState<BrowserPreviewFrame | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inFlight = useRef(false);
  const hasFrame = useRef(false);

  const fetchFrame = useCallback(async () => {
    if (!computerId || !enabled) {
      return;
    }
    if (inFlight.current) {
      return;
    }
    inFlight.current = true;
    if (!hasFrame.current) {
      setLoading(true);
    }
    try {
      const response = await cloudHostFetch(
        `/v1/computers/${encodeURIComponent(computerId)}/browser-preview`,
      );
      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || "Could not load browser preview");
      }
      const data: BrowserPreviewJson = await response.json();
      const contentType = data.contentType ?? "image/jpeg";
      const imageDataUrl =
        data.available && data.imageBase64
          ? `data:${contentType};base64,${data.imageBase64}`
          : null;
      hasFrame.current = true;
      setFrame({
        available: data.available,
        url: data.url ?? null,
        title: data.title ?? null,
        contentType: data.contentType ?? null,
        imageDataUrl,
        capturedAt: data.capturedAt ?? null,
      });
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Preview unavailable");
    } finally {
      inFlight.current = false;
      setLoading(false);
    }
  }, [computerId, enabled]);

  useEffect(() => {
    if (!computerId || !enabled) {
      hasFrame.current = false;
      setFrame(null);
      setError(null);
      setLoading(false);
      return;
    }

    void fetchFrame();
    const timer = window.setInterval(() => {
      void fetchFrame();
    }, POLL_MS);
    return () => window.clearInterval(timer);
  }, [computerId, enabled, fetchFrame]);

  return { frame, loading, error, refresh: fetchFrame };
}
