"use client";

import type { GroupConversationDetail, TranscriptMessage } from "@/lib/api-types";
import type { WorkspaceBotPresence } from "@/lib/workspace-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { AssistantMessageBubble } from "@/components/app/assistant-message-bubble";
import { UserPromptBubble } from "@/components/app/user-prompt-bubble";
import { MarkdownContent } from "@/components/app/markdown-content";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ChevronLeft, FileText, Plus, X } from "@/components/icons/lucide";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { ChatComposerFrame, ComposerIconButton } from "./chat-composer";
import {
  ComposerAttachmentStrip,
  COMPOSER_FILE_ACCEPT,
  composerCanSend,
  readyAttachmentIds,
  useComposerAttachments,
} from "./composer-attachments";
import { MessageAttachmentList } from "./message-attachments";
import {
  buildGroupSendPayload,
  GroupMentionComposer,
  type MentionToken,
} from "./group-mention-composer";
import { deleteConversationMessage, canArchiveWorkRun, archiveWorkRun } from "@/lib/archive-work-run";
import { MessageDeleteButton } from "@/components/app/message-delete-button";
import { StatusPill, type StatusTone } from "@/components/app/status-pill";

const MIN_GROUP_BOTS = 2;

function formatCloudError(message: string): string {
  const stripped = message.replace(/^(validation|conflict):\s*/i, "").trim();
  if (!stripped) {
    return message;
  }
  return stripped.charAt(0).toUpperCase() + stripped.slice(1);
}

function toastCloudError(message: string) {
  toast.error(formatCloudError(message));
}

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

function recipientStatusTone(status: string): StatusTone {
  switch (status) {
    case "completed":
      return "success";
    case "running":
      return "info";
    case "queued":
      return "warning";
    case "cancelled":
      return "destructive";
    default:
      return "neutral";
  }
}

function routingStatusLabel(routing: TranscriptMessage["routing"]): string | null {
  if (!routing) {
    return null;
  }
  switch (routing.status) {
    case "pending":
    case "routing":
      return "Choosing a responder…";
    case "no_response":
      return "No Bot selected";
    case "failed":
      return "Couldn't choose a responder";
    default:
      return null;
  }
}

export function GroupConversationView({ groupId, bots }: GroupConversationViewProps) {
  const [group, setGroup] = useState<GroupConversationDetail | null>(null);
  const [messages, setMessages] = useState<TranscriptMessage[]>([]);
  const [message, setMessage] = useState("");
  const [mentions, setMentions] = useState<MentionToken[]>([]);
  const [pending, setPending] = useState(false);
  const idempotencyRef = useRef<string | null>(null);
  const composerFiles = useComposerAttachments({ conversationId: groupId });

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
        toastCloudError(err instanceof Error ? err.message : "Could not load group");
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
    const attachmentIds = readyAttachmentIds(composerFiles.files);
    if (!composerCanSend(message, composerFiles.files) || pending) {
      return;
    }
    setPending(true);
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
          routingMode: payload.routingMode,
          attachmentIds,
        }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not send message");
      }
      idempotencyRef.current = null;
      composerFiles.reset();
      await loadMessages();
    } catch (err) {
      setMessage(trimmed);
      toastCloudError(err instanceof Error ? err.message : "Could not send message");
    } finally {
      setPending(false);
    }
  }

  async function handleRemoveParticipant(botId: string) {
    if (activeParticipants.length <= MIN_GROUP_BOTS) {
      toast.error("Keep at least 2 bots in the group.");
      return;
    }
    const response = await cloudHostFetch(
      `/v1/conversations/${groupId}/participants/${botId}`,
      { method: "DELETE" },
    );
    const body = await response.json();
    if (!response.ok) {
      toastCloudError(body.error ?? "Could not remove participant");
      return;
    }
    setGroup(body);
  }

  async function handleDeleteMessage(item: TranscriptMessage) {
    try {
      if (item.authorKind === "human") {
        const routingPending =
          item.routing?.status === "pending" || item.routing?.status === "routing";
        if (routingPending) {
          await deleteConversationMessage(groupId, item.id);
          await loadMessages();
          return;
        }
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
      toastCloudError(err instanceof Error ? err.message : "Could not delete message");
    }
  }

  async function handleRetryRoute(messageId: string) {
    const response = await cloudHostFetch(
      `/v1/conversations/${groupId}/messages/${messageId}/route/retry`,
      { method: "POST" },
    );
    if (!response.ok) {
      const body = await response.json();
      toastCloudError(body.error ?? "Could not retry routing");
      return;
    }
    await loadMessages();
  }

  function canDeleteMessage(item: TranscriptMessage): boolean {
    if (item.authorKind === "human") {
      const routingPending =
        item.routing?.status === "pending" || item.routing?.status === "routing";
      if (routingPending) {
        return true;
      }
      return !item.recipients?.some(
        (recipient) => recipient.status === "queued" || recipient.status === "running",
      );
    }
    return canArchiveWorkRun(item.status);
  }

  async function handleAddParticipant(botId: string) {
    const response = await cloudHostFetch(`/v1/conversations/${groupId}/participants`, {
      method: "POST",
      body: JSON.stringify({ botId }),
    });
    const body = await response.json();
    if (!response.ok) {
      toastCloudError(body.error ?? "Could not add participant");
      return;
    }
    setGroup(body);
  }

  const addableBots = bots.filter(
    (bot) => !activeParticipants.some((participant) => participant.botId === bot.id),
  );
  const canAddParticipant = addableBots.length > 0 && activeParticipants.length < 6;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex h-11 shrink-0 items-center justify-between gap-3 border-b border-border px-3">
        <div className="flex min-w-0 items-center gap-2">
          <Link
            href="/app"
            className="flex size-7 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-surface-hover lg:hidden"
            aria-label="Back to bots"
          >
            <ChevronLeft className="size-4" />
          </Link>
          <div className="flex shrink-0 -space-x-1.5">
            {activeParticipants.slice(0, 4).map((participant) => (
              <BotCreatureAvatar
                key={participant.botId}
                name={participant.name}
                avatarId={participant.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                size="xs"
                variant="tile"
                className="ring-2 ring-background"
              />
            ))}
          </div>
          <p className="truncate text-[13px] font-medium leading-tight">{group?.name ?? "Group"}</p>
          <span className="hidden text-[11px] text-muted-foreground sm:inline">
            · {activeParticipants.length} participants
          </span>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-5">
        <div className="mx-auto flex max-w-3xl flex-col gap-5">
          {messages.map((item) =>
            item.authorKind === "human" ? (
              <div key={item.id} className="flex flex-col items-end gap-1.5">
                <UserPromptBubble
                  sentAt={item.createdAt}
                  className="items-stretch"
                  extra={<MessageAttachmentList attachments={item.attachments ?? []} />}
                >
                  {item.body}
                </UserPromptBubble>
                <div className="flex flex-wrap items-center justify-end gap-1.5">
                  {item.recipients && item.recipients.length > 0 ? (
                    <ul className="flex flex-wrap justify-end gap-1.5" aria-label="Recipients">
                      {item.recipients.map((recipient) => {
                        const label = `${recipient.botName} · ${recipientStatusLabel(recipient.status)}`;
                        return (
                          <li key={recipient.botId}>
                            {recipient.runId && recipient.status === "completed" ? (
                              <Link
                                href={`/app/work/${recipient.runId}`}
                                className="rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                              >
                                <StatusPill tone={recipientStatusTone(recipient.status)}>
                                  {label}
                                </StatusPill>
                              </Link>
                            ) : (
                              <StatusPill
                                tone={recipientStatusTone(recipient.status)}
                                live={recipient.status === "running" || recipient.status === "queued"}
                              >
                                {label}
                              </StatusPill>
                            )}
                          </li>
                        );
                      })}
                    </ul>
                  ) : routingStatusLabel(item.routing) ? (
                    <div className="flex flex-wrap items-center justify-end gap-2 text-[11px] text-muted-foreground">
                      <span>{routingStatusLabel(item.routing)}</span>
                      {item.routing?.status === "failed" ? (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          className="h-6 rounded-md px-2 text-[11px]"
                          onClick={() => void handleRetryRoute(item.id)}
                        >
                          Retry
                        </Button>
                      ) : null}
                      {item.routing?.status === "no_response" ? (
                        <span className="text-[10px]">@mention a Bot if you&apos;d like a response.</span>
                      ) : null}
                    </div>
                  ) : null}
                  {canDeleteMessage(item) ? (
                    <MessageDeleteButton
                      onDelete={() => handleDeleteMessage(item)}
                      label="Delete"
                      className="h-6 rounded-md px-1.5 text-[11px] text-muted-foreground hover:text-destructive"
                    />
                  ) : null}
                </div>
              </div>
            ) : (
              <AssistantMessageBubble
                key={item.id}
                leading={
                  <BotCreatureAvatar
                    name={item.authorBotName ?? "Bot"}
                    avatarId={item.authorAvatarId ?? DEFAULT_BOT_AVATAR_ID}
                    size="xs"
                  />
                }
                footer={
                  canDeleteMessage(item) ? (
                    <MessageDeleteButton
                      onDelete={() => handleDeleteMessage(item)}
                      label="Delete"
                      className="h-6 rounded-md px-1.5 text-[11px] text-muted-foreground hover:text-destructive"
                    />
                  ) : null
                }
              >
                <p className="mb-1 text-[11px] font-medium text-muted-foreground">
                  {item.authorBotName ?? "Bot"}
                </p>
                <MarkdownContent
                  text={item.body || (item.status === "pending" ? "Working…" : "")}
                />
              </AssistantMessageBubble>
            ),
          )}
        </div>
      </div>

      <footer className="shrink-0 px-3 pb-3 pt-1 sm:px-5">
        <div className="mx-auto max-w-3xl space-y-2">
          {activeParticipants.length > 0 ? (
            <ul className="flex flex-wrap items-center gap-1.5 px-1" aria-label="Participants">
              {activeParticipants.map((participant) => (
                <li key={participant.botId}>
                  <button
                    type="button"
                    className="group inline-flex h-6 items-center gap-1 rounded-full bg-surface-hover pl-2 pr-1.5 text-[11px] text-muted-foreground transition-colors hover:bg-surface-active hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                    onClick={() => void handleRemoveParticipant(participant.botId)}
                    aria-label={`Remove ${participant.name} from group`}
                  >
                    {participant.name}
                    <X className="size-3 opacity-60 group-hover:opacity-100" aria-hidden />
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
          <ChatComposerFrame
            onSubmit={(event) => {
              event.preventDefault();
              void handleSubmit();
            }}
            canSend={composerCanSend(message, composerFiles.files) && !pending}
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
                  render={<ComposerIconButton label="Add" />}
                >
                  <Plus className="size-4" aria-hidden />
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" side="top" sideOffset={8} className="w-52">
                  <DropdownMenuItem
                    disabled={pending || composerFiles.files.length >= 4}
                    onClick={() => composerFiles.inputRef.current?.click()}
                  >
                    <FileText className="size-4" aria-hidden />
                    Add files
                  </DropdownMenuItem>
                  <DropdownMenuLabel>Add to group</DropdownMenuLabel>
                  {addableBots.map((bot) => (
                    <DropdownMenuItem
                      key={bot.id}
                      onClick={() => void handleAddParticipant(bot.id)}
                    >
                      <BotCreatureAvatar
                        name={bot.name}
                        avatarId={bot.avatarId ?? DEFAULT_BOT_AVATAR_ID}
                        size="xs"
                      />
                      <span className="truncate">{bot.name}</span>
                    </DropdownMenuItem>
                  ))}
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
          </ChatComposerFrame>
        </div>
      </footer>
    </div>
  );
}
