"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ProviderStatus } from "@/lib/api-types";
import { Button, buttonVariants } from "@/components/ui/button";
import { cn } from "cn";
import { CheckCircle2, ExternalLink, Loader2, RefreshCw, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

type LoginChallenge = { loginId: string; authUrl: string; userCode: string };

async function readResponse<T>(response: Response): Promise<T> {
  const body = await response.json();
  if (!response.ok) throw new Error(body.error ?? "Could not connect to Elsewhere");
  return body as T;
}

interface ProviderStatusCardProps {
  /** Larger layout for empty workspace / mobile sheet. */
  variant?: "panel" | "featured";
  className?: string;
}

export function ProviderStatusCard({ variant = "panel", className }: ProviderStatusCardProps) {
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

  async function handleConnect() {
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

  const connected = Boolean(status?.chatgptConnected);
  const loading = status === null;
  const canConnect = Boolean(status && !connected && status.codexLoginAllowed && !challenge);
  const connectBlocked = Boolean(status && !connected && !status.codexLoginAllowed);

  const featured = variant === "featured";

  return (
    <section
      className={cn(
        featured
          ? "w-full max-w-md rounded-2xl border border-border/80 bg-white/80 p-6 text-left shadow-sm backdrop-blur-sm"
          : "surface-card",
        className,
      )}
      aria-labelledby="chatgpt-heading"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2
            id="chatgpt-heading"
            className={cn("font-semibold tracking-tight text-foreground", featured ? "text-xl" : "text-lg font-medium")}
          >
            {featured ? "Connect ChatGPT" : "Your ChatGPT connection"}
          </h2>
          <p className="mt-1.5 text-sm text-muted-foreground">
            {featured
              ? "Link your ChatGPT plan so bots can run on your subscription allowance—not paid API keys."
              : "Power your bots with your ChatGPT plan. Your connection stays on the computer running Elsewhere."}
          </p>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="shrink-0 text-muted-foreground"
          onClick={() => void load()}
          disabled={loading}
          aria-label="Refresh connection status"
        >
          <RefreshCw className={cn("size-4", loading && "animate-spin")} aria-hidden />
        </Button>
      </div>

      <div
        className={cn(
          "mt-5 flex items-center gap-2 rounded-xl border px-3 py-2.5 text-sm",
          connected
            ? "border-emerald-200/80 bg-emerald-50/90 text-emerald-950"
            : "border-amber-200/80 bg-amber-50/80 text-amber-950",
        )}
        role="status"
      >
        {loading ? (
          <>
            <Loader2 className="size-4 shrink-0 animate-spin text-muted-foreground" aria-hidden />
            <span>Checking connection…</span>
          </>
        ) : connected ? (
          <>
            <CheckCircle2 className="size-4 shrink-0 text-emerald-600" aria-hidden />
            <span>
              Connected
              {status?.chatgptPlanType ? (
                <span className="text-emerald-800/80"> · {status.chatgptPlanType}</span>
              ) : null}
            </span>
          </>
        ) : (
          <>
            <span className="size-2 shrink-0 rounded-full bg-amber-500" aria-hidden />
            <span>Not connected — connect to run bots on your plan</span>
          </>
        )}
      </div>

      {!connected && !challenge ? (
        <div className="mt-4 space-y-2">
          <Button
            type="button"
            size="lg"
            className={cn(
              "w-full gap-2 shadow-sm",
              featured ? "h-11 rounded-full text-base" : "rounded-xl",
            )}
            disabled={busy || loading || !canConnect}
            onClick={() => void handleConnect()}
          >
            {busy ? (
              <Loader2 className="size-4 animate-spin" aria-hidden />
            ) : (
              <Sparkles className="size-4" aria-hidden />
            )}
            {busy ? "Preparing sign-in…" : "Connect ChatGPT"}
          </Button>
          {connectBlocked ? (
            <p className="text-center text-xs text-muted-foreground">
              Sign-in is not enabled on this host. Ask whoever runs Elsewhere to set{" "}
              <code className="rounded bg-muted px-1 py-0.5 text-[0.7rem]">ELSEWHERE_ALLOW_CODEX_LOGIN=1</code>{" "}
              and a profiles directory.
            </p>
          ) : (
            <p className="text-center text-xs text-muted-foreground">
              Uses your Codex allowance. Elsewhere never switches to paid API usage automatically.
            </p>
          )}
        </div>
      ) : null}

      {connected ? (
        <p className="mt-3 text-xs text-muted-foreground">
          Uses your Codex allowance. Elsewhere never switches to paid API usage automatically.
        </p>
      ) : null}

      {challenge ? (
        <div className="mt-5 space-y-4 rounded-xl border border-primary/20 bg-primary/5 p-4">
          <p className="text-sm font-medium text-foreground">Finish signing in to ChatGPT</p>
          <p className="text-sm text-muted-foreground">
            Open ChatGPT, choose device sign-in, and enter this one-time code:
          </p>
          <p
            className="select-all rounded-lg border border-border bg-background py-3 text-center font-mono text-2xl tracking-[0.35em] text-foreground"
            aria-label="Device sign-in code"
          >
            {challenge.userCode}
          </p>
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
            <a
              href={challenge.authUrl}
              target="_blank"
              rel="noopener noreferrer"
              className={cn(
                buttonVariants({ size: "lg" }),
                "inline-flex w-full gap-2 sm:flex-1",
                featured && "rounded-full",
              )}
            >
              <ExternalLink className="size-4" aria-hidden />
              Continue to ChatGPT
            </a>
            <Button
              type="button"
              variant="outline"
              size="lg"
              className="w-full sm:w-auto"
              disabled={busy}
              onClick={() => void cancel()}
            >
              Cancel
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            Waiting for you to finish sign-in. You may need to enable device code sign-in in ChatGPT
            security settings.
          </p>
        </div>
      ) : null}

      {error ? (
        <p className="mt-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-800" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
