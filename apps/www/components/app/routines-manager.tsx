"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, Routine } from "@/lib/api-types";
import { WorkspaceDataGrid } from "@/components/app/workspace-data-grid";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import {
  dataGridFeatures,
  type DataGridFeatures,
} from "@/components/reui/data-grid/data-grid";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { BotSelect } from "@/components/app/bot-select";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  ColumnDef,
  PaginationState,
  useTable,
} from "@tanstack/react-table";
import { CalendarClock } from "@/components/icons/lucide";

const intervals = [
  { value: 15, label: "Every 15 minutes" },
  { value: 60, label: "Every hour" },
  { value: 1440, label: "Every 24 hours" },
  { value: 10080, label: "Every 7 days" },
];

function localDateTime(date: Date) {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
    .toISOString()
    .slice(0, 16);
}

const emptyForm = () => ({
  name: "",
  botId: "",
  instructions: "",
  intervalMinutes: 1440,
  nextRunAt: localDateTime(new Date(Date.now() + 3600_000)),
  enabled: true,
});

async function read<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await cloudHostFetch(path, init);
  const body = await response.json();
  if (!response.ok) throw new Error(body.error ?? "Could not update routine");
  return body as T;
}

export function RoutinesManager() {
  const router = useRouter();
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [bots, setBots] = useState<BotSummary[]>([]);
  const [form, setForm] = useState(emptyForm);
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });

  const load = useCallback(async () => {
    try {
      const [nextRoutines, nextBots] = await Promise.all([
        read<Routine[]>("/v1/routines"),
        read<BotSummary[]>("/v1/bots"),
      ]);
      setRoutines(nextRoutines);
      setBots(nextBots);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load routines");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function save(event: React.FormEvent) {
    event.preventDefault();
    setBusy("form");
    setError(null);
    try {
      await read<Routine>(editing ? `/v1/routines/${editing}` : "/v1/routines", {
        method: editing ? "PUT" : "POST",
        body: JSON.stringify({
          ...form,
          nextRunAt: new Date(form.nextRunAt).toISOString(),
        }),
      });
      setForm(emptyForm());
      setEditing(null);
      setNotice("Routine saved. Its work and results will appear in Work.");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save routine");
    } finally {
      setBusy(null);
    }
  }

  const toggle = useCallback(
    async (routine: Routine) => {
      setBusy(routine.id);
      setError(null);
      try {
        await read<Routine>(`/v1/routines/${routine.id}/enabled`, {
          method: "POST",
          body: JSON.stringify({ enabled: !routine.enabled }),
        });
        setNotice(
          routine.enabled
            ? "Routine paused. Assignments already started keep their own status."
            : "Routine resumed. It will run at its next scheduled time.",
        );
        await load();
      } catch (err) {
        setError(err instanceof Error ? err.message : "Could not update routine");
      } finally {
        setBusy(null);
      }
    },
    [load],
  );

  const runOnce = useCallback(
    async (routine: Routine) => {
      setBusy(routine.id);
      setError(null);
      try {
        const result = await read<{ runId: string }>(`/v1/routines/${routine.id}/run`, {
          method: "POST",
          headers: { "Idempotency-Key": crypto.randomUUID() },
        });
        router.push(`/app/work/${result.runId}`);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Could not start routine");
      } finally {
        setBusy(null);
      }
    },
    [router],
  );

  const edit = useCallback((routine: Routine) => {
    setEditing(routine.id);
    setForm({
      name: routine.name,
      botId: routine.botId,
      instructions: routine.instructions,
      intervalMinutes: routine.intervalMinutes,
      nextRunAt: localDateTime(new Date(routine.nextRunAt)),
      enabled: routine.enabled,
    });
    document.getElementById("routine-name")?.focus();
  }, []);

  const botName = useCallback(
    (botId: string) => bots.find((bot) => bot.id === botId)?.name ?? "Bot",
    [bots],
  );

  const intervalLabel = useCallback((minutes: number) => {
    return intervals.find((interval) => interval.value === minutes)?.label ??
      `Every ${minutes} minutes`;
  }, []);

  const columns = useMemo<ColumnDef<DataGridFeatures, Routine>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Routine",
        cell: ({ row }) => (
          <div className="min-w-0 py-0.5">
            <p className="font-medium">{row.original.name}</p>
            <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
              {row.original.instructions}
            </p>
            {row.original.lastError ? (
              <p className="mt-2 text-xs text-warning-foreground">{row.original.lastError}</p>
            ) : null}
          </div>
        ),
      },
      {
        id: "bot",
        header: "Bot",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">{botName(row.original.botId)}</span>
        ),
      },
      {
        id: "schedule",
        header: "Schedule",
        cell: ({ row }) => (
          <div className="text-sm">
            <p>{intervalLabel(row.original.intervalMinutes)}</p>
            {row.original.enabled ? (
              <p className="mt-1 text-xs text-muted-foreground">
                Next: {new Date(row.original.nextRunAt).toLocaleString()}
              </p>
            ) : null}
          </div>
        ),
      },
      {
        id: "status",
        header: "Status",
        cell: ({ row }) => (
          <Badge variant={row.original.enabled ? "success-light" : "warning-light"}>
            {row.original.enabled ? "Scheduled" : "Paused"}
          </Badge>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => {
          const routine = row.original;
          const isBusy = busy !== null;
          return (
            <div className="flex flex-wrap items-center justify-start gap-1.5 sm:justify-end sm:gap-2">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={isBusy}
                onClick={() => void toggle(routine)}
              >
                {routine.enabled ? "Pause" : "Resume"}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={isBusy}
                onClick={() => void runOnce(routine)}
              >
                Run once
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={isBusy}
                onClick={() => edit(routine)}
              >
                Edit
              </Button>
              {routine.lastRunId ? (
                <Link
                  href={`/app/work/${routine.lastRunId}`}
                  className="text-sm text-primary underline-offset-2 hover:underline"
                >
                  Latest work
                </Link>
              ) : null}
            </div>
          );
        },
        enableSorting: false,
      },
    ],
    [botName, busy, edit, intervalLabel, runOnce, toggle],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: routines,
    pageCount: Math.ceil(routines.length / pagination.pageSize) || 1,
    getRowId: (row) => row.id,
    state: { pagination },
    onPaginationChange: setPagination,
  });

  return (
    <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="space-y-4">
        {error ? (
          <Alert variant="destructive">
            <AlertTitle>Could not update routines</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}
        {notice ? (
          <Alert variant="success">
            <AlertTitle>Saved</AlertTitle>
            <AlertDescription>{notice}</AlertDescription>
          </Alert>
        ) : null}
        <WorkspaceDataGrid
          table={table}
          recordCount={routines.length}
          loading={loading && routines.length === 0}
          toolbar={
            <div className="flex items-center justify-between gap-3">
              <p className="text-sm text-muted-foreground">Scheduled assignments for your bots.</p>
              <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
                Refresh
              </Button>
            </div>
          }
          emptyMessage={
            <WorkspaceEmptyState
              title="Delegate once. Make it a habit."
              description="Have a bot review new files, prepare a daily brief, or check a project regularly. Every occurrence gets its own progress, approvals, and result."
              icon={<CalendarClock aria-hidden />}
            />
          }
        />
      </div>

      <Frame className="w-full xl:sticky xl:top-6" spacing="sm">
        <FrameHeader>
          <FrameTitle>{editing ? "Edit routine" : "Create a routine"}</FrameTitle>
          <FrameDescription>
            Repeats at a fixed interval. Your Elsewhere host must be running.
          </FrameDescription>
        </FrameHeader>
        <FramePanel>
          <form onSubmit={(event) => void save(event)}>
            <FormFields>
            <FormItem>
              <Label htmlFor="routine-name">Name</Label>
              <Input
                id="routine-name"
                required
                maxLength={100}
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder="Morning project brief"
              />
            </FormItem>
            <BotSelect
              id="routine-bot"
              value={form.botId}
              onValueChange={(botId) => setForm({ ...form, botId })}
              bots={bots}
              requireComputer
              required
            />
            <FormItem>
              <Label htmlFor="routine-task">Assignment</Label>
              <Textarea
                id="routine-task"
                className="min-h-28"
                required
                maxLength={100_000}
                value={form.instructions}
                onChange={(e) => setForm({ ...form, instructions: e.target.value })}
                placeholder="Review project files and prepare a brief with changes, blockers, and next steps."
              />
            </FormItem>
            <FormItem>
              <Label htmlFor="routine-interval">Repeat</Label>
              <Select
                value={String(form.intervalMinutes)}
                onValueChange={(value) => {
                  if (value) setForm({ ...form, intervalMinutes: Number(value) });
                }}
              >
                <SelectTrigger id="routine-interval" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {intervals.map((interval) => (
                    <SelectItem key={interval.value} value={String(interval.value)}>
                      {interval.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormItem>
            <FormItem>
              <Label htmlFor="routine-first">First run · your local time</Label>
              <Input
                id="routine-first"
                type="datetime-local"
                required
                value={form.nextRunAt}
                onChange={(e) => setForm({ ...form, nextRunAt: e.target.value })}
              />
            </FormItem>
            <div className="flex items-center gap-2">
              <Checkbox
                id="routine-enabled"
                checked={form.enabled}
                onCheckedChange={(checked) => setForm({ ...form, enabled: checked === true })}
              />
              <Label htmlFor="routine-enabled" className="cursor-pointer font-normal">
                Enable scheduled work
              </Label>
            </div>
            <p className="text-xs leading-5 text-muted-foreground">
              Missed times are combined into one assignment. Computer changes still need approval.
            </p>
            <div className="flex flex-wrap gap-3">
              <Button type="submit" disabled={busy !== null}>
                {busy === "form" ? "Saving…" : "Save routine"}
              </Button>
              {editing ? (
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => {
                    setEditing(null);
                    setForm(emptyForm());
                  }}
                >
                  Cancel edit
                </Button>
              ) : null}
            </div>
            </FormFields>
          </form>
        </FramePanel>
      </Frame>
    </div>
  );
}
