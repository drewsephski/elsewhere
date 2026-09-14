"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ProviderStatus } from "@/lib/api-types";
import { useCallback, useEffect, useRef, useState } from "react";

/** Slightly above cloud-host status probe budget so the UI gets a response or a clear timeout. */
const PROVIDER_STATUS_TIMEOUT_MS = 40_000;

export type ProviderStatusLoadPhase = "idle" | "loading" | "ready" | "error";

type LoginChallenge = { loginId: string; authUrl: string; userCode: string };

async function readCloudJson<T>(response: Response): Promise<T> {
  const text = await response.text();
  let body: unknown = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    if (!response.ok) {
      throw new Error(
        text.trim() || `Could not connect to Elsewhere (${response.status})`,
      );
    }
    throw new Error("Could not read workspace response");
  }
  if (!response.ok) {
    const message =
      body &&
      typeof body === "object" &&
      "error" in body &&
      typeof (body as { error: unknown }).error === "string"
        ? (body as { error: string }).error
        : `Could not connect to Elsewhere (${response.status})`;
    throw new Error(message);
  }
  return body as T;
}

export function useProviderStatus() {
  const [status, setStatus] = useState<ProviderStatus | null>(null);
  const [phase, setPhase] = useState<ProviderStatusLoadPhase>("idle");
  const [challenge, setChallenge] = useState<LoginChallenge | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loadGeneration = useRef(0);

  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    setPhase("loading");
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/providers/status", {
        timeoutMs: PROVIDER_STATUS_TIMEOUT_MS,
      });
      const next = await readCloudJson<ProviderStatus>(response);
      if (generation !== loadGeneration.current) {
        return;
      }
      setStatus(next);
      setPhase("ready");
      if (next.chatgptConnected) {
        setChallenge(null);
      }
    } catch (err) {
      if (generation !== loadGeneration.current) {
        return;
      }
      setPhase("error");
      setError(err instanceof Error ? err.message : "Connection check failed");
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!challenge) {
      return;
    }
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const expires = Date.now() + 10 * 60_000;

    async function checkLogin() {
      try {
        const response = await cloudHostFetch("/v1/providers/codex/login/status", {
          timeoutMs: 20_000,
        });
        const result = await readCloudJson<{ connected: boolean }>(response);
        if (stopped) {
          return;
        }
        if (result.connected) {
          setChallenge(null);
          await load();
          return;
        }
        if (Date.now() >= expires) {
          setChallenge(null);
          setError("Sign-in expired. Connect ChatGPT again to get a new code.");
          return;
        }
      } catch (err) {
        if (!stopped) {
          setError(err instanceof Error ? err.message : "Connection check failed");
        }
      }
      if (!stopped) {
        timer = setTimeout(() => void checkLogin(), 5000);
      }
    }

    timer = setTimeout(() => void checkLogin(), 3000);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [challenge, load]);

  const handleConnect = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/providers/codex/login/start", {
        method: "POST",
        timeoutMs: 60_000,
      });
      setChallenge(await readCloudJson<LoginChallenge>(response));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start sign-in");
    } finally {
      setBusy(false);
    }
  }, []);

  const cancelLogin = useCallback(async () => {
    if (!challenge) {
      return;
    }
    setBusy(true);
    try {
      const response = await cloudHostFetch("/v1/providers/codex/login/cancel", {
        method: "POST",
        body: JSON.stringify({ loginId: challenge.loginId }),
        timeoutMs: 20_000,
      });
      if (!response.ok && response.status !== 404) {
        throw new Error("Could not cancel sign-in. Try again.");
      }
      setChallenge(null);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Cancellation failed");
    } finally {
      setBusy(false);
    }
  }, [challenge]);

  const connected = Boolean(status?.chatgptConnected);
  const checking = phase === "loading" || phase === "idle";
  const checkFailed = phase === "error";
  const canConnect = Boolean(
    status && !connected && status.codexLoginAllowed && !challenge && !checking,
  );
  const connectBlocked = Boolean(status && !connected && !status.codexLoginAllowed);
  return {
    status,
    challenge,
    busy,
    error,
    connected,
    checking,
    checkFailed,
    canConnect,
    connectBlocked,
    load,
    handleConnect,
    cancelLogin,
  };
}
