"use client";
import { useEffect, useState } from "react";
import Link from "next/link";
import { Bot, ArrowUpRight, Monitor } from "@/components/icons/lucide";
import { cloudHostFetch } from "@/lib/cloud-api";

import {
  presenceLabels,
  type WorkspaceOverview as Overview,
} from "@/lib/workspace-types";

export function WorkspaceOverview({ compact = false }: { compact?: boolean }) {
  const [data, setData] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const controller = new AbortController(); let timer: ReturnType<typeof setTimeout>;
    async function load() {
      try {
        const response = await cloudHostFetch("/v1/workspace", { signal: controller.signal });
        if (!response.ok) throw new Error("Could not refresh your workspace");
        const next: Overview = await response.json();
        if (!controller.signal.aborted) { setData(next); setError(null); }
      } catch (err) { if (!controller.signal.aborted) setError(err instanceof Error ? err.message : "Workspace unavailable"); }
      if (!controller.signal.aborted) timer = setTimeout(() => void load(), 5000);
    }
    void load(); return () => { controller.abort(); clearTimeout(timer); };
  }, []);
  return <div className="space-y-5">
    {error ? <p role="alert" className="text-sm text-amber-700">{error}. Displayed activity may be out of date.</p> : null}
    {data && !data.runnerReady ? <p role="status" className="rounded-xl border border-amber-200 bg-amber-50 p-3 text-sm text-amber-800">Background work is temporarily unavailable. Queued assignments stay saved while the runner reconnects.</p> : null}
    {!compact && data ? <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">{[
      { label: "In progress", count: data.counts.working + data.counts.queued, href: "/app/work" },
      { label: "Waiting for you", count: data.counts.approvals, href: "/app/approvals" },
      { label: "Saved results", count: data.counts.results, href: "/app/results" },
      { label: "Active routines", count: data.counts.routines, href: "/app/routines" },
    ].map(stat => <Link key={stat.label} href={stat.href} className="rounded-xl border border-border bg-background p-4 transition-colors hover:bg-muted/40"><p className="text-xs text-muted-foreground">{stat.label}</p><p className="mt-2 text-2xl font-semibold tabular-nums">{stat.count}</p></Link>)}</div> : null}
    <section className="surface-card">
      <div className="flex items-center justify-between gap-3"><h2 className="text-lg font-semibold">Your bots</h2>{!compact ? <Link href="/app/bots" className="text-sm underline underline-offset-4">Add a bot</Link> : null}</div>
      {!data ? <p className="mt-4 text-sm text-muted-foreground">Loading your team…</p> : !data.bots.length ? <div className="py-8"><Bot className="h-8 w-8 text-primary" /><h3 className="mt-4 text-base font-medium">Give your first bot a job</h3><p className="mt-2 max-w-lg text-sm leading-6 text-muted-foreground">Connect ChatGPT from the overview, create a computer, and give your bot a name and role. Then delegate a report, a draft, or a task from your project.</p><div className="mt-4 flex gap-4 text-sm"><Link href="/app/computers" className="font-medium underline underline-offset-4">Create a computer</Link><Link href="/app/bots" className="font-medium underline underline-offset-4">Create a bot</Link></div></div> : <ul className="mt-4 grid gap-3 xl:grid-cols-2">{data.bots.map(bot => {
        const attention = ["waiting_approval", "needs_attention", "needs_computer"].includes(bot.presence);
        const working = ["working", "queued", "saving_results"].includes(bot.presence);
        return <li key={bot.id} className="rounded-xl border border-border p-4"><div className="flex flex-wrap items-start justify-between gap-3"><Link href={`/app/bots/${bot.id}`} className="flex min-w-[120px] max-w-full items-center gap-3 font-semibold hover:underline"><span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"><Bot className="size-5" /></span><span className="truncate">{bot.name}</span></Link><span className={`shrink-0 rounded-full px-2 py-1 text-[11px] ${attention ? "bg-amber-50 text-amber-800" : working ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground"}`}>{presenceLabels[bot.presence] ?? "Available"}</span></div><p className="mt-4 flex items-center gap-2 text-xs text-muted-foreground"><Monitor className="size-3.5" />{bot.computerName ?? "Assign a computer in bot settings"}</p>{bot.workId && bot.task ? <Link href={`/app/work/${bot.workId}`} className="mt-3 flex items-start justify-between gap-2 text-sm text-muted-foreground hover:text-foreground"><span className="line-clamp-2">{bot.task}</span><ArrowUpRight className="mt-0.5 size-4 shrink-0" /></Link> : <p className="mt-3 text-sm text-muted-foreground">No assignments yet.</p>}</li>;
      })}</ul>}
    </section>
  </div>;
}
