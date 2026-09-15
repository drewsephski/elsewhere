"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { Routine } from "@/lib/api-types";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/reui/badge";

export default function RoutineDetailPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const [routine, setRoutine] = useState<Routine | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/routines/${params.id}`);
    const body = await response.json();
    if (!response.ok) {
      setError(body.error ?? "Routine not found");
      return;
    }
    setRoutine(body as Routine);
    setError(null);
  }, [params.id]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleTestRun() {
    if (!routine) return;
    setBusy(true);
    try {
      const response = await cloudHostFetch(`/v1/routines/${routine.id}/test`, {
        method: "POST",
        headers: { "Idempotency-Key": crypto.randomUUID() },
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Test run failed");
      router.push(`/app/work/${body.runId}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Test run failed");
    } finally {
      setBusy(false);
    }
  }

  if (error && !routine) {
    return (
      <div className="space-y-4">
        <WorkspacePageHeader title="Routine" description={error} />
        <Link href="/app/routines" className="text-sm text-primary underline-offset-2 hover:underline">
          Back to routines
        </Link>
      </div>
    );
  }

  if (!routine) {
    return <p className="text-sm text-muted-foreground">Loading routine…</p>;
  }

  return (
    <div className="mx-auto max-w-3xl space-y-8">
      <WorkspacePageHeader
        title={routine.name}
        description={`${routine.scheduleLabel} · ${routine.timezone}`}
      />
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={routine.enabled ? "success-light" : "warning-light"}>
          {routine.enabled ? "Active" : "Paused"}
        </Badge>
        <span className="text-sm text-muted-foreground">
          Next: {new Date(routine.nextRunAt).toLocaleString()}
        </span>
      </div>
      <p className="text-sm leading-6 text-muted-foreground whitespace-pre-wrap">
        {routine.instructions}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button type="button" disabled={busy} onClick={() => void handleTestRun()}>
          Test run
        </Button>
        <Link
          href="/app/routines"
          className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-background px-4 text-sm font-medium hover:bg-accent"
        >
          Edit in list
        </Link>
      </div>
      <section className="space-y-3">
        <h2 className="text-sm font-medium">Recent runs</h2>
        <ul className="divide-y rounded-lg border">
          {routine.recentRuns.length === 0 ? (
            <li className="p-4 text-sm text-muted-foreground">No runs yet.</li>
          ) : (
            routine.recentRuns.map((run) => (
              <li key={run.id} className="flex items-center justify-between gap-3 p-4 text-sm">
                <div>
                  <p className="font-medium capitalize">{run.status}</p>
                  <p className="text-xs text-muted-foreground">
                    {new Date(run.scheduledFor).toLocaleString()} · {run.triggerKind}
                  </p>
                </div>
                {run.runId ? (
                  <Link
                    href={`/app/work/${run.runId}`}
                    className="text-primary underline-offset-2 hover:underline"
                  >
                    View
                  </Link>
                ) : null}
              </li>
            ))
          )}
        </ul>
      </section>
    </div>
  );
}
