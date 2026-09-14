"use client";

import { CodexIcon } from "@/components/icons/codex-icon";
import { CheckCircle2, ExternalLink, Loader2, RefreshCw } from "@/components/icons/lucide";
import { Button, buttonVariants } from "@/components/ui/button";
import { useProviderStatus } from "@/hooks/use-provider-status";
import { cn } from "cn";

interface ProviderStatusCardProps {
  /** Larger layout for empty workspace / mobile sheet. */
  variant?: "panel" | "featured";
  className?: string;
}

export function ProviderStatusCard({ variant = "panel", className }: ProviderStatusCardProps) {
  const {
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
  } = useProviderStatus();

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
            className={cn(
              "font-semibold tracking-tight text-foreground",
              featured ? "text-xl" : "text-lg font-medium",
            )}
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
          disabled={checking}
          aria-label="Refresh connection status"
        >
          <RefreshCw className={cn("size-4", checking && "animate-spin")} aria-hidden />
        </Button>
      </div>

      <div
        className={cn(
          "mt-5 flex items-center gap-2 rounded-xl border px-3 py-2.5 text-sm",
          connected
            ? "border-emerald-200/80 bg-emerald-50/90 text-emerald-950"
            : checkFailed
              ? "border-red-200/80 bg-red-50/80 text-red-950"
              : "border-amber-200/80 bg-amber-50/80 text-amber-950",
        )}
        role="status"
      >
        {checking ? (
          <>
            <Loader2 className="size-4 shrink-0 animate-spin text-muted-foreground" aria-hidden />
            <span>Checking connection…</span>
          </>
        ) : checkFailed ? (
          <>
            <span className="size-2 shrink-0 rounded-full bg-red-500" aria-hidden />
            <span>Could not verify connection — you can retry or connect below</span>
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
            disabled={
              busy ||
              checking ||
              (!canConnect && !(checkFailed && !status))
            }
            onClick={() => {
              if (checkFailed && !status) {
                void load();
                return;
              }
              void handleConnect();
            }}
          >
            {busy ? (
              <Loader2 className="size-4 animate-spin" aria-hidden />
            ) : (
              <CodexIcon className="size-4" />
            )}
            {busy
              ? "Preparing sign-in…"
              : checkFailed && !status
                ? "Retry connection check"
                : "Connect ChatGPT"}
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
              onClick={() => void cancelLogin()}
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
