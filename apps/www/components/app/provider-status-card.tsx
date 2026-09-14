"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ProviderStatus } from "@/lib/api-types";
import { useCallback, useEffect, useState } from "react";

export function ProviderStatusCard() {
  const [status, setStatus] = useState<ProviderStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/providers/status");
      if (!response.ok) {
        throw new Error(`Provider status failed (${response.status})`);
      }
      setStatus((await response.json()) as ProviderStatus);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load provider status");
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section className="surface-card">
      <div className="flex items-center justify-between gap-4">
        <h2 className="text-sm font-medium uppercase tracking-[0.15em]">Runner provider</h2>
        <button
          type="button"
          onClick={() => void load()}
          className="text-xs text-brand-dark/60 underline-offset-4 hover:underline"
        >
          Refresh
        </button>
      </div>
      <p className="mt-2 text-xs text-brand-dark/60">
        ChatGPT connection is runner-local (Codex app-server). Elsewhere does not store Codex OAuth
        tokens.
      </p>
      {error ? (
        <p className="mt-4 text-sm text-red-700" role="alert">
          {error}
        </p>
      ) : null}
      {status ? (
        <dl className="mt-4 grid gap-2 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-brand-dark/50">Codex installed</dt>
            <dd>{status.codexInstalled ? "Yes" : "No"}</dd>
          </div>
          <div>
            <dt className="text-brand-dark/50">ChatGPT connected</dt>
            <dd>{status.chatgptConnected ? "Yes" : "No"}</dd>
          </div>
          <div>
            <dt className="text-brand-dark/50">Plan</dt>
            <dd>{status.chatgptPlanType ?? "—"}</dd>
          </div>
          <div>
            <dt className="text-brand-dark/50">Host engine default</dt>
            <dd>{status.preferredEngine}</dd>
          </div>
          <div>
            <dt className="text-brand-dark/50">API fallback configured</dt>
            <dd>{status.apiFallbackConfigured ? "Yes" : "No"}</dd>
          </div>
          <div>
            <dt className="text-brand-dark/50">Default model</dt>
            <dd>{status.defaultModel}</dd>
          </div>
        </dl>
      ) : (
        !error && <p className="mt-4 text-sm text-brand-dark/50">Loading…</p>
      )}
      {status?.codexLoginAllowed ? (
        <CodexLoginPanel onUpdated={load} connected={status.chatgptConnected} />
      ) : null}
    </section>
  );
}

function CodexLoginPanel({
  connected,
  onUpdated,
}: {
  connected: boolean;
  onUpdated: () => void;
}) {
  const [loginId, setLoginId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleStartLogin() {
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/providers/codex/login/start", {
        method: "POST",
      });
      if (!response.ok) {
        throw new Error(`Login start failed (${response.status})`);
      }
      const body = (await response.json()) as { loginId: string; authUrl: string };
      setLoginId(body.loginId);
      window.open(body.authUrl, "_blank", "noopener,noreferrer");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    }
  }

  useEffect(() => {
    if (!loginId) {
      return;
    }
    const timer = setInterval(() => {
      void cloudHostFetch("/v1/providers/codex/login/status")
        .then(async (response) => {
          if (!response.ok) {
            return;
          }
          const body = (await response.json()) as { connected: boolean };
          if (body.connected) {
            setLoginId(null);
            onUpdated();
          }
        })
        .catch(() => undefined);
    }, 3000);
    return () => clearInterval(timer);
  }, [loginId, onUpdated]);

  return (
    <div className="mt-4 border-t border-brand-dark/10 pt-4">
      <p className="text-xs text-brand-dark/60">
        {connected
          ? "Runner reports an active ChatGPT subscription session."
          : "Connect ChatGPT on this runner (opens Codex OAuth in a new tab)."}
      </p>
      {!connected ? (
        <button
          type="button"
          onClick={() => void handleStartLogin()}
          className="mt-3 rounded-lg border border-border bg-card px-3 py-1.5 text-sm shadow-sm transition-colors hover:bg-accent"
        >
          Connect ChatGPT on runner
        </button>
      ) : null}
      {error ? (
        <p className="mt-2 text-sm text-red-700" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
