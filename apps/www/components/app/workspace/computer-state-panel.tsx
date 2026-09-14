"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, ComputerSummary, RunSummary } from "@/lib/api-types";
import { workStatus } from "@/lib/work-events";
import { cn } from "cn";
import { FolderOpen, Monitor, Terminal } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";

interface ComputerStatePanelProps {
  bot: BotSummary | null;
  activeRun: RunSummary | null;
  className?: string;
}

export function ComputerStatePanel({ bot, activeRun, className }: ComputerStatePanelProps) {
  const [computer, setComputer] = useState<ComputerSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const displayName = computer?.displayName ?? bot?.computerId ? "Computer" : null;

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
                <span>{computer?.providerMetadata.provisioned ? "Ready for work" : "Will provision on first use"}</span>
              </div>
              {activeRun ? (
                <div className="rounded-lg bg-white/5 p-3 text-sm">
                  <p className="text-[11px] font-medium uppercase tracking-wide text-violet-200/90">
                    Current assignment
                  </p>
                  <p className="mt-1 line-clamp-3 text-white/90">{activeRun.task}</p>
                  <p className="mt-2 text-xs text-white/55">{workStatus(activeRun.status)}</p>
                </div>
              ) : (
                <p className="text-sm text-white/65">Idle — waiting for your next message.</p>
              )}
              <ul className="grid grid-cols-3 gap-2 text-center text-[10px] text-white/55">
                <li className="rounded-lg bg-white/5 px-2 py-2">
                  <FolderOpen className="mx-auto mb-1 size-4 text-white/70" aria-hidden />
                  Files
                </li>
                <li className="rounded-lg bg-white/5 px-2 py-2 opacity-60">
                  <Monitor className="mx-auto mb-1 size-4" aria-hidden />
                  Browser
                  <span className="mt-0.5 block text-[9px]">Soon</span>
                </li>
                <li className="rounded-lg bg-white/5 px-2 py-2">
                  <Terminal className="mx-auto mb-1 size-4 text-white/70" aria-hidden />
                  Terminal
                </li>
              </ul>
            </>
          )}
        </div>
      </div>
      {error ? (
        <p className="mt-2 text-xs text-red-600" role="alert">{error}</p>
      ) : null}
      <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
        Live browser view will replace this panel when streaming is available. Status reflects real work on the server.
      </p>
    </section>
  );
}
