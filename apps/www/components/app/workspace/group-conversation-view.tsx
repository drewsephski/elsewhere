"use client";

import type { GroupConversationDetail, TranscriptMessage } from "@/lib/api-types";
import type { WorkspaceBotPresence } from "@/lib/workspace-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { MarkdownContent } from "@/components/app/markdown-content";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { Button } from "@/components/ui/button";
import { ChevronLeft } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  buildGroupSendPayload,
  GroupMentionComposer,
  type MentionToken,
} from "./group-mention-composer";
import { deleteConversationMessage, canArchiveWorkRun, archiveWorkRun } from "@/lib/archive-work-run";
import { MessageDeleteButton } from "@/components/app/message-delete-button";

interface GroupConversationViewProps {
  groupId: string;
  bots: WorkspaceBotPresence[];
}

function recipientStatusLabel(status: string): string {
  switch (status) {
    case "queued":
      return "queued";
    case "running":
      return "working";
    case "completed":
      return "finished";
    case "cancelled":
      return "cancelled";
    default:
      return status;
  }
}

export function GroupConversationView({ groupId, bots }: GroupConversationViewProps) {
  const [group, setGroup] = useState<GroupConversationDetail | null>(null);
  const [messages, setMessages] = useState<TranscriptMessage[]>([]);
  const [message, setMessage] = useState("");
  const [mentions, setMentions] = useState<MentionToken[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const idempotencyRef = useRef<string | null>(null);

  const activeParticipants = useMemo(
    () => group?.participants.filter((p) => !p.leftAt) ?? [],
    [group],
  );

  const loadGroup = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/conversations/${groupId}`);
    if (!response.ok) {
      throw new Error("Group not found");
    }
    setGroup(await response.json());
  }, [groupId]);

  const loadMessages = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/conversations/${groupId}/messages`);
    if (!response.ok) {
      throw new Error("Could not load transcript");
    }
    setMessages(await response.json());
  }, [groupId]);

  useEffect(() => {
    void (async () => {
      try {
        await loadGroup();
        await loadMessages();
      } catch (err) {
        setError(err instanceof Error ? err.message : "Could not load group");
      }
    })();
  }, [loadGroup, loadMessages]);

  useEffect(() => {
    const timer = setInterval(() => {
      void loadMessages().catch(() => undefined);
    }, 2500);
    return () => clearInterval(timer);
  }, [loadMessages]);

  async function handleSubmit() {
    if (!message.trim() || pending) {
      return;
    }
    setPending(true);
    setError(null);
    const trimmed = message.trim();
    const payload = buildGroupSendPayload(trimmed, mentions, group?.participants ?? []);
    const idempotencyKey = idempotencyRef.current ?? crypto.randomUUID();
    idempotencyRef.current = idempotencyKey;
    setMessage("");
    setMentions([]);
    try {
      const response = await cloudHostFetch(`/v1/conversations/${groupId}/messages`, {
        method: "POST",
        headers: { "Idempotency-Key": idempotencyKey },
        body: JSON.stringify({
          body: trimmed,
          recipientBotIds: payload.recipientBotIds,
          mentionMode: payload.mentionMode,
        }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not send message");
      }
      idempotencyRef.current = null;
      await loadMessages();
    } catch (err) {
      setMessage(trimmed);
      setError(err instanceof Error ? err.message : "Could not send message");
    } finally {
      setPending(false);
    }
  }

  async function handleRemoveParticipant(botId: string) {
    setError(null);
    const response = await cloudHostFetch(
      `/v1/conversations/${groupId}/participants/${botId}`,
      { method: "DELETE" },
    );
    const body = await response.json();
    if (!response.ok) {
      setError(body.error ?? "Could not remove participant");
      return;
    }
    setGroup(body);
  }

  async function handleDeleteMessage(item: TranscriptMessage) {
    setError(null);
    try {
      if (item.authorKind === "human") {
        const activeRecipient = item.recipients?.some(
          (recipient) => recipient.status === "queued" || recipient.status === "running",
        );
        if (activeRecipient) {
          throw new Error("Wait for bots to finish before deleting this message");
        }
        await deleteConversationMessage(groupId, item.id);
      } else if (item.runId && canArchiveWorkRun(item.status)) {
        await archiveWorkRun(item.runId);
      } else {
        await deleteConversationMessage(groupId, item.id);
      }
      await loadMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delete message");
    }
  }

  function canDeleteMessage(item: TranscriptMessage): boolean {
    if (item.authorKind === "human") {
      return !item.recipients?.some(
        (recipient) => recipient.status === "queued" || recipient.status === "running",
      );
    }
    return canArchiveWorkRun(item.status);
  }

  async function handleAddParticipant(botId: string) {
    setError(null);
    const response = await cloudHostFetch(`/v1/conversations/${groupId}/participants`, {
      method: "POST",
      body: JSON.stringify({ botId }),
    });
    const body = await response.json();
    if (!response.ok) {
      setError(body.error ?? "Could not add participant");
      return;
    }
    setGroup(body);
  }

  const addableBots = bots.filter(
    (bot) => !activeParticipants.some((participant) => participant.botId === bot.id),
  );

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
          <div className="flex -space-x-2">
            {activeParticipants.slice(0, 4).map((participant) => (
              <BotCreatureAvatar
                key={participant.botId}
                name={participant.name}
                avatarId={participant.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                size="sm"
                className="ring-2 ring-white"
              />
            ))}
          </div>
          <div className="min-w-0">
            <p className="truncate font-semibold">{group?.name ?? "Group"}</p>
            <p className="text-xs text-muted-foreground">
              {activeParticipants.length} participants
            </p>
          </div>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
        <div className="mx-auto flex max-w-2xl flex-col gap-4">
          {messages.map((item) =>
            item.authorKind === "human" ? (
              <div key={item.id} className="flex flex-col items-end gap-1">
                <UserPromptBubble sentAt={item.createdAt}>{item.body}</UserPromptBubble>
                {canDeleteMessage(item) ? (
                  <MessageDeleteButton
                    onDelete={() => handleDeleteMessage(item)}
                    label="Delete message"
                    className="h-7 px-2 text-[11px] text-muted-foreground hover:text-destructive"
                  />
                ) : null}
                {item.recipients && item.recipients.length > 0 ? (
                  <ul className="flex flex-wrap justify-end gap-1.5 text-[11px] text-muted-foreground">
                    {item.recipients.map((recipient) => (
                      <li key={recipient.botId}>
                        {recipient.runId && recipient.status === "completed" ? (
                          <Link
                            href={`/app/work/${recipient.runId}`}
                            className="rounded-full border border-border/80 bg-white px-2 py-0.5 hover:bg-muted"
                          >
                            {recipient.botName} — {recipientStatusLabel(recipient.status)}
                          </Link>
                        ) : (
                          <span className="rounded-full border border-border/80 bg-white px-2 py-0.5">
                            {recipient.botName} — {recipientStatusLabel(recipient.status)}
                          </span>
                        )}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>
            ) : (
              <div key={item.id} className="flex flex-col items-start gap-1">
                <div className="flex justify-start">
                <div className="max-w-[90%] rounded-3xl rounded-bl-md border border-border/80 bg-white px-4 py-3 text-sm shadow-sm">
                  <div className="mb-2 flex items-center gap-2">
                    <BotCreatureAvatar
                      name={item.authorBotName ?? "Bot"}
                      avatarId={item.authorAvatarId ?? DEFAULT_BOT_AVATAR_ID}
                      size="sm"
                    />
                    <p className="text-xs font-medium text-muted-foreground">
                      {item.authorBotName ?? "Bot"}
                    </p>
                  </div>
                  <MarkdownContent text={item.body || (item.status === "pending" ? "Working…" : "")} />
                </div>
                </div>
                {canDeleteMessage(item) ? (
                  <MessageDeleteButton
                    onDelete={() => handleDeleteMessage(item)}
                    label="Delete message"
                    className="h-7 px-2 text-[11px] text-muted-foreground hover:text-destructive"
                  />
                ) : null}
              </div>
            ),
          )}
        </div>
      </div>

      <footer className="shrink-0 space-y-3 border-t border-border/70 bg-white/90 px-4 py-3 backdrop-blur-md">
        <div className="mx-auto flex max-w-2xl flex-wrap items-center gap-2">
          {activeParticipants.map((participant) => (
            <button
              key={participant.botId}
              type="button"
              className="rounded-full border border-border px-2 py-1 text-xs hover:bg-muted"
              onClick={() => void handleRemoveParticipant(participant.botId)}
              aria-label={`Remove ${participant.name} from group`}
            >
              {participant.name} ×
            </button>
          ))}
          {addableBots.length > 0 && activeParticipants.length < 6 ? (
            <select
              className="rounded-full border border-border px-2 py-1 text-xs"
              defaultValue=""
              onChange={(event) => {
                const botId = event.target.value;
                if (botId) {
                  void handleAddParticipant(botId);
                  event.target.value = "";
                }
              }}
              aria-label="Add participant"
            >
              <option value="">Add bot…</option>
              {addableBots.map((bot) => (
                <option key={bot.id} value={bot.id}>{bot.name}</option>
              ))}
            </select>
          ) : null}
        </div>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void handleSubmit();
          }}
          className="mx-auto flex max-w-2xl items-end gap-2"
        >
          <GroupMentionComposer
            participants={group?.participants ?? []}
            value={message}
            onChange={(next, nextMentions) => {
              setMessage(next);
              setMentions(nextMentions);
            }}
            onSubmit={() => void handleSubmit()}
            disabled={pending}
            pending={pending}
          />
          <Button type="submit" disabled={pending || !message.trim()}>
            {pending ? "Sending…" : "Send"}
          </Button>
        </form>
        {error ? (
          <p className="mx-auto max-w-2xl text-xs text-red-700" role="alert">{error}</p>
        ) : null}
      </footer>
    </div>
  );
}
