"use client";

import { cloudHostErrorMessage, cloudHostFetch, isRunnerUnreachableStatus, readCloudApiErrorBody } from "@/lib/cloud-api";
import type { BotSummary, ComputerSummary, RunSummary } from "@/lib/api-types";
import { useActiveRun } from "@/contexts/active-run-context";
import { workStatus } from "@/lib/work-events";
import { cn } from "cn";
import { Monitor } from "@/components/icons/lucide";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { ComputerBrowserPreview } from "./computer-browser-preview";

interface ComputerStatePanelProps {
  bot: BotSummary | null;
  activeRun: RunSummary | null;
  className?: string;
  variant?: "default" | "minimal";
}

export function ComputerStatePanel({
  bot,
  activeRun,
  className,
  variant = "default",
}: ComputerStatePanelProps) {
  const [computer, setComputer] = useState<ComputerSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { timeline } = useActiveRun();
  const latestActivity = useMemo(() => {
    if (variant === "minimal") {
      return null;
    }
    for (let i = timeline.length - 1; i >= 0; i -= 1) {
      const item = timeline[i];
      if (item.kind === "text") {
        return item.text;
      }
    }
    return null;
  }, [timeline, variant]);

  useEffect(() => {
    if (!bot?.computerId) {
      setComputer(null);
      return;
    }
    const controller = new AbortController();
    cloudHostFetch("/v1/computers", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          const body = await readCloudApiErrorBody(response);
          if (isRunnerUnreachableStatus(response.status, body)) {
            throw new Error(
              body?.error ??
                "Workspace runner is temporarily unreachable. Your saved ChatGPT connection has not been changed.",
            );
          }
          throw new Error(await cloudHostErrorMessage(response, "Could not load computer"));
        }
        const items: ComputerSummary[] = await response.json();
        setComputer(items.find((item) => item.id === bot.computerId) ?? null);
        setError(null);
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Computer unavailable");
        }
      });
    return () => controller.abort();
  }, [bot?.computerId]);

  const displayName = computer?.displayName ?? (bot?.computerId ? "Computer" : null);
  const readyLabel = computer?.providerMetadata.provisioned
    ? "Ready for work"
    : "Provisions on first use";
  if (variant === "minimal") {
    const screenOwner = bot?.name?.trim() ? `${bot.name.trim()}’s screen` : "Screen";
    return (
      <section className={cn("pb-1", className)} aria-labelledby="computer-panel-title">
        <h2 id="computer-panel-title" className="sr-only">
          Computer
        </h2>

        {!bot?.computerId ? (
          <div className="flex aspect-[16/10] flex-col items-center justify-center gap-2 rounded-xl border border-border bg-card px-5 text-center">
            <span
              className="flex size-9 items-center justify-center rounded-xl bg-surface-active text-muted-foreground"
              aria-hidden
            >
              <Monitor className="size-4" />
            </span>
            <p className="text-[11px] leading-snug text-muted-foreground">
              Assign a computer in Settings so your bot can keep files between assignments.
            </p>
            <Link
              href="/app/computers"
              className="text-[11px] font-medium text-foreground underline-offset-2 hover:underline"
            >
              Manage computers
            </Link>
          </div>
        ) : (
          <>
            <ComputerBrowserPreview
              caption={
                <p className="mt-2 text-center text-[11px] text-muted-foreground">{screenOwner}</p>
              }
            />
            <div className="mt-2 flex items-center justify-between gap-2 px-0.5 text-[11px] text-muted-foreground">
              <div className="flex min-w-0 items-center gap-1.5">
                <Monitor className="size-3.5 shrink-0 opacity-80" aria-hidden />
                <span className="truncate">
                  {displayName} · {readyLabel}
                </span>
              </div>
              <Link
                href="/app/computers"
                className="shrink-0 underline-offset-2 hover:text-foreground hover:underline"
              >
                Manage
              </Link>
            </div>
          </>
        )}

        {error ? (
          <p className="mt-2 px-0.5 text-[11px] text-destructive" role="alert">
            {error}
          </p>
        ) : null}
      </section>
    );
  }

  return (
    <section className={cn("workspace-rail-card", className)} aria-labelledby="computer-panel-title">
      <div className="flex items-center justify-between gap-2">
        <h2 id="computer-panel-title" className="text-sm font-semibold">
          {bot?.name ? `${bot.name}'s computer` : "Computer"}
        </h2>
        <Link
          href="/app/computers"
          className="text-xs text-muted-foreground underline-offset-2 hover:underline"
        >
          Manage
        </Link>
      </div>

      <div className="mt-3 overflow-hidden rounded-xl border border-border bg-[#101010] text-foreground">
        <div className="flex items-center gap-1.5 border-b border-border px-3 py-2">
          <Monitor className="size-3.5 text-muted-foreground" aria-hidden />
          <span className="truncate text-[11px] text-muted-foreground">
            {displayName ?? "No computer assigned"}
          </span>
        </div>
        <div className="space-y-3 p-4">
          {!bot?.computerId ? (
            <p className="text-sm text-muted-foreground">
              Assign a computer in bot settings so your bot can keep files between assignments.
            </p>
          ) : (
            <>
              <div className="flex items-center gap-2 text-sm">
                <span className="size-1.5 rounded-full bg-success" aria-hidden />
                <span>{readyLabel}</span>
              </div>
              {activeRun ? (
                <div className="rounded-lg bg-surface-hover p-3 text-sm">
                  <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Current assignment
                  </p>
                  <p className="mt-1 line-clamp-3 text-foreground">{activeRun.task}</p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {workStatus(activeRun.status)}
                    {latestActivity ? ` · ${latestActivity}` : null}
                  </p>
                </div>
              ) : (
                <p className="text-sm text-muted-foreground">Idle — waiting for your next message.</p>
              )}
              {bot.computerId ? (
                <ComputerBrowserPreview className="!mt-0" />
              ) : null}
            </>
          )}
        </div>
      </div>
      {error ? (
        <p className="mt-2 text-xs text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
