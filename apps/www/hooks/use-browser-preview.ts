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
  version: number | null;
}

interface BrowserPreviewJson {
  available: boolean;
  url?: string | null;
  title?: string | null;
  contentType?: string | null;
  imageBase64?: string | null;
  capturedAt?: string | null;
  version?: number | null;
}

const RECOVERY_POLL_MS = 30_000;
const INITIAL_POLL_MS = 8_000;
const FETCH_TIMEOUT_MS = 90_000;

function isAbortError(err: unknown): boolean {
  return err instanceof Error && err.name === "AbortError";
}

export function useBrowserPreview(
  computerId: string | null,
  enabled: boolean,
  refreshGeneration = 0,
) {
  const [frame, setFrame] = useState<BrowserPreviewFrame | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasFrame = useRef(false);
  const etagRef = useRef<string | null>(null);
  const fetchSeqRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const refreshGenerationRef = useRef(refreshGeneration);
  const pendingRefreshRef = useRef(false);

  const fetchFrame = useCallback(async () => {
    if (!computerId || !enabled) {
      return;
    }

    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    const seq = ++fetchSeqRef.current;
    const timeoutId = window.setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);

    if (!hasFrame.current) {
      setLoading(true);
    }
    try {
      const headers: HeadersInit = {};
      if (etagRef.current) {
        headers["If-None-Match"] = etagRef.current;
      }
      const response = await cloudHostFetch(
        `/v1/computers/${encodeURIComponent(computerId)}/browser-preview`,
        { headers, signal: controller.signal },
      );
      if (seq !== fetchSeqRef.current) {
        return;
      }
      if (response.status === 304) {
        setError(null);
        return;
      }
      if (!response.ok) {
        const body = await response.text();
        throw new Error(body || "Could not load browser preview");
      }
      const nextEtag = response.headers.get("ETag");
      if (nextEtag) {
        etagRef.current = nextEtag;
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
        version: data.version ?? null,
      });
      setError(null);
    } catch (err) {
      if (seq !== fetchSeqRef.current) {
        return;
      }
      if (isAbortError(err)) {
        setError(
          "Timed out loading browser preview. Your computer may still be starting — we will keep trying.",
        );
        return;
      }
      setError(err instanceof Error ? err.message : "Preview unavailable");
    } finally {
      window.clearTimeout(timeoutId);
      if (seq === fetchSeqRef.current) {
        setLoading(false);
        if (pendingRefreshRef.current) {
          pendingRefreshRef.current = false;
          void fetchFrame();
        }
      }
    }
  }, [computerId, enabled]);

  useEffect(() => {
    refreshGenerationRef.current = refreshGeneration;
  }, [refreshGeneration]);

  useEffect(() => {
    if (!computerId || !enabled) {
      abortRef.current?.abort();
      fetchSeqRef.current += 1;
      pendingRefreshRef.current = false;
      hasFrame.current = false;
      etagRef.current = null;
      setFrame(null);
      setError(null);
      setLoading(false);
      return;
    }

    void fetchFrame();
    return () => {
      abortRef.current?.abort();
    };
  }, [computerId, enabled, fetchFrame]);

  useEffect(() => {
    if (!computerId || !enabled || refreshGeneration === 0) {
      return;
    }
    if (loading) {
      pendingRefreshRef.current = true;
      return;
    }
    void fetchFrame();
  }, [computerId, enabled, refreshGeneration, loading, fetchFrame]);

  useEffect(() => {
    if (!computerId || !enabled) {
      return;
    }
    const pollMs = frame?.imageDataUrl ? RECOVERY_POLL_MS : INITIAL_POLL_MS;
    const timer = window.setInterval(() => {
      void fetchFrame();
    }, pollMs);
    return () => window.clearInterval(timer);
  }, [computerId, enabled, fetchFrame, frame?.imageDataUrl]);

  return { frame, loading, error, refresh: fetchFrame };
}
