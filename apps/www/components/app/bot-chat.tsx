"use client";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, CreateRunResponse } from "@/lib/api-types";
import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { RecentRunsPanel } from "./recent-runs-panel";

export function BotChat({ botId }: { botId: string }) {
  const router = useRouter();
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const request = useRef<{ message: string; key: string } | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/bots/${botId}`, { signal: controller.signal }).then(async response => {
      if (!response.ok) throw new Error("Bot not found");
      setBot(await response.json());
    }).catch(err => { if (!controller.signal.aborted) setError(err instanceof Error ? err.message : "Could not load bot"); });
    return () => controller.abort();
  }, [botId]);
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (pending || !message.trim()) return;
    setPending(true); setError(null);
    if (request.current?.message !== message.trim()) request.current = { message: message.trim(), key: crypto.randomUUID() };
    try {
      const response = await cloudHostFetch("/v1/runs", { method: "POST", headers: { "Idempotency-Key": request.current.key }, body: JSON.stringify({ botId, message: message.trim() }) });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not delegate work");
      router.push(`/app/work/${(body as CreateRunResponse).runId}`);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not delegate work"); }
    finally { setPending(false); }
  }
  return (
    <div className="mt-6 space-y-6">
      <section className="surface-card">
        <h2 className="text-xl font-semibold">{bot?.name ?? "Your bot"}</h2>
        <p className="mt-2 text-sm text-muted-foreground">Give your bot a job, useful context, and a clear description of the result you want.</p>
        <form className="mt-5 space-y-3" onSubmit={event => void submit(event)}>
          <label htmlFor="work-message" className="text-sm font-medium">What needs doing?</label>
          <textarea id="work-message" className="min-h-32 w-full rounded-xl border border-border bg-background px-4 py-3 text-sm" placeholder="Review the files on your computer and prepare a summary of what needs attention. Save your findings and include the sources." value={message} onChange={event => setMessage(event.target.value)} disabled={pending} maxLength={100000} />
          <div className="flex flex-wrap items-center justify-between gap-3"><p className="text-xs text-muted-foreground">Progress is saved. Computer changes need your approval.</p><Button type="submit" disabled={pending || !message.trim() || !bot?.computerId}>{pending ? "Saving work…" : "Delegate work"}</Button></div>
        </form>
        {bot && !bot.computerId ? <p className="mt-3 text-sm text-amber-700">Assign this bot a computer before delegating work.</p> : null}
        {error ? <p className="mt-3 text-sm text-red-700" role="alert">{error}</p> : null}
      </section>
      <RecentRunsPanel botId={botId} />
    </div>
  );
}
