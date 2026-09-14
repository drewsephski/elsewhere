"use client";
import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, Routine } from "@/lib/api-types";
import { Button } from "@/components/ui/button";

const intervals = [{ value: 15, label: "Every 15 minutes" }, { value: 60, label: "Every hour" }, { value: 1440, label: "Every 24 hours" }, { value: 10080, label: "Every 7 days" }];
function localDateTime(date: Date) { return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16); }
const emptyForm = () => ({ name: "", botId: "", instructions: "", intervalMinutes: 1440, nextRunAt: localDateTime(new Date(Date.now() + 3600_000)), enabled: true });
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
  const load = useCallback(async () => {
    const [routines, bots] = await Promise.all([read<Routine[]>("/v1/routines"), read<BotSummary[]>("/v1/bots")]);
    setRoutines(routines); setBots(bots);
  }, []);
  useEffect(() => { void load().catch(err => setError(err.message)); }, [load]);

  async function save(event: React.FormEvent) {
    event.preventDefault(); setBusy("form"); setError(null);
    try {
      await read<Routine>(editing ? `/v1/routines/${editing}` : "/v1/routines", {
        method: editing ? "PUT" : "POST", body: JSON.stringify({ ...form, nextRunAt: new Date(form.nextRunAt).toISOString() }),
      });
      setForm(emptyForm()); setEditing(null); setNotice("Routine saved. Its work and results will appear in Work."); await load();
    } catch (err) { setError(err instanceof Error ? err.message : "Could not save routine"); }
    finally { setBusy(null); }
  }
  async function toggle(routine: Routine) {
    setBusy(routine.id); setError(null);
    try {
      await read<Routine>(`/v1/routines/${routine.id}/enabled`, { method: "POST", body: JSON.stringify({ enabled: !routine.enabled }) });
      setNotice(routine.enabled ? "Routine paused. Assignments already started keep their own status." : "Routine resumed. It will run at its next scheduled time."); await load();
    } catch (err) { setError(err instanceof Error ? err.message : "Could not update routine"); }
    finally { setBusy(null); }
  }
  async function runOnce(routine: Routine) {
    setBusy(routine.id); setError(null);
    try {
      const result = await read<{ runId: string }>(`/v1/routines/${routine.id}/run`, { method: "POST", headers: { "Idempotency-Key": crypto.randomUUID() } });
      router.push(`/app/work/${result.runId}`);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not start routine"); }
    finally { setBusy(null); }
  }
  function edit(routine: Routine) {
    setEditing(routine.id); setForm({ name: routine.name, botId: routine.botId, instructions: routine.instructions, intervalMinutes: routine.intervalMinutes, nextRunAt: localDateTime(new Date(routine.nextRunAt)), enabled: routine.enabled });
    document.getElementById("routine-name")?.focus();
  }
  const inputClass = "mt-1 w-full rounded-lg border border-border bg-background px-3 py-2 text-sm";
  return (
    <div className="space-y-5">
      {error ? <p role="alert" className="text-sm text-red-700">{error}</p> : null}
      <p role="status" className="text-sm text-muted-foreground">{notice}</p>
      <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
        <section className="space-y-4">
          <div className="flex items-center justify-between"><h2 className="font-semibold">Your routines</h2><button type="button" onClick={() => void load().catch(err => setError(err.message))} className="text-sm underline">Refresh</button></div>
          {routines.map(routine => <article key={routine.id} className="surface-card">
            <div className="flex items-center justify-between gap-3"><h3 className="font-semibold">{routine.name}</h3><span className="rounded-full bg-muted px-2.5 py-1 text-xs">{routine.enabled ? "Scheduled" : "Paused"}</span></div>
            <p className="mt-1 text-xs text-muted-foreground">{bots.find(bot => bot.id === routine.botId)?.name ?? "Bot"} · {intervals.find(interval => interval.value === routine.intervalMinutes)?.label ?? `Every ${routine.intervalMinutes} minutes`}</p>
            <p className="mt-4 whitespace-pre-wrap text-sm leading-6">{routine.instructions}</p>
            {routine.enabled ? <p className="mt-4 text-xs text-muted-foreground">Next: {new Date(routine.nextRunAt).toLocaleString()}</p> : null}
            {routine.lastError ? <p className="mt-3 rounded-lg bg-amber-50 p-3 text-sm text-amber-900">{routine.lastError}</p> : null}
            <div className="mt-5 flex flex-wrap items-center gap-4 text-sm">
              <button type="button" disabled={busy !== null} onClick={() => void toggle(routine)} className="underline disabled:opacity-50">{routine.enabled ? "Pause" : "Resume"}</button>
              <button type="button" disabled={busy !== null} onClick={() => void runOnce(routine)} className="underline disabled:opacity-50">Run once</button>
              <button type="button" disabled={busy !== null} onClick={() => edit(routine)} className="underline disabled:opacity-50">Edit</button>
              {routine.lastRunId ? <Link href={`/app/work/${routine.lastRunId}`} className="ml-auto underline">Latest work →</Link> : null}
            </div>
          </article>)}
          {!routines.length ? <div className="surface-card"><h3 className="font-medium">Delegate once. Make it a habit.</h3><p className="mt-2 text-sm leading-6 text-muted-foreground">Have a bot review new files, prepare a daily brief, or check a project regularly. Every occurrence gets its own progress, approvals, and result.</p></div> : null}
        </section>
        <form className="surface-card space-y-4" onSubmit={event => void save(event)}>
          <h2 className="font-semibold">{editing ? "Edit routine" : "Create a routine"}</h2>
          <div><label htmlFor="routine-name" className="text-sm font-medium">Name</label><input id="routine-name" className={inputClass} required maxLength={100} value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} placeholder="Morning project brief" /></div>
          <div><label htmlFor="routine-bot" className="text-sm font-medium">Bot</label><select id="routine-bot" className={inputClass} required value={form.botId} onChange={e => setForm({ ...form, botId: e.target.value })}><option value="">Choose a bot</option>{bots.filter(bot => bot.computerId).map(bot => <option key={bot.id} value={bot.id}>{bot.name}</option>)}</select></div>
          <div><label htmlFor="routine-task" className="text-sm font-medium">Assignment</label><textarea id="routine-task" className={`${inputClass} min-h-28`} required maxLength={100000} value={form.instructions} onChange={e => setForm({ ...form, instructions: e.target.value })} placeholder="Review project files and prepare a brief with changes, blockers, and next steps." /></div>
          <div><label htmlFor="routine-interval" className="text-sm font-medium">Repeat</label><select id="routine-interval" className={inputClass} value={form.intervalMinutes} onChange={e => setForm({ ...form, intervalMinutes: Number(e.target.value) })}>{intervals.map(interval => <option key={interval.value} value={interval.value}>{interval.label}</option>)}</select></div>
          <div><label htmlFor="routine-first" className="text-sm font-medium">First run · your local time</label><input id="routine-first" type="datetime-local" required className={inputClass} value={form.nextRunAt} onChange={e => setForm({ ...form, nextRunAt: e.target.value })} /></div>
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={form.enabled} onChange={e => setForm({ ...form, enabled: e.target.checked })} />Enable scheduled work</label>
          <p className="text-xs leading-5 text-muted-foreground">Repeats at a fixed interval. Missed times are combined into one assignment. Computer changes still need approval. Your Elsewhere host must be running.</p>
          <div className="flex gap-3"><Button type="submit" disabled={busy !== null}>{busy === "form" ? "Saving…" : "Save routine"}</Button>{editing ? <button type="button" onClick={() => { setEditing(null); setForm(emptyForm()); }} className="text-sm underline">Cancel edit</button> : null}</div>
        </form>
      </div>
    </div>
  );
}
