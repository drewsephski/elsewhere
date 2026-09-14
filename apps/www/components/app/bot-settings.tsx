"use client";
import { useEffect, useState } from "react";
import type { BotSummary, ComputerSummary } from "@/lib/api-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export function BotSettings({ bot, onSaved }: { bot: BotSummary; onSaved: (bot: BotSummary) => void }) {
  const [name, setName] = useState(bot.name);
  const [instructions, setInstructions] = useState(bot.instructions);
  const [computer, setComputer] = useState(bot.computerId ?? "");
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch("/v1/computers", { signal: controller.signal }).then(async response => {
      if (!response.ok) throw new Error("Could not load computers");
      setComputers(await response.json());
    }).catch(err => { if (!controller.signal.aborted) setError(err instanceof Error ? err.message : "Could not load computers"); });
    return () => controller.abort();
  }, []);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (busy) return; setBusy(true); setError(null); setNotice("");
    try {
      const response = await cloudHostFetch(`/v1/bots/${bot.id}`, { method: "PATCH", body: JSON.stringify({ name, instructions, computerId: computer }) });
      const body = await response.json(); if (!response.ok) throw new Error(body.error ?? "Could not update bot");
      onSaved(body); setNotice("Saved. Existing work keeps its original instructions and computer.");
    } catch (err) { setError(err instanceof Error ? err.message : "Could not update bot"); }
    finally { setBusy(false); }
  }
  return <details className="surface-card"><summary className="cursor-pointer text-base font-semibold">Bot settings</summary><form className="mt-5 space-y-4" onSubmit={event => void save(event)}>
    <div><Label htmlFor="settings-name">Name</Label><Input id="settings-name" value={name} onChange={event => setName(event.target.value)} maxLength={100} required /></div>
    <div><Label htmlFor="settings-role">Role and instructions</Label><textarea id="settings-role" className="mt-1 min-h-28 w-full rounded-xl border border-border bg-background p-3 text-sm" value={instructions} onChange={event => setInstructions(event.target.value)} maxLength={16000} /></div>
    <div><Label htmlFor="settings-computer">Assigned computer</Label><select id="settings-computer" className="mt-1 w-full rounded-lg border border-border bg-background p-3 text-sm" value={computer} onChange={event => setComputer(event.target.value)}><option value="">No computer assigned</option>{computer && !computers.some(item => item.id === computer) ? <option value={computer}>Current computer unavailable</option> : null}{computers.map(item => <option key={item.id} value={item.id}>{item.displayName}</option>)}</select><p className="mt-2 text-xs text-muted-foreground">Changing computers does not move files. Queued work stays on its original computer.</p></div>
    <Button type="submit" disabled={busy || !name.trim()}>{busy ? "Saving…" : "Save settings"}</Button>
    {notice ? <p role="status" className="text-sm text-muted-foreground">{notice}</p> : null}{error ? <p role="alert" className="text-sm text-red-700">{error}</p> : null}
  </form></details>;
}
