"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, ConversationSummary, Routine } from "@/lib/api-types";
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
import { formatRoutineNextRun } from "@/lib/routine-time";

const intervals = [
  { value: 15, label: "Every 15 minutes" },
  { value: 60, label: "Every hour" },
  { value: 1440, label: "Every 24 hours" },
  { value: 10080, label: "Every 7 days" },
];

const scheduleKinds = [
  { value: "interval", label: "Every N minutes/hours" },
  { value: "daily", label: "Daily at time" },
  { value: "weekly", label: "Weekdays or weekly" },
  { value: "cron", label: "Advanced cron" },
];

const timezones = [
  "America/Chicago",
  "America/New_York",
  "America/Los_Angeles",
  "UTC",
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
  scheduleKind: "interval",
  scheduleExpression: "1440",
  dailyTime: "08:00",
  weeklyTime: "08:00",
  weeklyDays: "weekdays",
  cronExpression: "0 8 * * 1-5",
  timezone: "America/Chicago",
  destinationConversationId: "",
  failurePolicy: "pause_after_failure",
  nextRunAt: localDateTime(new Date(Date.now() + 3600_000)),
  enabled: true,
});

type RoutineFormState = ReturnType<typeof emptyForm>;

function buildScheduleExpression(form: RoutineFormState) {
  if (form.scheduleKind === "interval") return String(form.intervalMinutes);
  if (form.scheduleKind === "daily") return form.dailyTime;
  if (form.scheduleKind === "weekly") {
    return `${form.weeklyDays}|${form.weeklyTime}`;
  }
  return form.cronExpression;
}

function routineToForm(routine: Routine): RoutineFormState {
  const base = emptyForm();
  const scheduleKind = routine.scheduleKind || "interval";
  let dailyTime = base.dailyTime;
  let weeklyTime = base.weeklyTime;
  let weeklyDays = base.weeklyDays;
  let cronExpression = base.cronExpression;
  let intervalMinutes = routine.intervalMinutes;

  if (scheduleKind === "daily") {
    dailyTime = routine.scheduleExpression || dailyTime;
  } else if (scheduleKind === "weekly") {
    const [days, time] = (routine.scheduleExpression || "").split("|");
    weeklyDays = days || weeklyDays;
    weeklyTime = time || weeklyTime;
  } else if (scheduleKind === "cron") {
    cronExpression = routine.scheduleExpression || cronExpression;
  } else {
    const parsed = Number.parseInt(routine.scheduleExpression, 10);
    if (!Number.isNaN(parsed)) intervalMinutes = parsed;
  }

  return {
    ...base,
    name: routine.name,
    botId: routine.botId,
    instructions: routine.instructions,
    intervalMinutes,
    scheduleKind,
    scheduleExpression: routine.scheduleExpression || String(intervalMinutes),
    dailyTime,
    weeklyTime,
    weeklyDays,
    cronExpression,
    timezone: routine.timezone || base.timezone,
    destinationConversationId: routine.destinationConversationId ?? "",
    failurePolicy: routine.failurePolicy || base.failurePolicy,
    nextRunAt: localDateTime(new Date(routine.nextRunAt)),
    enabled: routine.enabled,
  };
}

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
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
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
      const [nextRoutines, nextBots, nextConversations] = await Promise.all([
        read<Routine[]>("/v1/routines"),
        read<BotSummary[]>("/v1/bots"),
        read<ConversationSummary[]>("/v1/conversations"),
      ]);
      setRoutines(nextRoutines);
      setBots(nextBots);
      setConversations(nextConversations);
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
          botId: form.botId,
          name: form.name,
          instructions: form.instructions,
          intervalMinutes: form.intervalMinutes,
          scheduleKind: form.scheduleKind,
          scheduleExpression: buildScheduleExpression(form),
          timezone: form.timezone,
          destinationConversationId: form.destinationConversationId || null,
          failurePolicy: form.failurePolicy,
          nextRunAt: new Date(form.nextRunAt).toISOString(),
          enabled: form.enabled,
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
        const result = await read<{ runId: string }>(`/v1/routines/${routine.id}/test`, {
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
    setForm(routineToForm(routine));
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
            <Link
              href={`/app/routines/${row.original.id}`}
              className="font-medium hover:underline underline-offset-2"
            >
              {row.original.name}
            </Link>
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
            <p>
              {row.original.scheduleLabel ?? intervalLabel(row.original.intervalMinutes)} ·{" "}
              {row.original.timezone}
            </p>
            {row.original.enabled ? (
              <p className="mt-1 text-xs text-muted-foreground">
                Next:{" "}
                {formatRoutineNextRun(
                  row.original.nextRunAt,
                  row.original.timezone || "UTC",
                )}
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
                Test run
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
            Server-side schedules run without your browser. Missed times combine into one run.
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
              <Label htmlFor="routine-schedule-kind">Schedule</Label>
              <Select
                value={form.scheduleKind}
                onValueChange={(value) => {
                  if (value) setForm({ ...form, scheduleKind: value });
                }}
              >
                <SelectTrigger id="routine-schedule-kind" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {scheduleKinds.map((kind) => (
                    <SelectItem key={kind.value} value={kind.value}>
                      {kind.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormItem>
            {form.scheduleKind === "interval" ? (
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
            ) : null}
            {form.scheduleKind === "daily" ? (
              <FormItem>
                <Label htmlFor="routine-daily-time">Time</Label>
                <Input
                  id="routine-daily-time"
                  type="time"
                  value={form.dailyTime}
                  onChange={(e) => setForm({ ...form, dailyTime: e.target.value })}
                />
              </FormItem>
            ) : null}
            {form.scheduleKind === "weekly" ? (
              <>
                <FormItem>
                  <Label htmlFor="routine-weekly-days">Days</Label>
                  <Input
                    id="routine-weekly-days"
                    value={form.weeklyDays}
                    onChange={(e) => setForm({ ...form, weeklyDays: e.target.value })}
                    placeholder="weekdays or MON,WED,FRI"
                  />
                </FormItem>
                <FormItem>
                  <Label htmlFor="routine-weekly-time">Time</Label>
                  <Input
                    id="routine-weekly-time"
                    type="time"
                    value={form.weeklyTime}
                    onChange={(e) => setForm({ ...form, weeklyTime: e.target.value })}
                  />
                </FormItem>
              </>
            ) : null}
            {form.scheduleKind === "cron" ? (
              <FormItem>
                <Label htmlFor="routine-cron">Cron</Label>
                <Input
                  id="routine-cron"
                  value={form.cronExpression}
                  onChange={(e) => setForm({ ...form, cronExpression: e.target.value })}
                  placeholder="0 8 * * 1-5"
                />
              </FormItem>
            ) : null}
            <FormItem>
              <Label htmlFor="routine-timezone">Timezone</Label>
              <Select
                value={form.timezone}
                onValueChange={(value) => {
                  if (value) setForm({ ...form, timezone: value });
                }}
              >
                <SelectTrigger id="routine-timezone" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {timezones.map((zone) => (
                    <SelectItem key={zone} value={zone}>
                      {zone}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FormItem>
            <FormItem>
              <Label htmlFor="routine-destination">Post results to</Label>
              <Select
                value={form.destinationConversationId || "direct"}
                onValueChange={(value) => {
                  setForm({
                    ...form,
                    destinationConversationId:
                      value === "direct" || value == null ? "" : value,
                  });
                }}
              >
                <SelectTrigger id="routine-destination" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="direct">Bot direct chat</SelectItem>
                  {conversations
                    .filter((c) => c.conversationType === "group")
                    .map((conversation) => (
                      <SelectItem key={conversation.id} value={conversation.id}>
                        {conversation.name ?? "Group"}
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
