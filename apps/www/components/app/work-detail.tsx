"use client";
import { useEffect, useState } from "react";
import Link from "next/link";
import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { RunDetail } from "@/lib/api-types";
import { activityText, workStatus } from "@/lib/work-events";
import { ApprovalCard, type ApprovalRequestedPayload, type ApprovalTerminalState } from "./approval-card";

type Activity = { id: string; text?: string; approval?: ApprovalRequestedPayload; decision?: ApprovalTerminalState };
const active = (status: string) => status === "queued" || status === "running";

export function WorkDetail({ runId }: { runId: string }) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [timeline, setTimeline] = useState<Activity[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState("Connecting to your work…");
  const [stopping, setStopping] = useState(false);
  useEffect(() => {
    const controller = new AbortController();
    let lastEventId: string | undefined;
    let timer: ReturnType<typeof setTimeout>;
    setDetail(null); setTimeline([]); setError(null);
    async function sync() {
      try {
        const response = await cloudHostFetch(`/v1/runs/${runId}`, { signal: controller.signal });
        if (!response.ok) throw new Error(response.status === 404 ? "Work not found" : "Could not load this work");
        const current: RunDetail = await response.json();
        if (controller.signal.aborted) return;
        setDetail(current);
        setConnection(active(current.status) ? "Following progress. You can leave this page." : "Saved work history");
        setError(null);
        await cloudHostEventStream(`/v1/runs/${runId}/events`, {
          signal: controller.signal, lastEventId,
          onEvent(event) {
            if (event.id) lastEventId = event.id;
            const payload: Record<string, unknown> = JSON.parse(event.data);
            if (event.event === "stream_error") throw new Error("Progress connection interrupted");
            if (typeof payload.status === "string") setDetail(previous => previous ? { ...previous, status: payload.status as string } : previous);
            const id = event.id ?? `terminal-${runId}`;
            if (event.event === "approval_requested" && typeof payload.approvalId === "string" && typeof payload.summary === "string") {
              const approval: ApprovalRequestedPayload = { approvalId: payload.approvalId, summary: payload.summary, tool: String(payload.tool ?? ""), operationKind: String(payload.operationKind ?? "") };
              setTimeline(previous => previous.some(item => item.id === id) ? previous : [...previous, { id, approval }]);
            } else if (event.event === "approval_resolved") {
              const decision = payload.decision;
              if (["approved", "denied", "cancelled", "expired"].includes(String(decision))) {
                setTimeline(previous => previous.map(item => item.approval?.approvalId === payload.approvalId ? { ...item, decision: decision as ApprovalTerminalState } : item));
              }
            } else {
              const text = activityText(event.event, payload);
              if (text) setTimeline(previous => previous.some(item => item.id === id) ? previous : [...previous.slice(-199), { id, text }]);
            }
          },
        });
        if (controller.signal.aborted) return;
        const refreshed = await cloudHostFetch(`/v1/runs/${runId}`, { signal: controller.signal });
        if (!refreshed.ok) throw new Error("Could not refresh work status");
        const result: RunDetail = await refreshed.json();
        setDetail(result);
        if (active(result.status)) timer = setTimeout(() => void sync(), 1500);
        else setConnection("Saved work history");
      } catch (err) {
        if (controller.signal.aborted) return;
        setError(err instanceof Error ? err.message : "Could not follow progress");
        setConnection("Reconnecting. Your work continues on the server.");
        timer = setTimeout(() => void sync(), 5000);
      }
    }
    void sync();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [runId]);

  async function stop() {
    setStopping(true);
    try {
      const response = await cloudHostFetch(`/v1/runs/${runId}/cancel`, { method: "POST" });
      if (!response.ok) throw new Error("Could not stop work. Try again.");
      setDetail(await response.json());
    } catch (err) { setError(err instanceof Error ? err.message : "Could not stop work"); }
    finally { setStopping(false); }
  }

  return (
    <section className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0 max-w-3xl"><p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">Assignment</p><h1 className="mt-2 whitespace-pre-wrap break-words text-xl font-semibold">{detail?.task ?? "Delegated work"}</h1><p className="mt-3 text-sm text-muted-foreground" role="status">{connection}</p></div>
        <div className="flex items-center gap-3"><span className="rounded-full bg-muted px-3 py-1.5 text-sm">{detail ? workStatus(detail.status) : "Loading…"}</span>{detail && active(detail.status) ? <button type="button" disabled={stopping} onClick={() => void stop()} className="rounded-lg border border-border px-3 py-1.5 text-sm disabled:opacity-50">{stopping ? "Stopping…" : "Stop work"}</button> : null}</div>
      </div>
      {error ? <p className="text-sm text-red-700" role="alert">{error}</p> : null}
      {detail?.status === "interrupted" ? <div className="rounded-xl border border-amber-300 bg-amber-50 p-4 text-sm">This work was interrupted. Actions already taken have not been repeated. Review the progress below, then <Link href={`/app/bots/${detail.botId}`} className="underline">give your bot a follow-up</Link> to continue safely.</div> : null}
      {detail?.status === "failed" ? <p className="text-sm text-red-700">Your bot could not finish this assignment. Review its progress and connection, then send a follow-up.</p> : null}
      {detail?.assistantResult ? <section className="surface-card"><h2 className="text-base font-semibold">{active(detail.status) ? "Work so far" : "Result"}</h2><p className="mt-4 whitespace-pre-wrap break-words text-sm leading-7">{detail.assistantResult}</p></section> : null}
      <section className="surface-card"><h2 className="text-base font-semibold">Progress</h2><ol className="mt-4 space-y-3">{timeline.map(item => <li key={item.id}>{item.approval ? <ApprovalCard payload={item.approval} externalStatus={item.decision} /> : <p className="border-l-2 border-border pl-4 text-sm text-muted-foreground">{item.text}</p>}</li>)}</ol>{!timeline.length ? <p className="mt-4 text-sm text-muted-foreground">Your bot’s activity will appear here.</p> : null}</section>
      {detail ? <Link href={`/app/bots/${detail.botId}`} className="inline-block text-sm underline underline-offset-4">Back to your bot</Link> : null}
    </section>
  );
}
