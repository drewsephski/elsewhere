"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { RunSummary } from "@/lib/api-types";
import { workStatus } from "@/lib/work-events";
import Link from "next/link";
import { useEffect, useState } from "react";

export function RecentRunsPanel({ botId, limit = 10 }: { botId?: string; limit?: number }) {
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function load() {
      try {
        const response = await cloudHostFetch(`/v1/runs?limit=${limit}${botId ? `&bot_id=${encodeURIComponent(botId)}` : ""}`);
        if (!response.ok) throw new Error("Could not load your work");
        const rows: RunSummary[] = await response.json();
        if (!stopped) { setRuns(rows); setError(null); }
      } catch (err) { if (!stopped) setError(err instanceof Error ? err.message : "Work unavailable"); }
      finally { if (!stopped) { setLoading(false); timer = setTimeout(() => void load(), 5000); } }
    }
    void load();
    return () => { stopped = true; clearTimeout(timer); };
  }, [botId, limit]);
  return (
    <section className="surface-card">
      <h2 className="text-base font-semibold">Recent work</h2>
      {error ? <p className="mt-3 text-sm text-red-700" role="alert">{error}</p> : null}
      <ul className="mt-4 divide-y divide-border">
        {runs.map(run => (
          <li key={run.runId}>
            <Link href={`/app/work/${run.runId}`} className="group flex items-start justify-between gap-4 rounded-lg py-4 transition-colors hover:bg-muted/50">
              <div className="min-w-0">
                <p className="line-clamp-2 text-sm font-medium group-hover:underline">{run.task}</p>
                <p className="mt-1 text-xs text-muted-foreground">{run.botName} · {new Date(run.createdAt).toLocaleString()}</p>
              </div>
              <span className="shrink-0 rounded-full bg-muted px-2.5 py-1 text-xs">{workStatus(run.status)}</span>
            </Link>
          </li>
        ))}
      </ul>
      {loading ? <p className="mt-4 text-sm text-muted-foreground">Loading your work…</p> : null}
      {!loading && !runs.length && !error ? <p className="mt-4 text-sm text-muted-foreground">Delegate a task to a bot. Its progress and finished work will appear here.</p> : null}
    </section>
  );
}
