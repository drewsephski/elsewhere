"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ProviderStatus } from "@/lib/api-types";
import { useCallback, useEffect, useState } from "react";

type LoginChallenge = { loginId: string; authUrl: string; userCode: string };

async function readResponse<T>(response: Response): Promise<T> {
  const body = await response.json();
  if (!response.ok) throw new Error(body.error ?? "Could not connect to Elsewhere");
  return body as T;
}

export function ProviderStatusCard() {
  const [status, setStatus] = useState<ProviderStatus | null>(null);
  const [challenge, setChallenge] = useState<LoginChallenge | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const next = await readResponse<ProviderStatus>(await cloudHostFetch("/v1/providers/status"));
      setStatus(next);
      setError(null);
      if (next.chatgptConnected) setChallenge(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Connection check failed");
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    if (!challenge) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const expires = Date.now() + 10 * 60_000;
    async function check() {
      try {
        const result = await readResponse<{ connected: boolean }>(
          await cloudHostFetch("/v1/providers/codex/login/status"),
        );
        if (stopped) return;
        if (result.connected) { setChallenge(null); await load(); return; }
        if (Date.now() >= expires) {
          setChallenge(null);
          setError("Sign-in expired. Connect ChatGPT again to get a new code.");
          return;
        }
      } catch (err) {
        if (!stopped) setError(err instanceof Error ? err.message : "Connection check failed");
      }
      if (!stopped) timer = setTimeout(() => void check(), 5000);
    }
    timer = setTimeout(() => void check(), 3000);
    return () => { stopped = true; clearTimeout(timer); };
  }, [challenge, load]);

  async function start() {
    setBusy(true);
    setError(null);
    try {
      setChallenge(await readResponse<LoginChallenge>(
        await cloudHostFetch("/v1/providers/codex/login/start", { method: "POST" }),
      ));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start sign-in");
    } finally { setBusy(false); }
  }

  async function cancel() {
    if (!challenge) return;
    setBusy(true);
    try {
      const response = await cloudHostFetch("/v1/providers/codex/login/cancel", {
        method: "POST", body: JSON.stringify({ loginId: challenge.loginId }),
      });
      if (!response.ok && response.status !== 404) throw new Error("Could not cancel sign-in. Try again.");
      setChallenge(null);
      setError(null);
    } catch (err) { setError(err instanceof Error ? err.message : "Cancellation failed"); }
    finally { setBusy(false); }
  }

  return (
    <section className="surface-card" aria-labelledby="chatgpt-heading">
      <div className="flex items-center justify-between gap-4">
        <h2 id="chatgpt-heading" className="text-lg font-medium">Your ChatGPT connection</h2>
        <button type="button" onClick={() => void load()} className="text-sm underline underline-offset-4">Check connection</button>
      </div>
      <p className="mt-2 text-sm text-muted-foreground">
        Power your bots with your ChatGPT plan. Your connection stays on the computer running Elsewhere, so you can come back to your work later.
      </p>
      <div className="mt-5 flex items-center gap-2 text-sm" role="status">
        <span className={`size-2 rounded-full ${status?.chatgptConnected ? "bg-emerald-600" : "bg-amber-500"}`} />
        {status ? (status.chatgptConnected ? `Connected${status.chatgptPlanType ? ` · ${status.chatgptPlanType}` : ""}` : "Connect ChatGPT to get started") : "Checking connection…"}
      </div>
      <p className="mt-2 text-xs text-muted-foreground">Uses your Codex allowance. Elsewhere never switches to paid API usage automatically.</p>
      {!status?.chatgptConnected && status?.codexLoginAllowed && !challenge ? (
        <button type="button" disabled={busy} onClick={() => void start()} className="mt-5 rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground disabled:opacity-50">
          {busy ? "Preparing sign-in…" : "Connect ChatGPT"}
        </button>
      ) : null}
      {status && !status.chatgptConnected && !status.codexLoginAllowed ? (
        <p className="mt-4 text-sm text-muted-foreground">ChatGPT sign-in needs to be enabled by the person hosting Elsewhere.</p>
      ) : null}
      {challenge ? (
        <div className="mt-5 rounded-xl border border-border bg-background p-4">
          <p className="text-sm">Open ChatGPT sign-in and enter this one-time code.</p>
          <p className="my-3 select-all font-mono text-2xl tracking-widest">{challenge.userCode}</p>
          <div className="flex items-center gap-4">
            <a href={challenge.authUrl} target="_blank" rel="noopener noreferrer" className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground">Continue to ChatGPT</a>
            <button type="button" disabled={busy} onClick={() => void cancel()} className="text-sm underline">Cancel</button>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">Waiting for you to finish sign-in. You may need to enable device code sign-in in your ChatGPT security settings.</p>
        </div>
      ) : null}
      {error ? <p className="mt-4 text-sm text-red-700" role="alert">{error}</p> : null}
    </section>
  );
}
