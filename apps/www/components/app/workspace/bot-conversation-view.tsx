"use client";

import { ApprovalCard } from "@/components/app/approval-card";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, CreateRunResponse, RunSummary } from "@/lib/api-types";
import { formatMessageTime } from "@/lib/format";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { workStatus } from "@/lib/work-events";
import { useRunEventStream } from "@/hooks/use-run-event-stream";
import { Button } from "@/components/ui/button";
import { cn } from "cn";
import { ChevronLeft, Info, Monitor, PanelRight } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChatResultCards } from "./chat-result-cards";
import { RunAssistantSnippet } from "./run-assistant-snippet";

function runIsActive(status: string): boolean {
  return status === "queued" || status === "running";
}

interface BotConversationViewProps {
  botId: string;
  onOpenContext?: () => void;
  onBotLoaded?: (bot: BotSummary) => void;
  onRenameBot?: (botId: string, name: string) => Promise<void>;
}

export function BotConversationView({
  botId,
  onOpenContext,
  onBotLoaded,
  onRenameBot,
}: BotConversationViewProps) {
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [liveRunId, setLiveRunId] = useState<string | null>(null);
  const requestRef = useRef<{ message: string; key: string } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const activeRun = useMemo(() => {
    if (liveRunId) {
      return runs.find((run) => run.runId === liveRunId) ?? null;
    }
    return runs.find((run) => runIsActive(run.status)) ?? null;
  }, [runs, liveRunId]);

  const streamRunId = activeRun && runIsActive(activeRun.status) ? activeRun.runId : null;
  const { detail: liveDetail, timeline, error: streamError, connection } =
    useRunEventStream(streamRunId);

  const loadRuns = useCallback(async () => {
    const response = await cloudHostFetch(
      `/v1/runs?limit=40&bot_id=${encodeURIComponent(botId)}`,
    );
    if (!response.ok) {
      throw new Error("Could not load conversation history");
    }
    const rows: RunSummary[] = await response.json();
    setRuns(rows);
    const active = rows.find((run) => runIsActive(run.status));
    if (active) {
      setLiveRunId(active.runId);
    }
  }, [botId]);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/bots/${botId}`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error("Bot not found");
        }
        const loaded: BotSummary = await response.json();
        setBot(loaded);
        onBotLoaded?.(loaded);
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Could not load bot");
        }
      });
    return () => controller.abort();
  }, [botId, onBotLoaded]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        await loadRuns();
      } catch {
        /* ignore transient errors */
      }
      timer = setTimeout(() => void poll(), 5000);
    }
    void poll();
    return () => clearTimeout(timer);
  }, [loadRuns]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [runs.length, timeline.length, liveDetail?.assistantResult]);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (pending || !message.trim() || !bot?.computerId) {
      return;
    }
    setPending(true);
    setError(null);
    const trimmed = message.trim();
    if (requestRef.current?.message !== trimmed) {
      requestRef.current = { message: trimmed, key: crypto.randomUUID() };
    }
    try {
      const response = await cloudHostFetch("/v1/runs", {
        method: "POST",
        headers: { "Idempotency-Key": requestRef.current.key },
        body: JSON.stringify({ botId, message: trimmed }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not delegate work");
      }
      const created = body as CreateRunResponse;
      setLiveRunId(created.runId);
      setMessage("");
      requestRef.current = null;
      await loadRuns();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delegate work");
    } finally {
      setPending(false);
    }
  }

  const chronologicalRuns = [...runs].reverse();

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex shrink-0 items-center justify-between gap-3 border-b border-border/70 bg-white/80 px-4 py-3 backdrop-blur-md">
        <div className="flex min-w-0 items-center gap-2.5">
          <Link
            href="/app"
            className="flex size-9 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-muted lg:hidden"
            aria-label="Back to bots"
          >
            <ChevronLeft className="size-5" />
          </Link>
          <BotCreatureAvatar
            name={bot?.name ?? "Bot"}
            avatarId={bot?.avatarId ?? DEFAULT_BOT_AVATAR_ID}
            size="xl"
            animated={Boolean(streamRunId)}
          />
          <div className="min-w-0">
            {onRenameBot && bot ? (
              <InlineRenameLabel
                value={bot.name}
                onCommit={(next) => onRenameBot(bot.id, next)}
                className="text-base font-semibold leading-tight"
                inputClassName="text-base"
                ariaLabel={`Rename ${bot.name}`}
              />
            ) : (
              <h1 className="truncate text-base font-semibold leading-tight">
                {bot?.name ?? "Bot"}
              </h1>
            )}
            <p className="truncate text-xs text-muted-foreground">
              {streamRunId ? connection ?? "Working" : "Ready for your next message"}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onOpenContext}
            className="flex size-9 items-center justify-center rounded-full text-muted-foreground hover:bg-muted lg:hidden"
            aria-label="Bot details"
          >
            <PanelRight className="size-5" />
          </button>
          <Link
            href="/app/computers"
            className="hidden size-9 items-center justify-center rounded-full text-muted-foreground hover:bg-muted sm:flex"
            aria-label="Computer settings"
          >
            <Monitor className="size-5" />
          </Link>
        </div>
      </header>

      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
        <div className="mx-auto flex max-w-2xl flex-col gap-4">
          {chronologicalRuns.map((run) => {
            const isLive = run.runId === streamRunId;
            const assistantText =
              isLive && liveDetail?.assistantResult
                ? liveDetail.assistantResult
                : undefined;

            return (
              <div key={run.runId} className="space-y-3">
                <div className="flex justify-end">
                  <div className="max-w-[85%] rounded-3xl rounded-br-md bg-foreground px-4 py-2.5 text-sm text-primary-foreground shadow-sm">
                    <p className="whitespace-pre-wrap break-words">{run.task}</p>
                    <p className="mt-1 text-[10px] text-white/60">
                      {formatMessageTime(run.createdAt)}
                    </p>
                  </div>
                </div>

                {(isLive ? timeline : []).map((item) =>
                  item.kind === "approval" ? (
                    <ApprovalCard
                      key={item.id}
                      payload={item.approval}
                      externalStatus={item.decision}
                    />
                  ) : (
                    <p
                      key={item.id}
                      className="text-center text-xs text-muted-foreground"
                    >
                      {item.text}
                    </p>
                  ),
                )}

                {!isLive && run.status !== "queued" && run.status !== "running" ? (
                  <div className="flex justify-start">
                    <div className="max-w-[90%] rounded-3xl rounded-bl-md border border-border/80 bg-white px-4 py-3 text-sm shadow-sm">
                      <p className="text-xs font-medium text-muted-foreground">
                        {workStatus(run.status)}
                      </p>
                      <RunAssistantSnippet runId={run.runId} />
                      <Link
                        href={`/app/work/${run.runId}`}
                        className="mt-2 inline-flex items-center gap-1 text-xs text-primary underline-offset-2 hover:underline"
                      >
                        View full progress
                        <Info className="size-3" aria-hidden />
                      </Link>
                      <ChatResultCards runId={run.runId} />
                    </div>
                  </div>
                ) : null}

                {isLive ? (
                  <div className="flex justify-start">
                    <div className="max-w-[90%] rounded-3xl rounded-bl-md border border-border/80 bg-white px-4 py-3 text-sm shadow-sm">
                      <p className="text-xs font-medium text-primary">
                        {workStatus(liveDetail?.status ?? run.status)}
                      </p>
                      {assistantText ? (
                        <p className="mt-2 whitespace-pre-wrap break-words leading-relaxed">
                          {assistantText}
                        </p>
                      ) : (
                        <p className="mt-2 text-muted-foreground">
                          Your bot is working on this…
                        </p>
                      )}
                      <ChatResultCards runId={run.runId} />
                      <Link
                        href={`/app/work/${run.runId}`}
                        className="mt-3 inline-block text-xs text-muted-foreground underline-offset-2 hover:underline"
                      >
                        Open detailed work view
                      </Link>
                    </div>
                  </div>
                ) : null}
              </div>
            );
          })}

          {!chronologicalRuns.length ? (
            <div className="rounded-2xl border border-dashed border-border bg-white/50 px-6 py-10 text-center">
              <p className="text-sm font-medium">Start a conversation</p>
              <p className="mt-2 text-sm text-muted-foreground">
                Delegate a report, draft, or task. Progress and approvals stay in this thread.
              </p>
            </div>
          ) : null}
        </div>
      </div>

      <footer className="shrink-0 border-t border-border/70 bg-white/90 px-4 py-3 backdrop-blur-md">
        <form
          onSubmit={(event) => void handleSubmit(event)}
          className="mx-auto flex max-w-2xl items-end gap-2"
        >
          <div className="flex min-w-0 flex-1 items-center gap-1 rounded-full border border-border/80 bg-[#f5f3f8] px-2 py-1.5 shadow-sm focus-within:ring-2 focus-within:ring-primary/20">
            <textarea
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              disabled={pending}
              maxLength={100000}
              rows={1}
              placeholder={bot?.name ? `Message ${bot.name}` : "Message your bot"}
              className="max-h-32 min-h-[2.25rem] flex-1 resize-none bg-transparent px-2 py-1.5 text-sm outline-none"
              aria-label="Message"
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  event.currentTarget.form?.requestSubmit();
                }
              }}
            />
          </div>
          <Button type="submit" disabled={pending || !message.trim() || !bot?.computerId}>
            {pending ? "Sending…" : "Send"}
          </Button>
        </form>
        {bot && !bot.computerId ? (
          <p className="mx-auto mt-2 max-w-2xl text-xs text-amber-800">
            Assign a computer in bot settings before delegating work.
          </p>
        ) : null}
        {error || streamError ? (
          <p className="mx-auto mt-2 max-w-2xl text-xs text-red-700" role="alert">
            {error ?? streamError}
          </p>
        ) : null}
      </footer>
    </div>
  );
}
