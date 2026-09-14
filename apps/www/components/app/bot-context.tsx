"use client";
import { useCallback, useEffect, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";

type Context = { content: string; revision: number };
export function BotContext({ botId }: { botId: string }) {
  const [saved, setSaved] = useState<Context | null>(null);
  const [content, setContent] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");
  const load = useCallback(async () => {
    setBusy(true); setError(null);
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/context`);
      if (!response.ok) throw new Error("Could not load saved context");
      const body: Context = await response.json(); setSaved(body); setContent(body.content);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not load context"); }
    finally { setBusy(false); }
  }, [botId]);
  useEffect(() => { void load(); }, [load]);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (!saved || busy) return;
    setBusy(true); setError(null); setNotice("");
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/context`, { method: "PUT", body: JSON.stringify({ content, revision: saved.revision }) });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not save context");
      setSaved(body); setContent(body.content); setNotice("Saved. New assignments and routines will use this context.");
    } catch (err) { setError(err instanceof Error ? err.message : "Could not save context"); }
    finally { setBusy(false); }
  }
  return <section className="surface-card">
    <h2 className="text-base font-semibold">What your bot should remember</h2>
    <p className="mt-2 text-sm text-muted-foreground">Keep project facts, preferences, and recurring instructions here. You control this memory. Leave passwords and sensitive credentials out.</p>
    <form className="mt-4 space-y-3" onSubmit={event => void save(event)}>
      <label htmlFor="bot-context" className="sr-only">Saved context</label>
      <textarea id="bot-context" value={content} onChange={event => { setContent(event.target.value); setNotice(""); }} disabled={busy || !saved} maxLength={16000} placeholder="Our audience is independent designers. Keep briefs concise, cite primary sources, and save drafts for review." className="min-h-32 w-full rounded-xl border border-border bg-background p-3 text-sm leading-6" />
      <div className="flex flex-wrap items-center justify-between gap-3"><p className="text-xs text-muted-foreground">Applies to future work. Approvals still apply.</p><div className="flex gap-2">{error ? <Button type="button" variant="outline" disabled={busy} onClick={() => void load()}>Reload saved context</Button> : null}<Button type="submit" disabled={busy || !saved || content === saved.content}>{busy ? (saved ? "Saving…" : "Loading…") : "Save context"}</Button></div></div>
      {error ? <p role="alert" className="text-sm text-red-700">{error}</p> : null}
      {notice ? <p role="status" className="text-sm text-muted-foreground">{notice}</p> : null}
    </form>
  </section>;
}
