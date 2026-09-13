import { useCallback, useEffect, useRef, useState } from "react";
import type { Bot, Message, ModelDescriptor } from "@/lib/definitions";
import { useChatStreamListener } from "@/hooks/use-chat-stream";
import { applyStreamEvent } from "@/providers/stream-assembler";
import { botService } from "@/services/bot-service";
import { chatService } from "@/services/chat-service";
import { tauriApi } from "@/lib/tauri-api";
import { BotSidebar } from "@/ui/bot-sidebar";
import { BotSettingsPanel } from "@/ui/bot-settings-panel";
import { ChatComposer } from "@/ui/chat-composer";
import { CreateBotModal } from "@/ui/create-bot-modal";
import { MessageBubble } from "@/ui/message-bubble";
import { SettingsModal } from "@/ui/settings-modal";

function formatInvokeError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "An unexpected error occurred";
}

export default function App() {
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
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [createName, setCreateName] = useState("");
  const [createPrompt, setCreatePrompt] = useState("");
  const [createModel, setCreateModel] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [botSaving, setBotSaving] = useState(false);
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);
  const [loadingChat, setLoadingChat] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const userScrolledUpRef = useRef(false);

  const selectedBot = bots.find((b) => b.id === selectedBotId) ?? null;
  const displayBot = botDraft ?? selectedBot;
  const isStreaming = activeRequestId !== null;

  const refreshBots = useCallback(async () => {
    const list = await botService.list();
    setBots(list);
    if (list.length > 0 && !selectedBotId) {
      setSelectedBotId(list[0].id);
    }
  }, [selectedBotId]);

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
      if (!createModel && list[0]) {
        setCreateModel(list[0].id);
      }
    } catch (error) {
      setModelsError(formatInvokeError(error));
    } finally {
      setModelsLoading(false);
    }
  }, [apiKeyConfigured, createModel]);

  const loadChat = useCallback(async (botId: string) => {
    setLoadingChat(true);
    setGlobalError(null);
    try {
      const { conversation, messages: loaded } =
        await chatService.loadConversation(botId);
      setConversationId(conversation.id);
      setMessages(loaded);
      setBotDraft(null);
    } catch (error) {
      setGlobalError(formatInvokeError(error));
    } finally {
      setLoadingChat(false);
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

  useChatStreamListener((event) => {
    if (activeRequestId && event.requestId !== activeRequestId) {
      return;
    }
    if (event.type === "error") {
      setGlobalError(event.error ?? "Stream failed");
      setActiveRequestId(null);
      if (selectedBotId) {
        void loadChat(selectedBotId);
      }
      return;
    }
    if (event.type === "done" || event.type === "cancelled") {
      setActiveRequestId(null);
      if (selectedBotId) {
        void loadChat(selectedBotId);
      }
      return;
    }
    if (event.type === "delta") {
      setMessages((prev) =>
        prev.map((m) => {
          if (m.id !== event.assistantMessageId) {
            return m;
          }
          return {
            ...m,
            body: applyStreamEvent(m.body, event),
          };
        }),
      );
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

  async function handleArchiveBot() {
    if (!displayBot) {
      return;
    }
    try {
      await botService.archive(displayBot.id);
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
    setMessages((prev) => [...prev, optimisticUser, optimisticAssistant]);

    try {
      const result = await chatService.sendMessage({
        botId: selectedBot.id,
        providerId: selectedBot.provider,
        conversationId: conversationId ?? undefined,
        content,
      });
      setConversationId(result.conversationId);
      setActiveRequestId(result.requestId);
      setMessages((prev) => {
        const withoutTemp = prev.filter((m) => !m.id.startsWith("temp-"));
        return [
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
      });
    } catch (error) {
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

  return (
    <div className="h-full flex bg-surface-0">
      <BotSidebar
        bots={bots}
        selectedBotId={selectedBotId}
        onSelectBot={setSelectedBotId}
        onCreateBot={() => {
          setCreateOpen(true);
          setCreateError(null);
          if (models[0]) {
            setCreateModel(models[0].id);
          }
        }}
        onOpenSettings={() => {
          setSettingsOpen(true);
          setSettingsError(null);
        }}
      />

      <main className="flex-1 flex flex-col min-w-0">
        {!apiKeyConfigured && (
          <div className="bg-amber-950/40 border-b border-amber-900/50 px-4 py-2 text-sm text-amber-200/90">
            Add your OpenAI API key in Settings to chat.
            <button
              type="button"
              className="ml-2 underline"
              onClick={() => setSettingsOpen(true)}
            >
              Open Settings
            </button>
          </div>
        )}

        {displayBot && (
          <BotSettingsPanel
            bot={displayBot}
            models={models}
            modelsLoading={modelsLoading}
            modelsError={modelsError}
            onChange={(patch) => {
              setBotDraft((prev) => ({
                ...(prev ?? displayBot),
                ...patch,
              }));
            }}
            onSave={handleSaveBot}
            onArchive={handleArchiveBot}
            saving={botSaving}
          />
        )}

        {globalError && (
          <div className="px-4 py-2 text-sm text-danger bg-danger/10 border-b border-danger/30">
            {globalError}
          </div>
        )}

        <div
          ref={scrollContainerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto px-4 py-4"
        >
          {!selectedBot && (
            <div className="h-full flex items-center justify-center text-muted text-sm">
              Create or select a Bot to start.
            </div>
          )}
          {selectedBot && loadingChat && messages.length === 0 && (
            <p className="text-muted text-sm">Loading conversation…</p>
          )}
          {messages.map((message) => (
            <MessageBubble key={message.id} message={message} />
          ))}
          <div ref={messagesEndRef} />
        </div>

        <ChatComposer
          value={composer}
          disabled={!selectedBot || loadingChat}
          isStreaming={isStreaming}
          onChange={setComposer}
          onSend={handleSend}
          onStop={handleStop}
        />
      </main>

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
