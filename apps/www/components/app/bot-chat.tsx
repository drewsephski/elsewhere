"use client";

import { cloudHostEventStream, cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, CreateRunResponse, RunDetail } from "@/lib/api-types";
import { useCallback, useEffect, useRef, useState } from "react";
import { ApprovalCard, type ApprovalRequestedPayload } from "@/components/app/approval-card";
import { Button } from "@/components/ui/button";

interface BotChatProps {
  botId: string;
}

interface ActivityLine {
  id: string;
  text: string;
}

interface TimelineItem {
  id: string;
  kind: "activity" | "approval";
  text?: string;
  approval?: ApprovalRequestedPayload;
  resolved?: boolean;
}

export function BotChat({ botId }: BotChatProps) {
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [message, setMessage] = useState("");
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [runId, setRunId] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [assistantResult, setAssistantResult] = useState<string | null>(null);
  const [timeline, setTimeline] = useState<TimelineItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const lastEventIdRef = useRef<string | undefined>(undefined);

  const loadBot = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/bots/${botId}`);
    if (!response.ok) {
      setError("Bot not found");
      return;
    }
    setBot((await response.json()) as BotSummary);
  }, [botId]);

  useEffect(() => {
    void loadBot();
  }, [loadBot]);

  async function refreshRunDetail(id: string) {
    const response = await cloudHostFetch(`/v1/runs/${id}`);
    if (!response.ok) {
      return;
    }
    const detail = (await response.json()) as RunDetail;
    setStatus(detail.status);
    if (detail.assistantResult) {
      setAssistantResult(detail.assistantResult);
    }
    if (detail.conversationId) {
      setConversationId(detail.conversationId);
    }
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!message.trim()) {
      return;
    }
    setError(null);
    setPending(true);
    setTimeline([]);
    setAssistantResult(null);
    abortRef.current?.abort();
    lastEventIdRef.current = undefined;

    try {
      const response = await cloudHostFetch("/v1/runs", {
        method: "POST",
        headers: {
          "Idempotency-Key": crypto.randomUUID(),
        },
        body: JSON.stringify({
          botId,
          conversationId: conversationId ?? undefined,
          message: message.trim(),
        }),
      });
      if (!response.ok) {
        const body = (await response.json().catch(() => ({}))) as { error?: string };
        throw new Error(body.error ?? `Run failed (${response.status})`);
      }
      const created = (await response.json()) as CreateRunResponse;
      setRunId(created.runId);
      setConversationId(created.conversationId);
      setStatus(created.status);
      setMessage("");

      const controller = new AbortController();
      abortRef.current = controller;
      void cloudHostEventStream(`/v1/runs/${created.runId}/events`, {
        signal: controller.signal,
        lastEventId: lastEventIdRef.current,
        onEvent: (event) => {
          if (event.id) {
            lastEventIdRef.current = event.id;
          }
          const id = event.id ?? `${Date.now()}-${event.event}`;
          if (event.event === "approval_requested") {
            try {
              const payload = JSON.parse(event.data) as ApprovalRequestedPayload;
              setTimeline((prev) => [
                ...prev,
                { id, kind: "approval", approval: payload },
              ]);
              return;
            } catch {
              // fall through to activity line
            }
          }
          if (event.event === "approval_resolved") {
            try {
              const body = JSON.parse(event.data) as { approvalId?: string };
              if (body.approvalId) {
                setTimeline((prev) =>
                  prev.map((item) =>
                    item.kind === "approval" && item.approval?.approvalId === body.approvalId
                      ? { ...item, resolved: true }
                      : item,
                  ),
                );
              }
            } catch {
              // ignore
            }
          }
          setTimeline((prev) => [
            ...prev,
            {
              id,
              kind: "activity",
              text: `${event.event}: ${event.data.slice(0, 240)}`,
            },
          ]);
        },
      })
        .catch(() => undefined)
        .finally(() => {
          void refreshRunDetail(created.runId);
        });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Run failed");
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  return (
    <div className="mt-8 border border-brand-dark/15 bg-white p-5">
      <h2 className="text-sm font-medium uppercase tracking-[0.15em]">Chat</h2>
      {bot ? (
        <p className="mt-2 text-xs text-brand-dark/55">
          Model {bot.model} · engine {bot.enginePreference}
        </p>
      ) : null}
      <form className="mt-4 flex flex-col gap-3 sm:flex-row" onSubmit={(e) => void handleSubmit(e)}>
        <textarea
          className="min-h-20 flex-1 border border-brand-dark/15 px-3 py-2 text-sm"
          placeholder="Ask Luna to use the computer…"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          disabled={pending}
        />
        <Button type="submit" disabled={pending || !message.trim()}>
          {pending ? "Running…" : "Send"}
        </Button>
      </form>
      {error ? (
        <p className="mt-3 text-sm text-red-700" role="alert">
          {error}
        </p>
      ) : null}
      {runId ? (
        <p className="mt-3 text-xs text-brand-dark/50">
          Run {runId.slice(0, 8)}… · {status ?? "unknown"}
        </p>
      ) : null}
      {timeline.length > 0 ? (
        <div className="mt-4 max-h-64 overflow-y-auto border border-brand-dark/10 bg-brand-cream/40 p-3 text-[11px] leading-relaxed">
          {timeline.map((item) =>
            item.kind === "approval" && item.approval ? (
              <ApprovalCard key={item.id} payload={item.approval} />
            ) : (
              <div key={item.id} className="font-mono">{item.text}</div>
            ),
          )}
        </div>
      ) : null}
      {assistantResult ? (
        <div className="mt-4 border-t border-brand-dark/10 pt-4">
          <h3 className="text-xs uppercase tracking-wide text-brand-dark/50">Answer</h3>
          <p className="mt-2 whitespace-pre-wrap text-sm">{assistantResult}</p>
        </div>
      ) : null}
    </div>
  );
}
