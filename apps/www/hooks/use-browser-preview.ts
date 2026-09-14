"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { BrowserPreviewFetchScheduler } from "@/lib/browser-preview-fetch-scheduler";
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
const TOOL_RESULT_RETRY_MS = 450;
const TOOL_RESULT_RETRY_ATTEMPTS = 4;

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
  const schedulerRef = useRef(new BrowserPreviewFetchScheduler());
  const computerIdRef = useRef(computerId);
  const enabledRef = useRef(enabled);

  const cancelInFlight = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
    fetchSeqRef.current += 1;
    schedulerRef.current.reset();
  }, []);

  const executeFetch = useCallback(async () => {
    const activeComputerId = computerIdRef.current;
    if (!activeComputerId || !enabledRef.current) {
      return;
    }

    const seq = fetchSeqRef.current;
    const controller = new AbortController();
    abortRef.current = controller;
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
        `/v1/computers/${encodeURIComponent(activeComputerId)}/browser-preview`,
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
      }
    }
  }, []);

  const requestRefresh = useCallback((): Promise<void> => {
    if (!computerIdRef.current || !enabledRef.current) {
      return Promise.resolve();
    }
    return schedulerRef.current.runCoalesced(executeFetch);
  }, [executeFetch]);

  useEffect(() => {
    computerIdRef.current = computerId;
    enabledRef.current = enabled;
  }, [computerId, enabled]);

  useEffect(() => {
    if (!computerId || !enabled) {
      cancelInFlight();
      hasFrame.current = false;
      etagRef.current = null;
      setFrame(null);
      setError(null);
      setLoading(false);
      return;
    }

    requestRefresh();
    return () => {
      cancelInFlight();
    };
  }, [computerId, enabled, requestRefresh, cancelInFlight]);

  useEffect(() => {
    if (!computerId || !enabled || refreshGeneration === 0) {
      return;
    }
    let cancelled = false;
    let attempt = 0;
    const runBurst = async () => {
      while (!cancelled && attempt < TOOL_RESULT_RETRY_ATTEMPTS) {
        await requestRefresh();
        attempt += 1;
        if (attempt < TOOL_RESULT_RETRY_ATTEMPTS) {
          await new Promise((resolve) => window.setTimeout(resolve, TOOL_RESULT_RETRY_MS));
        }
      }
    };
    void runBurst();
    return () => {
      cancelled = true;
    };
  }, [computerId, enabled, refreshGeneration, requestRefresh]);

  useEffect(() => {
    if (!computerId || !enabled) {
      return;
    }
    const pollMs = frame?.imageDataUrl ? RECOVERY_POLL_MS : INITIAL_POLL_MS;
    const timer = window.setInterval(() => {
      requestRefresh();
    }, pollMs);
    return () => window.clearInterval(timer);
  }, [computerId, enabled, requestRefresh, frame?.imageDataUrl]);

  return { frame, loading, error, refresh: requestRefresh };
}
