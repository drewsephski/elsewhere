"use client";
import { useEffect, useState } from "react";
import Link from "next/link";
import { Download, FileText } from "lucide-react";
import { cloudHostFetch } from "@/lib/cloud-api";

type ResultItem = { id: string; runId: string; name: string; kind: string; size: number; botName: string; task: string; createdAt: string };
type RunResults = { items: ResultItem[]; collecting: boolean; note: string | null };
export function ResultsPanel({ runId }: { runId?: string }) {
  const [items, setItems] = useState<ResultItem[]>([]);
  const [collecting, setCollecting] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function load() {
      try {
        const response = await cloudHostFetch(runId ? `/v1/runs/${runId}/results` : "/v1/results", { signal: controller.signal });
        if (!response.ok) throw new Error("Could not load saved results");
        const data: RunResults = runId ? await response.json() : { items: await response.json(), collecting: false, note: null };
        if (controller.signal.aborted) return;
        setItems(data.items); setCollecting(data.collecting); setNote(data.note); setError(null);
        if (data.collecting || !runId) timer = setTimeout(() => void load(), 5000);
      } catch (err) {
        if (controller.signal.aborted) return;
        setError(err instanceof Error ? err.message : "Could not load results");
        timer = setTimeout(() => void load(), 10000);
      } finally { if (!controller.signal.aborted) setLoading(false); }
    }
    void load();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [runId]);
  return <section className="surface-card">
    <div className="flex items-center gap-2"><FileText className="h-4 w-4 text-primary" /><h2 className="text-base font-semibold">{runId ? "Saved results" : "From your bots"}</h2></div>
    {error ? <p className="mt-3 text-sm text-red-700" role="alert">{error}</p> : null}
    {note ? <p className="mt-3 text-sm text-amber-700">{note}</p> : null}
    {!items.length ? <p className="mt-4 text-sm text-muted-foreground">{loading ? "Loading results…" : collecting ? "Results will be saved here when your bot finishes." : "No saved results yet. Delegate work to a bot and ask for a report, draft, or file."}</p> : <ul className="mt-4 divide-y divide-border">{items.map(item => <li key={item.id} className="flex flex-wrap items-center justify-between gap-3 py-4 first:pt-0 last:pb-0">
      <div className="min-w-0"><p className="break-words text-sm font-medium">{item.kind === "summary" ? "Assignment summary" : item.name}</p><p className="mt-1 text-xs text-muted-foreground">{item.botName} · {Math.max(1, Math.ceil(item.size / 1024))} KB · {new Date(item.createdAt).toLocaleString()}</p>{!runId ? <Link className="mt-2 block max-w-xl truncate text-sm text-muted-foreground underline underline-offset-4" href={`/app/work/${item.runId}`}>{item.task}</Link> : null}</div>
      <a href={`/api/results/${item.id}/download`} download={item.name} className="flex items-center gap-2 rounded-lg border border-border px-3 py-2 text-sm hover:bg-muted/40"><Download className="h-4 w-4" />Download<span className="sr-only"> {item.name}</span></a>
    </li>)}</ul>}
    {items.length && collecting ? <p className="mt-4 text-xs text-muted-foreground">Still collecting results…</p> : null}
  </section>;
}
