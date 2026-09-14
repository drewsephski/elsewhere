"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, ComputerSummary, RunSummary } from "@/lib/api-types";
import { useRunEventStream } from "@/hooks/use-run-event-stream";
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

  const streamRunId =
    variant !== "minimal" &&
    activeRun &&
    (activeRun.status === "queued" || activeRun.status === "running")
      ? activeRun.runId
      : null;
  const { timeline } = useRunEventStream(streamRunId);
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
          throw new Error("Could not load computer");
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
    return (
      <section
        className={cn("border-b border-border/60 pb-3 pt-2", className)}
        aria-labelledby="computer-panel-title"
      >
        <div className="flex items-center justify-between gap-2 px-1">
          <h2 id="computer-panel-title" className="text-sm font-semibold">
            Computer
          </h2>
          <Link
            href="/app/computers"
            className="text-xs text-muted-foreground underline-offset-2 hover:underline"
          >
            Manage
          </Link>
        </div>

        {!bot?.computerId ? (
          <p className="mt-2 px-1 text-xs leading-relaxed text-muted-foreground">
            Assign a computer in Settings so your bot can keep files between assignments.
          </p>
        ) : (
          <div className="mt-1.5 space-y-2 px-1">
            <div className="flex items-start gap-3 rounded-xl px-2 py-1.5 transition-colors hover:bg-white/70">
              <span
                className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary ring-1 ring-primary/15"
                aria-hidden
              >
                <Monitor className="size-4" />
              </span>
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium">{displayName}</p>
                <p className="text-xs text-muted-foreground">{readyLabel}</p>
              </div>
            </div>
            {bot.computerId ? <ComputerBrowserPreview className="!mt-0" /> : null}
          </div>
        )}

        {error ? (
          <p className="mt-2 px-1 text-xs text-red-600" role="alert">
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

      <div className="mt-3 overflow-hidden rounded-xl border border-border/80 bg-[#1a1625] text-white shadow-inner">
        <div className="flex items-center gap-1.5 border-b border-white/10 px-3 py-2">
          <span className="size-2.5 rounded-full bg-[#ff5f57]" aria-hidden />
          <span className="size-2.5 rounded-full bg-[#febc2e]" aria-hidden />
          <span className="size-2.5 rounded-full bg-[#28c840]" aria-hidden />
          <span className="ml-2 truncate text-[11px] text-white/60">
            {displayName ?? "No computer assigned"}
          </span>
        </div>
        <div className="space-y-3 p-4">
          {!bot?.computerId ? (
            <p className="text-sm text-white/75">
              Assign a computer in bot settings so your bot can keep files between assignments.
            </p>
          ) : (
            <>
              <div className="flex items-center gap-2 text-sm">
                <Monitor className="size-4 text-violet-300" aria-hidden />
                <span>{readyLabel}</span>
              </div>
              {activeRun ? (
                <div className="rounded-lg bg-white/5 p-3 text-sm">
                  <p className="text-[11px] font-medium uppercase tracking-wide text-violet-200/90">
                    Current assignment
                  </p>
                  <p className="mt-1 line-clamp-3 text-white/90">{activeRun.task}</p>
                  <p className="mt-2 text-xs text-white/55">
                    {workStatus(activeRun.status)}
                    {latestActivity ? ` · ${latestActivity}` : null}
                  </p>
                </div>
              ) : (
                <p className="text-sm text-white/65">Idle — waiting for your next message.</p>
              )}
              {bot.computerId ? (
                <ComputerBrowserPreview className="!mt-0" />
              ) : null}
            </>
          )}
        </div>
      </div>
      {error ? (
        <p className="mt-2 text-xs text-red-600" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
