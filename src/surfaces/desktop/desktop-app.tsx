import { useCallback, useEffect, useRef, useState } from "react";
import type { Bot, Message, ModelDescriptor } from "@desktop/lib/definitions";
import { DEFAULT_MODEL_ID, resolveDefaultModelId } from "@desktop/lib/definitions";
import { useChatStreamListener } from "@desktop/hooks/use-chat-stream";
import { shouldCommitChatLoad } from "@desktop/lib/chat-load-guard";
import {
  applyChatStreamEventToMessages,
  applyChatStreamEventsToMessages,
} from "@desktop/providers/chat-stream-state";
import {
  drainPreAckStreamEvents,
  pushPreAckStreamEvent,
} from "@desktop/providers/pre-ack-stream-buffer";
import type { ProviderStreamEvent } from "@desktop/providers/types";
import { isDemoAgent } from "@desktop/lib/demo-agent";
import { botService } from "@desktop/services/bot-service";
import { chatService } from "@desktop/services/chat-service";
import { tauriApi } from "@desktop/lib/tauri-api";
import { Alert, AlertAction, AlertDescription } from "@desktop/components/ui/alert";
import { Button } from "@desktop/components/ui/button";
import { XIcon } from "@desktop/components/icons/lucide";
import { Skeleton } from "@desktop/components/ui/skeleton";
import { BotSidebar } from "@desktop/ui/bot-sidebar";
import { ChatComposer } from "@desktop/ui/chat-composer";
import { ChatHeader } from "@desktop/ui/chat-header";
import { ContextSidebar } from "@desktop/ui/context-sidebar";
import { CreateBotModal } from "@desktop/ui/create-bot-modal";
import { EmptyChat } from "@desktop/ui/empty-chat";
import { MessageBubble } from "@desktop/ui/message-bubble";
import { SettingsModal } from "@desktop/ui/settings-modal";

function formatInvokeError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "An unexpected error occurred";
}

export default function DesktopApp() {
  const [bots, setBots] = useState<Bot[]>([]);
  const [selectedBotId, setSelectedBotId] = useState<string | null>(null);
  const [botDraft, setBotDraft] = useState<Bot | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [composer, setComposer] = useState("");
  const [models, setModels] = useState<ModelDescriptor[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [globalError, setGlobalError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [apiKeyConfigured, setApiKeyConfigured] = useState(false);
  const [apiKeyBannerDismissed, setApiKeyBannerDismissed] = useState(false);
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createPrompt, setCreatePrompt] = useState("");
  const [createModel, setCreateModel] = useState(DEFAULT_MODEL_ID);
  const [createError, setCreateError] = useState<string | null>(null);
  const [botSaving, setBotSaving] = useState(false);
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);
  const [loadingChat, setLoadingChat] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const userScrolledUpRef = useRef(false);
  const loadChatEpochRef = useRef(0);
  const selectedBotIdRef = useRef<string | null>(selectedBotId);
  selectedBotIdRef.current = selectedBotId;
  const ackPendingRequestIdRef = useRef<string | null>(null);
  const preAckStreamBufferRef = useRef<ProviderStreamEvent[]>([]);

  const selectedBot = bots.find((b) => b.id === selectedBotId) ?? null;
  const displayBot = botDraft ?? selectedBot;
  const isStreaming = activeRequestId !== null;

  const refreshBots = useCallback(async () => {
    try {
      const list = await botService.bootstrap();
      setBots(list);
      setSelectedBotId((current) => {
        if (current && list.some((bot) => bot.id === current)) {
          return current;
        }
        if (list.length === 0) {
          return null;
        }
        const preferred = list.find((bot) => isDemoAgent(bot)) ?? list[0];
        return preferred.id;
      });
    } catch (error) {
      setGlobalError(formatInvokeError(error));
    }
  }, []);

  const refreshApiKeyStatus = useCallback(async () => {
    const status = await tauriApi.getApiKeyStatus();
    setApiKeyConfigured(status.configured);
  }, []);

  const loadModels = useCallback(async () => {
    if (!apiKeyConfigured) {
      setModels([]);
      setModelsError("Configure your OpenAI API key in Settings.");
      return;
    }
    setModelsLoading(true);
    setModelsError(null);
    try {
      const list = await tauriApi.listOpenAiModels();
      setModels(list);
      setCreateModel((current) => {
        if (list.some((m) => m.id === current)) {
          return current;
        }
        return resolveDefaultModelId(list);
      });
    } catch (error) {
      setModelsError(formatInvokeError(error));
    } finally {
      setModelsLoading(false);
    }
  }, [apiKeyConfigured]);

  const loadChat = useCallback(async (botId: string) => {
    const epoch = ++loadChatEpochRef.current;
    setLoadingChat(true);
    setGlobalError(null);
    try {
      const { conversation, messages: loaded } =
        await chatService.loadConversation(botId);
      if (
        !shouldCommitChatLoad(
          epoch,
          loadChatEpochRef.current,
          botId,
          selectedBotIdRef.current,
        )
      ) {
        return;
      }
      setConversationId(conversation.id);
      setMessages(loaded);
      setBotDraft(null);
    } catch (error) {
      if (
        !shouldCommitChatLoad(
          epoch,
          loadChatEpochRef.current,
          botId,
          selectedBotIdRef.current,
        )
      ) {
        return;
      }
      setGlobalError(formatInvokeError(error));
    } finally {
      if (
        shouldCommitChatLoad(
          epoch,
          loadChatEpochRef.current,
          botId,
          selectedBotIdRef.current,
        )
      ) {
        setLoadingChat(false);
      }
    }
  }, []);

  useEffect(() => {
    void refreshBots();
    void refreshApiKeyStatus();
  }, [refreshBots, refreshApiKeyStatus]);

  useEffect(() => {
    if (selectedBotId) {
      void loadChat(selectedBotId);
    }
  }, [selectedBotId, loadChat]);

  useEffect(() => {
    if (apiKeyConfigured) {
      void loadModels();
    }
  }, [apiKeyConfigured, loadModels]);

  useEffect(() => {
    if (selectedBot) {
      setBotDraft(null);
    }
  }, [selectedBot]);

  function handleStreamTerminal(event: ProviderStreamEvent) {
    if (event.type === "error") {
      setGlobalError(event.error ?? "Stream failed");
    }
    setActiveRequestId(null);
    if (selectedBotIdRef.current) {
      void loadChat(selectedBotIdRef.current);
    }
  }

  useChatStreamListener((event) => {
    if (activeRequestId && event.requestId !== activeRequestId) {
      return;
    }
    if (ackPendingRequestIdRef.current === event.requestId) {
      pushPreAckStreamEvent(preAckStreamBufferRef.current, event);
      return;
    }
    if (event.type === "error") {
      handleStreamTerminal(event);
      return;
    }
    if (event.type === "done" || event.type === "cancelled") {
      handleStreamTerminal(event);
      return;
    }
    if (event.type === "delta" || event.type === "message") {
      setMessages((prev) => applyChatStreamEventToMessages(prev, event));
    }
  });

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el || userScrolledUpRef.current) {
      return;
    }
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  function handleScroll() {
    const el = scrollContainerRef.current;
    if (!el) {
      return;
    }
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    userScrolledUpRef.current = distanceFromBottom > 80;
  }

  async function handleSaveBot() {
    if (!displayBot) {
      return;
    }
    setBotSaving(true);
    setGlobalError(null);
    try {
      const updated = await botService.update({
        id: displayBot.id,
        name: displayBot.name,
        systemPrompt: displayBot.systemPrompt,
        model: displayBot.model,
      });
      setBots((prev) => prev.map((b) => (b.id === updated.id ? updated : b)));
      setBotDraft(null);
    } catch (error) {
      setGlobalError(formatInvokeError(error));
    } finally {
      setBotSaving(false);
    }
  }

  async function handleRenameBot(id: string, name: string) {
    const trimmed = name.trim();
    if (!trimmed) {
      return;
    }
    const existing = bots.find((bot) => bot.id === id);
    if (!existing || existing.name === trimmed) {
      return;
    }
    setGlobalError(null);
    try {
      const updated = await botService.update({ id, name: trimmed });
      setBots((prev) => prev.map((b) => (b.id === updated.id ? updated : b)));
      setBotDraft((prev) =>
        prev?.id === updated.id ? { ...prev, name: updated.name } : prev,
      );
    } catch (error) {
      setGlobalError(formatInvokeError(error));
    }
  }

  async function handleArchiveBot() {
    if (!displayBot) {
      return;
    }
    try {
      await botService.archive(displayBot.id);
      loadChatEpochRef.current += 1;
      setSelectedBotId(null);
      setConversationId(null);
      setMessages([]);
      await refreshBots();
    } catch (error) {
      setGlobalError(formatInvokeError(error));
    }
  }

  async function handleSend() {
    if (!selectedBot || !composer.trim() || isStreaming) {
      return;
    }
    if (!apiKeyConfigured) {
      setSettingsOpen(true);
      return;
    }
    const content = composer.trim();
    setComposer("");
    setGlobalError(null);
    userScrolledUpRef.current = false;

    const optimisticUser: Message = {
      id: `temp-user-${Date.now()}`,
      conversationId: conversationId ?? "",
      role: "user",
      kind: "text",
      body: content,
      status: "complete",
      model: null,
      errorMessage: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };
    const optimisticAssistant: Message = {
      id: `temp-assistant-${Date.now()}`,
      conversationId: conversationId ?? "",
      role: "assistant",
      kind: "text",
      body: "",
      status: "streaming",
      model: selectedBot.model,
      errorMessage: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    };
    const requestId = crypto.randomUUID();
    ackPendingRequestIdRef.current = requestId;
    preAckStreamBufferRef.current = [];
    setActiveRequestId(requestId);
    setMessages((prev) => [...prev, optimisticUser, optimisticAssistant]);

    try {
      const result = await chatService.sendMessage({
        botId: selectedBot.id,
        providerId: selectedBot.provider,
        conversationId: conversationId ?? undefined,
        content,
        requestId,
        useAgent: selectedBot.computerEnabled,
      });
      const preAckEvents = drainPreAckStreamEvents(
        preAckStreamBufferRef.current,
      );
      ackPendingRequestIdRef.current = null;

      setConversationId(result.conversationId);
      setMessages((prev) => {
        const withoutTemp = prev.filter((m) => !m.id.startsWith("temp-"));
        const mapped: Message[] = [
          ...withoutTemp,
          {
            ...optimisticUser,
            id: result.userMessageId,
            conversationId: result.conversationId,
          },
          {
            ...optimisticAssistant,
            id: result.assistantMessageId,
            conversationId: result.conversationId,
          },
        ];
        return applyChatStreamEventsToMessages(mapped, preAckEvents);
      });

      for (const event of preAckEvents) {
        if (
          event.type === "error" ||
          event.type === "done" ||
          event.type === "cancelled"
        ) {
          handleStreamTerminal(event);
        }
      }
    } catch (error) {
      ackPendingRequestIdRef.current = null;
      preAckStreamBufferRef.current = [];
      setActiveRequestId(null);
      setGlobalError(formatInvokeError(error));
      void loadChat(selectedBot.id);
    }
  }

  async function handleStop() {
    if (!activeRequestId || !selectedBot) {
      return;
    }
    try {
      await chatService.cancel(selectedBot.provider, activeRequestId);
    } catch (error) {
      setGlobalError(formatInvokeError(error));
    }
  }

  async function handleCreateBot() {
    setCreateError(null);
    try {
      const bot = await botService.create({
        name: createName.trim(),
        systemPrompt: createPrompt,
        model: createModel,
        provider: "openai",
      });
      setCreateOpen(false);
      setCreateName("");
      setCreatePrompt("");
      await refreshBots();
      setSelectedBotId(bot.id);
    } catch (error) {
      setCreateError(formatInvokeError(error));
    }
  }

  async function handleSaveApiKey() {
    setSettingsSaving(true);
    setSettingsError(null);
    try {
      await tauriApi.setOpenAiApiKey(apiKeyDraft);
      setApiKeyDraft("");
      await refreshApiKeyStatus();
      await loadModels();
      setSettingsOpen(false);
    } catch (error) {
      setSettingsError(formatInvokeError(error));
    } finally {
      setSettingsSaving(false);
    }
  }

  async function handleClearApiKey() {
    setSettingsSaving(true);
    try {
      await tauriApi.clearOpenAiApiKey();
      await refreshApiKeyStatus();
      setModels([]);
    } catch (error) {
      setSettingsError(formatInvokeError(error));
    } finally {
      setSettingsSaving(false);
    }
  }

  function handleOpenCreateBot() {
    setCreateOpen(true);
    setCreateError(null);
    setCreateModel(resolveDefaultModelId(models));
  }

  function handleOpenSettings() {
    setSettingsOpen(true);
    setSettingsError(null);
  }

  return (
    <div className="workspace-window flex h-full bg-background text-foreground">
      <BotSidebar
        bots={bots}
        selectedBotId={selectedBotId}
        onSelectBot={setSelectedBotId}
        onCreateBot={handleOpenCreateBot}
        onOpenSettings={handleOpenSettings}
        onRenameBot={handleRenameBot}
        apiKeyConfigured={apiKeyConfigured}
        isStreaming={isStreaming}
      />

      <main className="flex min-w-0 flex-1 flex-col bg-background">
        <ChatHeader
          bot={selectedBot}
          mobileNavOpen={mobileNavOpen}
          onMobileNavOpenChange={setMobileNavOpen}
          bots={bots}
          selectedBotId={selectedBotId}
          onSelectBot={setSelectedBotId}
          onCreateBot={handleOpenCreateBot}
          onOpenSettings={handleOpenSettings}
          onRenameBot={handleRenameBot}
          apiKeyConfigured={apiKeyConfigured}
          isStreaming={isStreaming}
        />

        {!apiKeyConfigured && !apiKeyBannerDismissed && (
          <div className="min-w-0 px-3 pt-2 sm:px-4">
            <Alert className="border-amber-500/30 bg-amber-500/10">
              <AlertDescription className="flex min-w-0 flex-wrap items-center gap-2 pr-1 text-amber-950/90 dark:text-amber-100/90">
                <span className="min-w-0">
                  Add your OpenAI API key to start chatting.
                </span>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 border-amber-600/40 bg-transparent text-amber-900 hover:bg-amber-500/20 dark:border-amber-500/40 dark:text-amber-50 dark:hover:bg-amber-500/15"
                  onClick={handleOpenSettings}
                >
                  Open settings
                </Button>
              </AlertDescription>
              <AlertAction>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  className="text-amber-900/80 hover:bg-amber-500/20 hover:text-amber-950 dark:text-amber-50/80 dark:hover:bg-amber-500/15 dark:hover:text-amber-50"
                  aria-label="Dismiss API key reminder"
                  onClick={() => setApiKeyBannerDismissed(true)}
                >
                  <XIcon size={16} className="pointer-events-none" />
                </Button>
              </AlertAction>
            </Alert>
          </div>
        )}

        {globalError && (
          <div className="min-w-0 px-3 pt-2 sm:px-4">
            <Alert variant="destructive">
            <AlertDescription className="min-w-0 break-words">{globalError}</AlertDescription>
          </Alert>
          </div>
        )}

        <div
          ref={scrollContainerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto"
        >
          <div className="mx-auto flex w-full max-w-3xl flex-col gap-5 px-3 py-4 sm:px-5">
            {!selectedBot && (
              <EmptyChat
                apiKeyConfigured={apiKeyConfigured}
                onCreateBot={handleOpenCreateBot}
                onOpenSettings={handleOpenSettings}
              />
            )}
            {selectedBot && loadingChat && messages.length === 0 && (
              <div className="space-y-4 py-4" aria-busy="true">
                <Skeleton className="h-16 w-[85%] rounded-2xl" />
                <Skeleton className="ml-auto h-12 w-[60%] rounded-2xl" />
                <Skeleton className="h-20 w-[75%] rounded-2xl" />
              </div>
            )}
            {messages.map((message) => (
              <MessageBubble key={message.id} message={message} />
            ))}
            <div ref={messagesEndRef} />
          </div>
        </div>

        <ChatComposer
          value={composer}
          disabled={!selectedBot || loadingChat}
          isStreaming={isStreaming}
          recipientName={selectedBot?.name ?? null}
          onChange={setComposer}
          onSend={handleSend}
          onStop={handleStop}
        />
      </main>

      <ContextSidebar
        bot={selectedBot}
        displayBot={displayBot}
        models={models}
        modelsLoading={modelsLoading}
        modelsError={modelsError}
        onBotChange={(patch) => {
          if (!displayBot) {
            return;
          }
          setBotDraft((prev) => ({
            ...(prev ?? displayBot),
            ...patch,
          }));
        }}
        onSaveBot={handleSaveBot}
        onArchiveBot={handleArchiveBot}
        onRenameBot={handleRenameBot}
        botSaving={botSaving}
      />

      <SettingsModal
        open={settingsOpen}
        apiKeyConfigured={apiKeyConfigured}
        apiKeyDraft={apiKeyDraft}
        error={settingsError}
        saving={settingsSaving}
        onClose={() => setSettingsOpen(false)}
        onApiKeyChange={setApiKeyDraft}
        onSave={handleSaveApiKey}
        onClear={handleClearApiKey}
      />

      <CreateBotModal
        open={createOpen}
        name={createName}
        systemPrompt={createPrompt}
        model={createModel}
        models={models}
        modelsLoading={modelsLoading}
        error={createError}
        onClose={() => setCreateOpen(false)}
        onNameChange={setCreateName}
        onSystemPromptChange={setCreatePrompt}
        onModelChange={setCreateModel}
        onCreate={handleCreateBot}
      />
    </div>
  );
}
