"use client";

import { ApprovalCard } from "@/components/app/approval-card";
import { cloudHostFetch } from "@/lib/cloud-api";
import { cloudApiErrorFromResponse } from "@/lib/cloud-api-error";
import { formatUserFacingError } from "@/lib/format-api-error";
import { HumanInterventionBanner } from "@/components/app/human-intervention-banner";
import type {
  BotSummary,
  ConversationSummary,
  CreateConversationResponse,
  CreateRunResponse,
  DelegationSummary,
  MessageAttachment,
  RunSummary,
} from "@/lib/api-types";
import { DelegationCard } from "@/components/app/delegation-card";
import { SubagentCard } from "@/components/app/subagent-card";
import { RunDelegationList } from "@/components/app/workspace/run-delegation-list";
import { AssistantMessageBubble } from "@/components/app/assistant-message-bubble";
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { archiveWorkRun, canArchiveWorkRun } from "@/lib/archive-work-run";
import { MessageDeleteButton } from "@/components/app/message-delete-button";
import { StatusPill } from "@/components/app/status-pill";
import { useActiveRun } from "@/contexts/active-run-context";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ChevronLeft, FileText, MessageSquare, Monitor, PanelRight, Plus } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useConversationIdentityLayout } from "@/hooks/use-conversation-identity-layout";
import { MarkdownContent } from "@/components/app/markdown-content";
import { botPresetPrompts } from "@/lib/bot-preset-prompts";
import { BotPresetPrompts } from "./bot-preset-prompts";
import { ChatComposerFrame, ChatComposerTextarea, ComposerIconButton } from "./chat-composer";
import {
  ComposerAttachmentStrip,
  COMPOSER_FILE_ACCEPT,
  composerCanSend,
  readyAttachmentIds,
  useComposerAttachments,
} from "./composer-attachments";
import { MessageAttachmentList } from "./message-attachments";
import { UserQuestionCard } from "@/components/app/user-question-card";
import { ChatResultCards } from "./chat-result-cards";
import { RunAssistantSnippet } from "./run-assistant-snippet";
import { WorkStatusCard } from "./work-status-card";
import { useOptionalBrowserPreviewContext } from "@/contexts/browser-preview-context";
import { FloatingBrowserPreview } from "./floating-browser-preview";
import { Spinner } from "@/components/ui/spinner";
import {
  bumpLoadScope,
  createLoadScopeRef,
  isActiveLoadScope,
} from "@/lib/conversation-load-scope";

function runIsActive(status: string): boolean {
  return status === "queued" || status === "running";
}

interface BotConversationViewProps {
  botId: string;
  onOpenContext?: () => void;
  onBotLoaded?: (bot: BotSummary) => void;
  /** Parent-owned bot snapshot (e.g. after settings save) to keep header in sync. */
  syncedBot?: BotSummary | null;
  onRenameBot?: (botId: string, name: string) => Promise<void>;
  onStreamRunIdChange?: (runId: string | null) => void;
  /** Desktop context rail is hidden; show an affordance to bring it back. */
  railCollapsed?: boolean;
  onExpandRail?: () => void;
}

export function BotConversationView({
  botId,
  onOpenContext,
  onBotLoaded,
  syncedBot = null,
  onRenameBot,
  onStreamRunIdChange,
  railCollapsed = false,
  onExpandRail,
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
    attachmentIds: string[];
    attachments?: MessageAttachment[];
    runId?: string;
  } | null>(null);
  const [liveRunId, setLiveRunId] = useState<string | null>(null);
  const [liveDelegations, setLiveDelegations] = useState<DelegationSummary[]>([]);
  const requestRef = useRef<{
    fingerprint: string;
    key: string;
  } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const stickToBottomRef = useRef(true);
  const loadScopeRef = useRef(createLoadScopeRef());
  const [conversationLoading, setConversationLoading] = useState(true);
  const { reset: resetComposerAttachments, ...composerFiles } = useComposerAttachments(
    botId ? { botId } : null,
  );

  const handleBotIdentityChange = useCallback(() => {
    bumpLoadScope(loadScopeRef.current);
    setRuns([]);
    setConversationId(null);
    setLiveRunId(null);
    setLiveDelegations([]);
    setPendingTurn(null);
    setMessage("");
    setError(null);
    setConversationLoading(true);
    stickToBottomRef.current = true;
    requestRef.current = null;
    resetComposerAttachments();
  }, [resetComposerAttachments]);

  useConversationIdentityLayout(botId, handleBotIdentityChange);

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

  const {
    detail: liveDetail,
    timeline,
    assistantStream,
    error: streamError,
    connection,
    pendingHumanIntervention,
  } = useActiveRun();

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

  const loadRuns = useCallback(
    async (activeConversationId: string | null, generation: number) => {
      if (!isActiveLoadScope(loadScopeRef.current, generation)) {
        return;
      }
      if (!activeConversationId) {
        setRuns([]);
        setLiveRunId(null);
        return;
      }
      const response = await cloudHostFetch(
        `/v1/runs?limit=40&bot_id=${encodeURIComponent(botId)}&conversation_id=${encodeURIComponent(activeConversationId)}`,
      );
      if (!isActiveLoadScope(loadScopeRef.current, generation)) {
        return;
      }
      if (!response.ok) {
        throw new Error("Could not load conversation history");
      }
      const rows: RunSummary[] = await response.json();
      if (!isActiveLoadScope(loadScopeRef.current, generation)) {
        return;
      }
      setRuns(rows);
      syncLiveRunId(rows);
      return rows;
    },
    [botId, syncLiveRunId],
  );

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
    if (!syncedBot || syncedBot.id !== botId) {
      return;
    }
    setBot(syncedBot);
  }, [botId, syncedBot]);

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
    const generation = loadScopeRef.current.current;
    void (async () => {
      try {
        const id = await resolveConversationId();
        if (!isActiveLoadScope(loadScopeRef.current, generation)) {
          return;
        }
        setConversationId(id);
        if (!id) {
          setRuns([]);
          return;
        }
        await loadRuns(id, generation);
      } catch (err) {
        if (isActiveLoadScope(loadScopeRef.current, generation)) {
          setError(err instanceof Error ? err.message : "Could not load conversation");
        }
      } finally {
        if (isActiveLoadScope(loadScopeRef.current, generation)) {
          setConversationLoading(false);
        }
      }
    })();
  }, [botId, loadRuns, resolveConversationId]);

  useEffect(() => {
    if (!conversationId) {
      return;
    }
    const generation = loadScopeRef.current.current;
    let timer: ReturnType<typeof setTimeout>;
    let cancelled = false;
    async function poll() {
      if (cancelled || !isActiveLoadScope(loadScopeRef.current, generation)) {
        return;
      }
      try {
        await loadRuns(conversationId, generation);
      } catch {
        /* ignore transient errors */
      }
      if (cancelled || !isActiveLoadScope(loadScopeRef.current, generation)) {
        return;
      }
      timer = setTimeout(() => void poll(), 5000);
    }
    void poll();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [conversationId, loadRuns]);

  useEffect(() => {
    if (!pendingTurn?.runId) {
      return;
    }
    if (runs.some((run) => run.runId === pendingTurn.runId)) {
      setPendingTurn(null);
    }
  }, [pendingTurn, runs]);

  function handleConversationScroll() {
    const element = scrollRef.current;
    if (!element) {
      return;
    }
    const distanceFromBottom = element.scrollHeight - element.scrollTop - element.clientHeight;
    stickToBottomRef.current = distanceFromBottom < 96;
  }

  useEffect(() => {
    if (!stickToBottomRef.current) {
      return;
    }
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [runs.length, timeline.length, liveDetail?.assistantResult]);

  useEffect(() => {
    stickToBottomRef.current = true;
  }, [botId, conversationId]);

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
      if (!response.ok) {
        throw await cloudApiErrorFromResponse(response, "Could not start a new chat");
      }
      const body = await response.json();
      const created = body as CreateConversationResponse;
      setConversationId(created.id);
      setRuns([]);
      setLiveRunId(null);
      setMessage("");
      resetComposerAttachments();
      requestRef.current = null;
    } catch (err) {
      setError(formatUserFacingError(err, "Could not start a new chat"));
    } finally {
      setStartingNewChat(false);
    }
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    const attachmentIds = readyAttachmentIds(composerFiles.files);
    const stagedAttachments = composerFiles.files
      .filter((file) => file.status === "ready" && file.attachment)
      .map((file) => file.attachment!);
    if (
      pending ||
      !bot?.computerId ||
      !composerCanSend(message, composerFiles.files)
    ) {
      return;
    }
    setError(null);
    const trimmed = message.trim();
    const fingerprint = `${trimmed}|${attachmentIds.join(",")}`;
    const idempotencyKey =
      requestRef.current?.fingerprint === fingerprint
        ? requestRef.current.key
        : crypto.randomUUID();
    requestRef.current = { fingerprint, key: idempotencyKey };
    setPendingTurn({
      idempotencyKey,
      message: trimmed,
      attachmentIds,
      attachments: stagedAttachments,
    });
    setMessage("");
    setPending(true);
    try {
      const payload: {
        botId: string;
        message: string;
        conversationId?: string;
        attachmentIds?: string[];
      } = {
        botId,
        message: trimmed,
      };
      if (conversationId) {
        payload.conversationId = conversationId;
      }
      if (attachmentIds.length) {
        payload.attachmentIds = attachmentIds;
      }
      const response = await cloudHostFetch("/v1/runs", {
        method: "POST",
        headers: { "Idempotency-Key": idempotencyKey },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        throw await cloudApiErrorFromResponse(response, "Could not delegate work");
      }
      const created = (await response.json()) as CreateRunResponse;
      setConversationId(created.conversationId);
      setLiveRunId(created.runId);
      setPendingTurn((previous) =>
        previous
          ? { ...previous, runId: created.runId }
          : {
              idempotencyKey,
              message: trimmed,
              attachmentIds,
              attachments: stagedAttachments,
              runId: created.runId,
            },
      );
      requestRef.current = null;
      resetComposerAttachments();
      const rows = await loadRuns(created.conversationId, loadScopeRef.current.current);
      const runVisible =
        rows?.some((run) => run.runId === created.runId) ??
        false;
      if (runVisible) {
        setPendingTurn(null);
      }
    } catch (err) {
      setMessage(trimmed);
      setPendingTurn(null);
      setError(formatUserFacingError(err, "Could not delegate work"));
    } finally {
      setPending(false);
    }
  }

  async function handleDeleteRun(run: RunSummary) {
    setError(null);
    try {
      await archiveWorkRun(run.runId);
      if (conversationId) {
        await loadRuns(conversationId, loadScopeRef.current.current);
      }
      if (liveRunId === run.runId) {
        setLiveRunId(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not archive from chat");
    }
  }

  const chronologicalRuns = [...runs].reverse();
  const canSend =
    composerCanSend(message, composerFiles.files) && Boolean(bot?.computerId) && !pending;
  const presetPrompts = useMemo(() => (bot ? botPresetPrompts(bot) : []), [bot]);
  const showPresetPrompts =
    Boolean(bot) &&
    !conversationLoading &&
    chronologicalRuns.length === 0 &&
    !pendingTurn;

  function handleSelectPreset(prompt: string) {
    setMessage(prompt);
    const placeholder = /\[[^\]]+\]/.exec(prompt);
    window.setTimeout(() => {
      const el = composerRef.current;
      if (!el) {
        return;
      }
      el.focus();
      if (placeholder && placeholder.index !== undefined) {
        el.setSelectionRange(placeholder.index, placeholder.index + placeholder[0].length);
        return;
      }
      el.setSelectionRange(el.value.length, el.value.length);
    }, 0);
  }

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      <header className="flex h-11 shrink-0 items-center justify-between gap-3 border-b border-border px-3">
        <div className="flex min-w-0 items-center gap-2">
          <Link
            href="/app"
            className="flex size-7 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-surface-hover lg:hidden"
            aria-label="Back to bots"
          >
            <ChevronLeft className="size-4" />
          </Link>
          <BotCreatureAvatar
            name={bot?.name ?? "Bot"}
            avatarId={bot?.avatarId ?? DEFAULT_BOT_AVATAR_ID}
            size="sm"
            animated={Boolean(streamRunId)}
          />
          {onRenameBot && bot ? (
            <InlineRenameLabel
              value={bot.name}
              onCommit={(next) => onRenameBot(bot.id, next)}
              className="text-[13px] font-medium leading-tight"
              inputClassName="text-[13px]"
              ariaLabel={`Rename ${bot.name}`}
            />
          ) : (
            <h1 className="truncate text-[13px] font-medium leading-tight">
              {bot?.name ?? "Bot"}
            </h1>
          )}
          {streamRunId ? (
            <StatusPill tone="info" live className="hidden sm:inline-flex">
              {connection ?? "Working"}
            </StatusPill>
          ) : null}
        </div>
        <div className="flex items-center gap-0.5">
          <ComposerIconButton
            label="New chat"
            className="size-7"
            disabled={startingNewChat || pending}
            onClick={() => void handleStartNewChat()}
          >
            <MessageSquare className="size-4" aria-hidden />
          </ComposerIconButton>
          <Link
            href="/app/computers"
            className="hidden size-7 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground sm:flex"
            aria-label="Computer settings"
            title="Computer settings"
          >
            <Monitor className="size-4" />
          </Link>
          <ComposerIconButton
            label="Bot details"
            className="size-7 lg:hidden"
            onClick={onOpenContext}
          >
            <PanelRight className="size-4" />
          </ComposerIconButton>
          {railCollapsed && onExpandRail ? (
            <ComposerIconButton
              label="Expand sidebar"
              className="hidden size-7 lg:flex"
              onClick={onExpandRail}
            >
              <PanelRight className="size-4" />
            </ComposerIconButton>
          ) : null}
        </div>
      </header>

      <div className="relative min-h-0 flex-1 overflow-hidden">
        <div
          ref={scrollRef}
          onScroll={handleConversationScroll}
          className="h-full min-h-0 overflow-y-auto"
        >
        <div
          className={`mx-auto flex min-h-full max-w-3xl flex-col gap-5 px-3 py-4 sm:px-5 ${
            showPresetPrompts ? "justify-center" : ""
          }`}
        >
          {!conversationLoading
            ? chronologicalRuns.map((run) => {
            const isLive = run.runId === streamRunId;
            const assistantText = isLive
              ? assistantStream.answerText ||
                (assistantStream.streaming ? "" : liveDetail?.assistantResult)
              : undefined;
            const showOptimisticUser =
              pendingTurn &&
              (isLive || !run.task?.trim()) &&
              run.runId === liveRunId;
            const finished = !isLive && !runIsActive(run.status);
            const deleteAction =
              canArchiveWorkRun(run.status) && !runIsActive(run.status) ? (
                <MessageDeleteButton
                  intent="archive"
                  onDelete={() => handleDeleteRun(run)}
                  className="h-7 rounded-lg px-2 text-xs text-muted-foreground hover:text-foreground"
                />
              ) : null;

            return (
              <div key={run.runId} className="space-y-2.5">
                <UserPromptBubble
                  sentAt={run.createdAt}
                  extra={
                    <MessageAttachmentList
                      attachments={
                        showOptimisticUser
                          ? (pendingTurn.attachments ?? [])
                          : (run.attachments ?? [])
                      }
                    />
                  }
                >
                  {showOptimisticUser ? pendingTurn.message : run.task}
                </UserPromptBubble>

                {isLive && liveDelegations.length > 0 ? (
                  <div className="space-y-2">
                    {liveDelegations.map((delegation) => (
                      <DelegationCard key={delegation.id} delegation={delegation} />
                    ))}
                  </div>
                ) : null}

                {isLive && pendingHumanIntervention ? (
                  <HumanInterventionBanner pending={pendingHumanIntervention} />
                ) : null}

                {(isLive ? timeline : []).map((item) =>
                  item.kind === "approval" ? (
                    <ApprovalCard
                      key={item.id}
                      payload={item.approval}
                      externalStatus={item.decision}
                    />
                  ) : item.kind === "subagent" ? (
                    <SubagentCard key={item.id} activity={item.subagent} />
                  ) : item.kind === "question" ? (
                    <UserQuestionCard
                      key={item.id}
                      question={item.question}
                      botName={bot?.name}
                    />
                  ) : (
                    <p key={item.id} className="text-center text-[11px] text-muted-foreground">
                      {item.text}
                    </p>
                  ),
                )}

                {finished ? (
                  <>
                    <RunDelegationList runId={run.runId} enabled={finished} />
                    <RunAssistantSnippet
                      runId={run.runId}
                      fallbackText={
                        run.runId === liveRunId
                          ? assistantStream.answerText ||
                            liveDetail?.assistantResult ||
                            undefined
                          : undefined
                      }
                      leading={
                        <BotCreatureAvatar
                          name={bot?.name ?? "Bot"}
                          avatarId={bot?.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                          size="xs"
                        />
                      }
                    />
                    <WorkStatusCard run={run} actions={deleteAction}>
                      <ChatResultCards runId={run.runId} />
                    </WorkStatusCard>
                  </>
                ) : null}

                {isLive ? (
                  <>
                    <AssistantMessageBubble
                      leading={
                        <BotCreatureAvatar
                          name={bot?.name ?? "Bot"}
                          avatarId={bot?.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                          size="xs"
                        />
                      }
                    >
                      {assistantStream.commentaryText ? (
                        <p className="mb-1.5 text-xs text-muted-foreground">
                          {assistantStream.commentaryText}
                        </p>
                      ) : null}
                      {assistantText ? (
                        <div>
                          <MarkdownContent text={assistantText} />
                          {assistantStream.streaming ? (
                            <span
                              className="ml-0.5 inline-block h-3.5 w-0.5 animate-pulse bg-foreground/80 align-middle"
                              aria-hidden
                            />
                          ) : null}
                        </div>
                      ) : (
                        <p className="text-muted-foreground">
                          {assistantStream.streaming
                            ? "Composing a reply…"
                            : "Your bot is working on this…"}
                        </p>
                      )}
                    </AssistantMessageBubble>
                    <WorkStatusCard run={run} status={liveDetail?.status ?? run.status}>
                      <ChatResultCards runId={run.runId} />
                    </WorkStatusCard>
                  </>
                ) : null}
              </div>
            );
          })
            : null}

          {pendingTurn &&
          !chronologicalRuns.some(
            (run) =>
              (pendingTurn.runId && run.runId === pendingTurn.runId) ||
              (runIsActive(run.status) && run.task?.trim() === pendingTurn.message),
          ) ? (
            <div className="space-y-2.5">
              <UserPromptBubble
                sentAt={new Date().toISOString()}
                extra={<MessageAttachmentList attachments={pendingTurn.attachments ?? []} />}
              >
                {pendingTurn.message}
              </UserPromptBubble>
              <p className="text-center text-[11px] text-muted-foreground">Sending…</p>
            </div>
          ) : null}

          {conversationLoading && !pendingTurn ? (
            <div className="flex justify-center py-16" role="status" aria-label="Loading conversation">
              <Spinner className="size-5 text-muted-foreground" />
            </div>
          ) : null}

          {!conversationLoading && !chronologicalRuns.length && !pendingTurn ? (
            <div className="mx-auto flex w-full max-w-lg flex-col items-center px-4 py-6 text-center">
              <BotCreatureAvatar
                name={bot?.name ?? "Bot"}
                avatarId={bot?.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                size="2xl"
              />
              <p className="mt-4 text-[15px] font-medium tracking-tight text-foreground">
                {bot?.name ? `What should ${bot.name} work on?` : "What should this Bot work on?"}
              </p>
              {showPresetPrompts ? (
                <div className="mt-5 w-full">
                  <BotPresetPrompts
                    prompts={presetPrompts}
                    onSelect={handleSelectPreset}
                    disabled={pending}
                  />
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
        </div>
        <FloatingBrowserPreview />
      </div>

      <footer className="shrink-0 px-3 pb-3 pt-1 sm:px-5">
        <div className="mx-auto max-w-3xl space-y-2">
          <ChatComposerFrame
            onSubmit={(event) => void handleSubmit(event)}
            canSend={canSend}
            pending={pending}
            onFiles={(files) => void composerFiles.addFiles(files)}
            attachments={
              <ComposerAttachmentStrip
                files={composerFiles.files}
                onRemove={composerFiles.removeFile}
              />
            }
            leading={
              <DropdownMenu>
                <DropdownMenuTrigger
                  render={<ComposerIconButton label="More" disabled={startingNewChat} />}
                >
                  <Plus className="size-4" aria-hidden />
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" side="top" sideOffset={8} className="w-44">
                  <DropdownMenuItem
                    disabled={startingNewChat || pending || composerFiles.files.length >= 4}
                    onClick={() => composerFiles.inputRef.current?.click()}
                  >
                    <FileText className="size-4" aria-hidden />
                    Add files
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    disabled={startingNewChat || pending}
                    onClick={() => void handleStartNewChat()}
                  >
                    <MessageSquare className="size-4" aria-hidden />
                    New chat
                  </DropdownMenuItem>
                  <DropdownMenuItem render={<Link href="/app/computers" />}>
                    <Monitor className="size-4" aria-hidden />
                    Computer settings
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            }
          >
            <input
              ref={composerFiles.inputRef}
              type="file"
              className="hidden"
              accept={COMPOSER_FILE_ACCEPT}
              multiple
              onChange={(event) => {
                const files = Array.from(event.target.files ?? []);
                event.target.value = "";
                void composerFiles.addFiles(files);
              }}
            />
            <ChatComposerTextarea
              ref={composerRef}
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              disabled={pending}
              maxLength={100000}
              placeholder={bot?.name ? `Message ${bot.name}` : "Message your bot"}
              aria-label="Message"
            />
          </ChatComposerFrame>
          {bot && !bot.computerId ? (
            <p className="px-3 text-[11px] text-warning">
              Assign a computer in bot settings before delegating work.
            </p>
          ) : null}
          {error || streamError ? (
            <p className="px-3 text-[11px] text-destructive" role="alert">
              {error ?? streamError}
            </p>
          ) : null}
        </div>
      </footer>
    </div>
  );
}
