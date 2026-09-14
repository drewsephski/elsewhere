"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { Routine } from "@/lib/api-types";
import { cn } from "cn";
import { CalendarClock, Pause, Play } from "lucide-react";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

const intervalLabels: Record<number, string> = {
  15: "Every 15 minutes",
  60: "Every hour",
  1440: "Every day",
  10080: "Every 7 days",
};

function scheduleLabel(routine: Routine): string {
  return (
    intervalLabels[routine.intervalMinutes] ??
    `Every ${routine.intervalMinutes} minutes`
  );
}

interface BotRoutinesSidebarProps {
  botId: string;
  className?: string;
}

export function BotRoutinesSidebar({ botId, className }: BotRoutinesSidebarProps) {
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const response = await cloudHostFetch("/v1/routines");
      if (!response.ok) {
        throw new Error("Could not load routines");
      }
      const all: Routine[] = await response.json();
      setRoutines(all.filter((routine) => routine.botId === botId));
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Routines unavailable");
    }
  }, [botId]);

  useEffect(() => {
    void load();
    const timer = setInterval(() => void load(), 15000);
    return () => clearInterval(timer);
  }, [load]);

  async function toggle(routine: Routine) {
    setBusy(routine.id);
    try {
      const response = await cloudHostFetch(`/v1/routines/${routine.id}/enabled`, {
        method: "POST",
        body: JSON.stringify({ enabled: !routine.enabled }),
      });
      if (!response.ok) {
        throw new Error("Could not update routine");
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update routine");
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className={cn("workspace-rail-card", className)} aria-labelledby="routines-panel-title">
      <div className="flex items-center justify-between gap-2">
        <h2 id="routines-panel-title" className="text-sm font-semibold">Routines</h2>
        <Link
          href="/app/routines"
          className="text-xs text-muted-foreground underline-offset-2 hover:underline"
        >
          All routines
        </Link>
      </div>
      {error ? (
        <p className="mt-2 text-xs text-red-600" role="alert">{error}</p>
      ) : null}
      <ul className="mt-3 space-y-2">
        {routines.map((routine) => (
          <li
            key={routine.id}
            className="flex items-start gap-2 rounded-xl border border-border/70 bg-white/60 px-3 py-2.5"
          >
            <span className="mt-0.5 text-muted-foreground" aria-hidden>
              {routine.enabled ? (
                <CalendarClock className="size-4 text-primary" />
              ) : (
                <Pause className="size-4" />
              )}
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">{routine.name}</p>
              <p className="text-xs text-muted-foreground">{scheduleLabel(routine)}</p>
              {routine.enabled ? (
                <p className="mt-1 text-[11px] text-muted-foreground">
                  Next {new Date(routine.nextRunAt).toLocaleString()}
                </p>
              ) : null}
            </div>
            <button
              type="button"
              disabled={busy !== null}
              onClick={() => void toggle(routine)}
              className="shrink-0 rounded-lg p-1.5 text-muted-foreground hover:bg-muted disabled:opacity-50"
              aria-label={routine.enabled ? "Pause routine" : "Resume routine"}
            >
              {routine.enabled ? (
                <Pause className="size-4" />
              ) : (
                <Play className="size-4" />
              )}
            </button>
          </li>
        ))}
      </ul>
      {!routines.length ? (
        <p className="mt-3 text-sm text-muted-foreground">
          No routines for this bot yet.{" "}
          <Link href="/app/routines" className="underline underline-offset-2">
            Create one
          </Link>
        </p>
      ) : null}
    </section>
  );
}
