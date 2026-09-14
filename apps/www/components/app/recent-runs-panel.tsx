"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { RunSummary } from "@/lib/api-types";
import Link from "next/link";
import { useEffect, useState } from "react";

export function RecentRunsPanel() {
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void cloudHostFetch("/v1/runs?limit=10")
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`Runs list failed (${response.status})`);
        }
        setRuns((await response.json()) as RunSummary[]);
      })
      .catch((err: Error) => setError(err.message));
  }, []);

  return (
    <section className="border border-brand-dark/15 bg-white p-5">
      <h2 className="text-sm font-medium uppercase tracking-[0.15em]">Recent runs</h2>
      {error ? (
        <p className="mt-3 text-sm text-red-700" role="alert">
          {error}
        </p>
      ) : null}
      <ul className="mt-4 space-y-2">
        {runs.map((run) => (
          <li key={run.runId} className="flex flex-wrap items-baseline justify-between gap-2 text-sm">
            <Link href={`/app/bots/${run.botId}`} className="underline-offset-4 hover:underline">
              {run.status} · {run.model}
            </Link>
            <span className="text-xs text-brand-dark/50">{run.runId.slice(0, 8)}…</span>
          </li>
        ))}
        {runs.length === 0 && !error ? (
          <li className="text-sm text-brand-dark/50">No runs yet.</li>
        ) : null}
      </ul>
    </section>
  );
}
