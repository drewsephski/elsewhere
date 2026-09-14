"use client";

import { ApprovalCard } from "@/components/app/approval-card";
import { cloudHostFetch } from "@/lib/cloud-api";
import type {
  BotSummary,
  ConversationSummary,
  CreateConversationResponse,
  CreateRunResponse,
  DelegationSummary,
  RunSummary,
} from "@/lib/api-types";
import { DelegationCard } from "@/components/app/delegation-card";
import { RunDelegationList } from "@/components/app/workspace/run-delegation-list";
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { workStatus } from "@/lib/work-events";
import { useActiveRun } from "@/contexts/active-run-context";
import { Button } from "@/components/ui/button";
import { ChevronLeft, Info, MessageSquare, Monitor, PanelRight } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { MarkdownContent } from "@/components/app/markdown-content";
import { ChatResultCards } from "./chat-result-cards";
import { RunAssistantSnippet } from "./run-assistant-snippet";
import { useOptionalBrowserPreviewContext } from "@/contexts/browser-preview-context";
import { FloatingBrowserPreview } from "./floating-browser-preview";

function runIsActive(status: string): boolean {
  return status === "queued" || status === "running";
}

interface BotConversationViewProps {
  botId: string;
  onOpenContext?: () => void;
  onBotLoaded?: (bot: BotSummary) => void;
  onRenameBot?: (botId: string, name: string) => Promise<void>;
  onStreamRunIdChange?: (runId: string | null) => void;
}

export function BotConversationView({
  botId,
  onOpenContext,
  onBotLoaded,
  onRenameBot,
  onStreamRunIdChange,
}: BotConversationViewProps) {
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [startingNewChat, setStartingNewChat] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [pendingTurn, setPendingTurn] = useState<{
    idempotencyKey: string;
    message: string;
    runId?: string;
  } | null>(null);
  const [liveRunId, setLiveRunId] = useState<string | null>(null);
  const [liveDelegations, setLiveDelegations] = useState<DelegationSummary[]>([]);
  const requestRef = useRef<{ message: string; key: string } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const activeRun = useMemo(() => {
    if (liveRunId) {
      return runs.find((run) => run.runId === liveRunId) ?? null;
    }
    return runs.find((run) => runIsActive(run.status)) ?? null;
  }, [runs, liveRunId]);

  const streamRunId = useMemo(() => {
    if (liveRunId) {
      const tracked = runs.find((run) => run.runId === liveRunId);
      if (!tracked) {
        // Run was just created; history fetch may lag behind POST.
        return liveRunId;
      }
      if (runIsActive(tracked.status)) {
        return liveRunId;
      }
    }
    const active = runs.find((run) => runIsActive(run.status));
    return active?.runId ?? null;
  }, [liveRunId, runs]);

  const { detail: liveDetail, timeline, assistantStream, error: streamError, connection } =
    useActiveRun();

  useEffect(() => {
    if (!streamRunId) {
      setLiveDelegations([]);
      return;
    }
    const controller = new AbortController();
    cloudHostFetch(`/v1/runs/${streamRunId}/delegations`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) return;
        setLiveDelegations(await response.json());
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, [streamRunId, timeline.length]);

  useLayoutEffect(() => {
    onStreamRunIdChange?.(streamRunId);
  }, [onStreamRunIdChange, streamRunId]);
  const browserPreview = useOptionalBrowserPreviewContext();

  useEffect(() => {
    const element = scrollRef.current;
    if (!element || !browserPreview) {
      return;
    }
    const preview = browserPreview;
    const scrollRoot = element;
    function handleScroll() {
      if (scrollRoot.scrollTop < 72) {
        return;
      }
      if (
        !preview.frame?.available ||
        !preview.frame.imageDataUrl ||
        preview.pipDismissed ||
        preview.pipOpen
      ) {
        return;
      }
      preview.openPip();
    }
    element.addEventListener("scroll", handleScroll, { passive: true });
    return () => element.removeEventListener("scroll", handleScroll);
  }, [
    browserPreview,
    browserPreview?.frame?.available,
    browserPreview?.frame?.imageDataUrl,
    browserPreview?.pipDismissed,
    browserPreview?.pipOpen,
  ]);

  const syncLiveRunId = useCallback((rows: RunSummary[]) => {
    setLiveRunId((current) => {
      const active = rows.find((run) => runIsActive(run.status));
      if (active) {
        return active.runId;
      }
      if (!current) {
        return null;
      }
      const tracked = rows.find((run) => run.runId === current);
      if (!tracked) {
        // Keep tracking until the history endpoint catches up with a new run.
        return current;
      }
      if (runIsActive(tracked.status)) {
        return current;
      }
      return null;
    });
  }, []);

  const loadRuns = useCallback(async (activeConversationId: string | null) => {
    if (!activeConversationId) {
      setRuns([]);
      setLiveRunId(null);
      return;
    }
    const response = await cloudHostFetch(
      `/v1/runs?limit=40&bot_id=${encodeURIComponent(botId)}&conversation_id=${encodeURIComponent(activeConversationId)}`,
    );
    if (!response.ok) {
      throw new Error("Could not load conversation history");
    }
    const rows: RunSummary[] = await response.json();
    setRuns(rows);
    syncLiveRunId(rows);
    return rows;
  }, [botId, syncLiveRunId]);

  const resolveConversationId = useCallback(async () => {
    const response = await cloudHostFetch(
      `/v1/conversations?limit=1&bot_id=${encodeURIComponent(botId)}`,
    );
    if (!response.ok) {
      throw new Error("Could not load conversation");
    }
    const rows: ConversationSummary[] = await response.json();
    return rows[0]?.id ?? null;
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
    let cancelled = false;
    setMessage("");
    setError(null);
    setPendingTurn(null);
    setLiveRunId(null);
    void (async () => {
      try {
        const id = await resolveConversationId();
        if (cancelled) {
          return;
        }
        setConversationId(id);
        if (!id) {
          setRuns([]);
          return;
        }
        await loadRuns(id);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Could not load conversation");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [botId, loadRuns, resolveConversationId]);

  useEffect(() => {
    if (!conversationId) {
      return;
    }
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        await loadRuns(conversationId);
      } catch {
        /* ignore transient errors */
      }
      timer = setTimeout(() => void poll(), 5000);
    }
    void poll();
    return () => clearTimeout(timer);
  }, [conversationId, loadRuns]);

  useEffect(() => {
    if (!pendingTurn?.runId) {
      return;
    }
    if (runs.some((run) => run.runId === pendingTurn.runId)) {
      setPendingTurn(null);
    }
  }, [pendingTurn, runs]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [runs.length, timeline.length, liveDetail?.assistantResult]);

  async function handleStartNewChat() {
    if (startingNewChat || pending) {
      return;
    }
    setStartingNewChat(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/conversations", {
        method: "POST",
        body: JSON.stringify({ botId }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not start a new chat");
      }
      const created = body as CreateConversationResponse;
      setConversationId(created.id);
      setRuns([]);
      setLiveRunId(null);
      setMessage("");
      requestRef.current = null;
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not start a new chat");
    } finally {
      setStartingNewChat(false);
    }
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (pending || !message.trim() || !bot?.computerId) {
      return;
    }
    setError(null);
    const trimmed = message.trim();
    const idempotencyKey =
      requestRef.current?.message === trimmed
        ? requestRef.current.key
        : crypto.randomUUID();
    requestRef.current = { message: trimmed, key: idempotencyKey };
    setPendingTurn({ idempotencyKey, message: trimmed });
    setMessage("");
    setPending(true);
    try {
      const payload: { botId: string; message: string; conversationId?: string } = {
        botId,
        message: trimmed,
      };
      if (conversationId) {
        payload.conversationId = conversationId;
      }
      const response = await cloudHostFetch("/v1/runs", {
        method: "POST",
        headers: { "Idempotency-Key": idempotencyKey },
        body: JSON.stringify(payload),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not delegate work");
      }
      const created = body as CreateRunResponse;
      setConversationId(created.conversationId);
      setLiveRunId(created.runId);
      setPendingTurn((previous) =>
        previous
          ? { ...previous, runId: created.runId }
          : { idempotencyKey, message: trimmed, runId: created.runId },
      );
      requestRef.current = null;
      const rows = await loadRuns(created.conversationId);
      const runVisible =
        rows?.some((run) => run.runId === created.runId) ??
        false;
      if (runVisible) {
        setPendingTurn(null);
      }
    } catch (err) {
      setMessage(trimmed);
      setPendingTurn(null);
      setError(err instanceof Error ? err.message : "Could not delegate work");
    } finally {
      setPending(false);
    }
  }

  const chronologicalRuns = [...runs].reverse();

  return (
    <div className="relative flex h-full min-h-0 flex-col">
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
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="hidden rounded-full sm:inline-flex"
            disabled={startingNewChat || pending}
            onClick={() => void handleStartNewChat()}
          >
            New chat
          </Button>
          <button
            type="button"
            onClick={() => void handleStartNewChat()}
            disabled={startingNewChat || pending}
            className="flex size-9 items-center justify-center rounded-full text-muted-foreground hover:bg-muted disabled:opacity-50 sm:hidden"
            aria-label="Start new chat"
          >
            <MessageSquare className="size-5" aria-hidden />
          </button>
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
            const assistantText = isLive
              ? assistantStream.answerText ||
                (assistantStream.streaming ? "" : liveDetail?.assistantResult)
              : undefined;
            const showOptimisticUser =
              pendingTurn &&
              (isLive || !run.task?.trim()) &&
              run.runId === liveRunId;

            return (
              <div key={run.runId} className="space-y-3">
                <UserPromptBubble sentAt={run.createdAt}>
                  {showOptimisticUser ? pendingTurn.message : run.task}
                </UserPromptBubble>

                {isLive && liveDelegations.length > 0 ? (
                  <div className="space-y-2">
                    {liveDelegations.map((delegation) => (
                      <DelegationCard key={delegation.id} delegation={delegation} />
                    ))}
                  </div>
                ) : null}

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
                      <RunDelegationList
                        runId={run.runId}
                        enabled={!isLive && run.status !== "queued" && run.status !== "running"}
                      />
                      <p className="text-xs font-medium text-muted-foreground">
                        {workStatus(run.status)}
                      </p>
                      <RunAssistantSnippet
                        runId={run.runId}
                        fallbackText={
                          run.runId === liveRunId
                            ? assistantStream.answerText ||
                              liveDetail?.assistantResult ||
                              undefined
                            : undefined
                        }
                      />
                      <Link
                        href={`/app/work/${run.runId}`}
                        className="mt-2 inline-flex items-center gap-1 text-xs text-primary underline-offset-2 hover:underline"
                      >
                        View full progress
                        <Info className="size-3" aria-hidden />
                      </Link>
                      <ChatResultCards runId={run.runId} className="mt-3" />
                    </div>
                  </div>
                ) : null}

                {isLive ? (
                  <div className="flex justify-start">
                    <div className="max-w-[90%] rounded-3xl rounded-bl-md border border-border/80 bg-white px-4 py-3 text-sm shadow-sm">
                      <p className="text-xs font-medium text-primary">
                        {workStatus(liveDetail?.status ?? run.status)}
                      </p>
                      {assistantStream.commentaryText ? (
                        <p className="mt-1 text-xs text-muted-foreground">
                          {assistantStream.commentaryText}
                        </p>
                      ) : null}
                      {assistantText ? (
                        <div className="mt-2">
                          <MarkdownContent text={assistantText} />
                          {assistantStream.streaming ? (
                            <span
                              className="ml-0.5 inline-block h-4 w-0.5 animate-pulse bg-primary align-middle"
                              aria-hidden
                            />
                          ) : null}
                        </div>
                      ) : (
                        <p className="mt-2 text-muted-foreground">
                          {assistantStream.streaming
                            ? "Composing a reply…"
                            : "Your bot is working on this…"}
                        </p>
                      )}
                      <Link
                        href={`/app/work/${run.runId}`}
                        className="mt-3 inline-block text-xs text-muted-foreground underline-offset-2 hover:underline"
                      >
                        Open detailed work view
                      </Link>
                      <ChatResultCards runId={run.runId} className="mt-2" />
                    </div>
                  </div>
                ) : null}
              </div>
            );
          })}

          {pendingTurn &&
          !chronologicalRuns.some(
            (run) =>
              (pendingTurn.runId && run.runId === pendingTurn.runId) ||
              (runIsActive(run.status) && run.task?.trim() === pendingTurn.message),
          ) ? (
            <div className="space-y-3">
              <UserPromptBubble sentAt={new Date().toISOString()}>
                {pendingTurn.message}
              </UserPromptBubble>
              <p className="text-center text-xs text-muted-foreground">Sending…</p>
            </div>
          ) : null}

          {!chronologicalRuns.length && !pendingTurn ? (
            <div className="rounded-2xl border border-dashed border-border bg-white/50 px-6 py-10 text-center">
              <p className="text-sm font-medium">Start a conversation</p>
              <p className="mt-2 text-sm text-muted-foreground">
                Describe the outcome you want—a report, draft, or task on your computer. Follow
                live progress, results, and approval prompts right here in chat.
              </p>
            </div>
          ) : null}
        </div>
      </div>

      <FloatingBrowserPreview />

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
