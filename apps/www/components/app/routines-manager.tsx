"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, ConversationSummary, Routine } from "@/lib/api-types";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { BotSelect } from "@/components/app/bot-select";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { FormDescription, FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { AnimatedTabs, AnimatedTabsTrigger } from "@/components/ui/animated-tabs";
import {
  CalendarClock,
  ChevronRight,
  MoreHorizontal,
  Pause,
  Play,
  Plug,
  Plus,
} from "@/components/icons/lucide";
import {
  formatRoutineNextRun,
  formatRoutineTrigger,
  formatWebhookLastReceived,
} from "@/lib/routine-time";
import { toast } from "sonner";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";

const intervals = [
  { value: "15", label: "Every 15 minutes" },
  { value: "60", label: "Every hour" },
  { value: "1440", label: "Every 24 hours" },
  { value: "10080", label: "Every 7 days" },
];

const scheduleKinds = [
  { value: "interval", label: "Repeating" },
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "cron", label: "Cron" },
];

const weeklyDayOptions = [
  { value: "weekdays", label: "Weekdays" },
  { value: "MON,TUE,WED,THU,FRI,SAT,SUN", label: "Every day" },
  { value: "SAT,SUN", label: "Weekends" },
  { value: "MON,WED,FRI", label: "Mon, Wed, Fri" },
];

const fallbackTimezones = [
  "America/Chicago",
  "America/New_York",
  "America/Los_Angeles",
  "UTC",
];

function localTimezone() {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "America/Chicago";
  } catch {
    return "America/Chicago";
  }
}

function timezoneOptions() {
  const local = localTimezone();
  return local && !fallbackTimezones.includes(local)
    ? [local, ...fallbackTimezones]
    : fallbackTimezones;
}

function localDateTime(date: Date) {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
    .toISOString()
    .slice(0, 16);
}

function labeledItems(options: { value: string; label: string }[]) {
  return Object.fromEntries(options.map((option) => [option.value, option.label])) as Record<
    string,
    string
  >;
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
  timezone: localTimezone(),
  destinationConversationId: "",
  failurePolicy: "pause_after_failure",
  skillId: "",
  pinnedSkillVersion: "",
  nextRunAt: localDateTime(new Date(Date.now() + 3600_000)),
  enabled: true,
  triggerMode: "schedule",
});

type RoutineFormState = ReturnType<typeof emptyForm>;

function absoluteWebhookUrl(url: string) {
  if (url.startsWith("http://") || url.startsWith("https://")) {
    return url;
  }
  const path = url.startsWith("/") ? url : `/${url}`;
  if (typeof window === "undefined") {
    return path;
  }
  return `${window.location.origin}${path}`;
}

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
    skillId: routine.skillId ?? "",
    pinnedSkillVersion:
      routine.pinnedSkillVersion != null ? String(routine.pinnedSkillVersion) : "",
    nextRunAt: localDateTime(new Date(routine.nextRunAt)),
    enabled: routine.enabled,
    triggerMode: routine.triggerMode === "webhook" ? "webhook" : "schedule",
  };
}

async function read<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await cloudHostFetch(path, init);
  const body = await response.json();
  if (!response.ok) throw new Error(body.error ?? "Could not update routine");
  return body as T;
}

function FormSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3">
      <div className="space-y-1">
        <h2 className="text-sm font-medium tracking-tight">{title}</h2>
        {description ? (
          <p className="text-sm leading-relaxed text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <FormFields>{children}</FormFields>
    </section>
  );
}

function routineDetailLine(routine: Routine, botLabel: string) {
  const parts = [formatRoutineTrigger(routine), botLabel];
  if (routine.triggerMode === "webhook") {
    const last = formatWebhookLastReceived(routine.webhook?.lastTriggeredAt);
    parts.push(last ? `Last received ${last}` : "Waiting for an event");
  } else if (routine.enabled) {
    parts.push(
      `Next ${formatRoutineNextRun(routine.nextRunAt, routine.timezone || "UTC")}`,
    );
  }
  return parts.filter(Boolean).join(" · ");
}

export function RoutinesManager() {
  const router = useRouter();
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [bots, setBots] = useState<BotSummary[]>([]);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [skills, setSkills] = useState<
    { id: string; name: string; slug: string; currentVersion: number; status: string }[]
  >([]);
  const [form, setForm] = useState(emptyForm);
  const [editing, setEditing] = useState<string | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [revealedWebhookUrl, setRevealedWebhookUrl] = useState<string | null>(null);
  const [confirmRotateWebhook, setConfirmRotateWebhook] = useState(false);
  const appliedEditRef = useRef(false);

  const zones = useMemo(() => timezoneOptions(), []);
  const activeSkills = useMemo(
    () => skills.filter((skill) => skill.status === "active"),
    [skills],
  );
  const skillItems = useMemo(
    () => ({
      __none__: "No skill",
      ...Object.fromEntries(
        activeSkills.map((skill) => [skill.id, `${skill.name} (v${skill.currentVersion})`]),
      ),
    }),
    [activeSkills],
  );
  const destinationItems = useMemo(() => {
    const items: Record<string, string> = { direct: "Bot direct chat" };
    for (const conversation of conversations) {
      if (conversation.conversationType === "group") {
        items[conversation.id] = conversation.name ?? "Group";
      }
    }
    return items;
  }, [conversations]);
  const weeklyItems = useMemo(() => {
    const options = [...weeklyDayOptions];
    if (form.weeklyDays && !options.some((option) => option.value === form.weeklyDays)) {
      options.push({ value: form.weeklyDays, label: form.weeklyDays });
    }
    return options;
  }, [form.weeklyDays]);
  const editingRoutine = editing
    ? routines.find((routine) => routine.id === editing)
    : undefined;

  const load = useCallback(async () => {
    try {
      const [nextRoutines, nextBots, nextConversations, skillsRes] = await Promise.all([
        read<Routine[]>("/v1/routines"),
        read<BotSummary[]>("/v1/bots"),
        read<ConversationSummary[]>("/v1/conversations"),
        cloudHostFetch("/v1/skills"),
      ]);
      setRoutines(nextRoutines);
      setBots(nextBots);
      setConversations(nextConversations);
      if (skillsRes.ok) {
        setSkills(
          (await skillsRes.json()) as {
            id: string;
            name: string;
            slug: string;
            currentVersion: number;
            status: string;
          }[],
        );
      }
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

  const handleCloseForm = useCallback(() => {
    setFormOpen(false);
    setEditing(null);
    setForm(emptyForm());
    setRevealedWebhookUrl(null);
  }, []);

  const handleNewRoutine = useCallback(() => {
    setEditing(null);
    setForm(emptyForm());
    setRevealedWebhookUrl(null);
    setFormOpen(true);
    setError(null);
  }, []);

  const handleEdit = useCallback((routine: Routine) => {
    setEditing(routine.id);
    setForm(routineToForm(routine));
    setRevealedWebhookUrl(null);
    setFormOpen(true);
    setError(null);
  }, []);

  useEffect(() => {
    if (appliedEditRef.current || routines.length === 0) {
      return;
    }
    const editId = new URLSearchParams(window.location.search).get("edit");
    if (!editId) {
      return;
    }
    const routine = routines.find((row) => row.id === editId);
    if (!routine) {
      return;
    }
    appliedEditRef.current = true;
    handleEdit(routine);
    window.history.replaceState(null, "", "/app/routines");
  }, [handleEdit, routines]);

  useEffect(() => {
    if (formOpen) {
      document.getElementById("routine-name")?.focus();
    }
  }, [formOpen, editing]);

  async function handleSave(event: React.FormEvent) {
    event.preventDefault();
    setBusy("form");
    setError(null);
    try {
      const saved = await read<Routine>(editing ? `/v1/routines/${editing}` : "/v1/routines", {
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
          skillId: form.skillId || null,
          pinnedSkillVersion: form.pinnedSkillVersion.trim()
            ? Number.parseInt(form.pinnedSkillVersion, 10)
            : null,
          nextRunAt: new Date(form.nextRunAt).toISOString(),
          enabled: form.enabled,
          triggerMode: form.triggerMode,
        }),
      });
      if (saved.webhook?.webhookUrl) {
        setRevealedWebhookUrl(absoluteWebhookUrl(saved.webhook.webhookUrl));
        setEditing(saved.id);
        setForm(routineToForm(saved));
        setFormOpen(true);
        toast.success("Webhook URL created. Copy it now — it will not be shown again.");
      } else {
        handleCloseForm();
        toast.success("Routine saved. Its work will appear in Work.");
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save routine");
    } finally {
      setBusy(null);
    }
  }

  const handleToggle = useCallback(
    async (routine: Routine) => {
      setBusy(routine.id);
      setError(null);
      try {
        await read<Routine>(`/v1/routines/${routine.id}/enabled`, {
          method: "POST",
          body: JSON.stringify({ enabled: !routine.enabled }),
        });
        toast.success(
          routine.enabled
            ? routine.triggerMode === "webhook"
              ? "Routine paused. Incoming webhook events will not run."
              : "Routine paused."
            : routine.triggerMode === "webhook"
              ? "Routine resumed. Incoming webhook events will run again."
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

  const handleRunOnce = useCallback(
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

  async function handleCopyWebhookUrl(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      toast.success("Webhook URL copied");
    } catch {
      toast.error("Could not copy webhook URL");
    }
  }

  async function handleRotateWebhookUrl() {
    if (!editing) return;
    setBusy("form");
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/routines/${editing}/webhook/rotate`, {
        method: "POST",
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not rotate webhook URL");
      }
      const url = (body as { webhookUrl?: string }).webhookUrl;
      if (url) {
        setRevealedWebhookUrl(absoluteWebhookUrl(url));
        toast.success("New webhook URL created. The previous URL no longer works.");
      }
      await load();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not rotate webhook URL");
    } finally {
      setBusy(null);
    }
  }

  const botName = useCallback(
    (botId: string) => bots.find((bot) => bot.id === botId)?.name ?? "Bot",
    [bots],
  );

  function renderCreateButton() {
    return (
      <Button
        type="button"
        onClick={handleNewRoutine}
        aria-label="Create a new routine"
      >
        <Plus data-icon="inline-start" aria-hidden />
        New routine
      </Button>
    );
  }

  return (
    <div className="w-full max-w-3xl space-y-8">
      <WorkspacePageHeader
        title="Routines"
        description="Set an assignment once. Your bot runs it on a schedule or a webhook."
        action={renderCreateButton()}
      />

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Could not update routines</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {formOpen ? (
        <section
          id="routine-form"
          className="overflow-hidden rounded-xl border border-border bg-card"
          aria-labelledby="routine-form-title"
        >
          <div className="flex items-start justify-between gap-3 border-b border-border px-5 py-4">
            <div className="min-w-0 space-y-1">
              <h2 id="routine-form-title" className="text-sm font-medium tracking-tight">
                {editing ? "Edit routine" : "New routine"}
              </h2>
              <p className="text-sm leading-relaxed text-muted-foreground">
                {form.triggerMode === "webhook"
                  ? "Runs whenever a service sends a POST to its webhook URL."
                  : "Runs the same assignment at the times you choose."}
              </p>
            </div>
            <Button type="button" variant="ghost" size="sm" onClick={handleCloseForm}>
              Cancel
            </Button>
          </div>

          <form className="space-y-8 px-5 py-5" onSubmit={(event) => void handleSave(event)}>
            <FormSection title="Work">
              <FormItem>
                <Label htmlFor="routine-name">Name</Label>
                <Input
                  id="routine-name"
                  required
                  maxLength={100}
                  value={form.name}
                  onChange={(event) => setForm({ ...form, name: event.target.value })}
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
                  onChange={(event) => setForm({ ...form, instructions: event.target.value })}
                  placeholder="Review project files and prepare a brief with changes, blockers, and next steps."
                />
              </FormItem>
              <FormItem>
                <Label htmlFor="routine-skill">Skill</Label>
                <Select
                  items={skillItems}
                  value={form.skillId || "__none__"}
                  onValueChange={(value) => {
                    if (!value) return;
                    setForm({
                      ...form,
                      skillId: value === "__none__" ? "" : value,
                      pinnedSkillVersion: "",
                    });
                  }}
                >
                  <SelectTrigger id="routine-skill" className="w-full">
                    <SelectValue placeholder="No skill" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">No skill</SelectItem>
                    {activeSkills.map((skill) => (
                      <SelectItem key={skill.id} value={skill.id}>
                        {skill.name} (v{skill.currentVersion})
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </FormItem>
              {form.skillId ? (
                <FormItem>
                  <Label htmlFor="routine-skill-pin">Pin skill version</Label>
                  <Input
                    id="routine-skill-pin"
                    inputMode="numeric"
                    placeholder="Latest"
                    value={form.pinnedSkillVersion}
                    onChange={(event) =>
                      setForm({ ...form, pinnedSkillVersion: event.target.value })
                    }
                  />
                </FormItem>
              ) : null}
            </FormSection>

            <FormSection title="When">
              <FormItem>
                <Label id="routine-trigger-label">Trigger</Label>
                <AnimatedTabs
                  value={form.triggerMode}
                  onValueChange={(value) => setForm({ ...form, triggerMode: value })}
                  variant="pill"
                  selection="radio"
                  aria-label="Routine trigger"
                  className="w-full sm:w-auto"
                >
                  <AnimatedTabsTrigger value="schedule" className="flex-1 px-3 sm:flex-none">
                    Schedule
                  </AnimatedTabsTrigger>
                  <AnimatedTabsTrigger value="webhook" className="flex-1 px-3 sm:flex-none">
                    Webhook
                  </AnimatedTabsTrigger>
                </AnimatedTabs>
              </FormItem>

              {form.triggerMode === "schedule" ? (
                <>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <FormItem>
                      <Label htmlFor="routine-schedule-kind">Repeats</Label>
                      <Select
                        items={labeledItems(scheduleKinds)}
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
                        <Label htmlFor="routine-interval">Every</Label>
                        <Select
                          items={labeledItems(intervals)}
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
                              <SelectItem key={interval.value} value={interval.value}>
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
                          onChange={(event) =>
                            setForm({ ...form, dailyTime: event.target.value })
                          }
                        />
                      </FormItem>
                    ) : null}
                    {form.scheduleKind === "weekly" ? (
                      <FormItem>
                        <Label htmlFor="routine-weekly-days">Days</Label>
                        <Select
                          items={labeledItems(weeklyItems)}
                          value={form.weeklyDays}
                          onValueChange={(value) => {
                            if (value) setForm({ ...form, weeklyDays: value });
                          }}
                        >
                          <SelectTrigger id="routine-weekly-days" className="w-full">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {weeklyItems.map((option) => (
                              <SelectItem key={option.value} value={option.value}>
                                {option.label}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </FormItem>
                    ) : null}
                    {form.scheduleKind === "cron" ? (
                      <FormItem>
                        <Label htmlFor="routine-cron">Expression</Label>
                        <Input
                          id="routine-cron"
                          value={form.cronExpression}
                          onChange={(event) =>
                            setForm({ ...form, cronExpression: event.target.value })
                          }
                          placeholder="0 8 * * 1-5"
                        />
                      </FormItem>
                    ) : null}
                  </div>
                  {form.scheduleKind === "weekly" ? (
                    <FormItem>
                      <Label htmlFor="routine-weekly-time">Time</Label>
                      <Input
                        id="routine-weekly-time"
                        type="time"
                        value={form.weeklyTime}
                        onChange={(event) =>
                          setForm({ ...form, weeklyTime: event.target.value })
                        }
                      />
                    </FormItem>
                  ) : null}
                  <div className="grid gap-3 sm:grid-cols-2">
                    <FormItem>
                      <Label htmlFor="routine-timezone">Timezone</Label>
                      <Select
                        items={Object.fromEntries(zones.map((zone) => [zone, zone])) as Record<
                          string,
                          string
                        >}
                        value={form.timezone}
                        onValueChange={(value) => {
                          if (value) setForm({ ...form, timezone: value });
                        }}
                      >
                        <SelectTrigger id="routine-timezone" className="w-full">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {zones.map((zone) => (
                            <SelectItem key={zone} value={zone}>
                              {zone}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </FormItem>
                    <FormItem>
                      <Label htmlFor="routine-first">Starts</Label>
                      <Input
                        id="routine-first"
                        type="datetime-local"
                        required={form.triggerMode === "schedule"}
                        value={form.nextRunAt}
                        onChange={(event) => setForm({ ...form, nextRunAt: event.target.value })}
                      />
                      <FormDescription>Your local time.</FormDescription>
                    </FormItem>
                  </div>
                </>
              ) : (
                <div className="space-y-3">
                  <p className="text-sm leading-relaxed text-muted-foreground">
                    Send a POST request to run this routine. The full URL is shown only when it is
                    created or rotated.
                  </p>
                  {revealedWebhookUrl ? (
                    <FormItem>
                      <Label htmlFor="routine-webhook-url">Webhook URL</Label>
                      <Input
                        id="routine-webhook-url"
                        readOnly
                        value={revealedWebhookUrl}
                        onFocus={(event) => event.currentTarget.select()}
                      />
                    </FormItem>
                  ) : editing ? (
                    <p className="text-sm leading-relaxed text-muted-foreground">
                      {editingRoutine?.webhook?.tokenHint
                        ? `Current ending: …${editingRoutine.webhook.tokenHint}`
                        : "Save this routine to generate a URL."}
                    </p>
                  ) : (
                    <p className="text-sm leading-relaxed text-muted-foreground">
                      Save this routine to generate a webhook URL.
                    </p>
                  )}
                  {formatWebhookLastReceived(editingRoutine?.webhook?.lastTriggeredAt) ? (
                    <p className="text-sm text-muted-foreground">
                      Last received:{" "}
                      {formatWebhookLastReceived(editingRoutine?.webhook?.lastTriggeredAt)}
                    </p>
                  ) : null}
                  <div className="flex flex-wrap gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={!revealedWebhookUrl}
                      onClick={() => {
                        if (revealedWebhookUrl) void handleCopyWebhookUrl(revealedWebhookUrl);
                      }}
                    >
                      Copy URL
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      disabled={
                        !editing ||
                        busy !== null ||
                        form.triggerMode !== "webhook" ||
                        editingRoutine?.triggerMode !== "webhook"
                      }
                      onClick={() => setConfirmRotateWebhook(true)}
                    >
                      Rotate URL
                    </Button>
                  </div>
                </div>
              )}
            </FormSection>

            <FormSection title="Results">
              <FormItem>
                <Label htmlFor="routine-destination">Post results to</Label>
                <Select
                  items={destinationItems}
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
                      .filter((conversation) => conversation.conversationType === "group")
                      .map((conversation) => (
                        <SelectItem key={conversation.id} value={conversation.id}>
                          {conversation.name ?? "Group"}
                        </SelectItem>
                      ))}
                  </SelectContent>
                </Select>
              </FormItem>
              <div className="flex items-start gap-2">
                <Checkbox
                  id="routine-enabled"
                  checked={form.enabled}
                  onCheckedChange={(checked) =>
                    setForm({ ...form, enabled: checked === true })
                  }
                />
                <div className="space-y-1">
                  <Label htmlFor="routine-enabled" className="cursor-pointer font-normal">
                    {form.triggerMode === "webhook"
                      ? "Accept incoming webhook events"
                      : "Run on schedule"}
                  </Label>
                  <FormDescription>
                    {form.triggerMode === "webhook"
                      ? "Events become normal routine work with the same approvals and history."
                      : "Missed times combine into one assignment. Computer changes still need approval."}
                  </FormDescription>
                </div>
              </div>
            </FormSection>

            <div className="flex flex-wrap items-center gap-3 border-t border-border pt-5">
              <Button type="submit" disabled={busy !== null}>
                {busy === "form" ? "Saving…" : editing ? "Save changes" : "Create routine"}
              </Button>
              <Button type="button" variant="ghost" onClick={handleCloseForm}>
                Cancel
              </Button>
            </div>
          </form>
        </section>
      ) : null}

      {loading ? (
        <ul className="divide-y divide-border overflow-hidden rounded-xl border border-border bg-card">
          {Array.from({ length: 3 }).map((_, index) => (
            <li key={index} className="flex items-center gap-4 px-4 py-4">
              <Skeleton className="size-9 rounded-lg" />
              <div className="min-w-0 flex-1 space-y-2">
                <Skeleton className="h-4 w-40" />
                <Skeleton className="h-3 w-3/4" />
              </div>
            </li>
          ))}
        </ul>
      ) : routines.length === 0 && !formOpen ? (
        <WorkspaceEmptyState
          title="No routines yet"
          description="Have a bot review files, send a daily brief, or watch a project. Each run has its own progress and result."
          icon={<CalendarClock aria-hidden />}
        >
          {renderCreateButton()}
        </WorkspaceEmptyState>
      ) : routines.length > 0 ? (
        <ul className="overflow-hidden rounded-xl border border-border bg-card">
          {routines.map((routine) => {
            const isBusy = busy !== null;
            const Icon = routine.triggerMode === "webhook" ? Plug : CalendarClock;
            return (
              <li
                key={routine.id}
                className="group/routine border-b border-border last:border-b-0 hover:bg-muted/40"
              >
                <div className="flex items-stretch">
                  <Link
                    href={`/app/routines/${routine.id}`}
                    className="group flex min-w-0 flex-1 items-start gap-3 px-4 py-3.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
                  >
                    <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg bg-surface-active text-foreground">
                      <Icon className="size-4" aria-hidden />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                        <h2 className="truncate text-sm font-medium tracking-tight">
                          {routine.name}
                        </h2>
                        <Badge
                          variant={routine.enabled ? "success-light" : "warning-light"}
                          size="sm"
                        >
                          {routine.enabled ? "Active" : "Paused"}
                        </Badge>
                      </div>
                      <p className="mt-1 text-sm leading-6 text-muted-foreground">
                        {routineDetailLine(routine, botName(routine.botId))}
                      </p>
                      {routine.lastError ? (
                        <p className="mt-1 text-xs text-warning-foreground">{routine.lastError}</p>
                      ) : null}
                    </div>
                    <ChevronRight
                      className="mt-2 size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover:text-foreground"
                      aria-hidden
                    />
                  </Link>
                  <div className="flex shrink-0 items-start py-3 pr-2">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="text-muted-foreground opacity-70 transition-opacity group-hover/routine:opacity-100 hover:opacity-100"
                      disabled={isBusy}
                      aria-label={routine.enabled ? `Pause ${routine.name}` : `Resume ${routine.name}`}
                      onClick={() => void handleToggle(routine)}
                    >
                      {routine.enabled ? <Pause aria-hidden /> : <Play aria-hidden />}
                    </Button>
                    <DropdownMenu>
                      <DropdownMenuTrigger
                        render={
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon-sm"
                            className="text-muted-foreground opacity-70 transition-opacity group-hover/routine:opacity-100 hover:opacity-100"
                            disabled={isBusy}
                            aria-label={`More actions for ${routine.name}`}
                          />
                        }
                      >
                        <MoreHorizontal aria-hidden />
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="min-w-40">
                        <DropdownMenuItem onClick={() => handleEdit(routine)}>
                          Edit
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => void handleRunOnce(routine)}>
                          Test run
                        </DropdownMenuItem>
                        {routine.lastRunId ? (
                          <DropdownMenuItem
                            render={<Link href={`/app/work/${routine.lastRunId}`} />}
                          >
                            Latest work
                          </DropdownMenuItem>
                        ) : null}
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      ) : null}

      <ConfirmAlertDialog
        open={confirmRotateWebhook}
        onOpenChange={(open) => {
          if (!open && busy === null) {
            setConfirmRotateWebhook(false);
          }
        }}
        title="Rotate webhook URL?"
        description="The previous URL stops working immediately. Any integrations still using it will fail until you update them with the new URL."
        confirmLabel="Rotate URL"
        pendingLabel="Rotating…"
        destructive
        pending={busy === "form"}
        onConfirm={() => {
          void (async () => {
            await handleRotateWebhookUrl();
            setConfirmRotateWebhook(false);
          })();
        }}
      />
    </div>
  );
}
