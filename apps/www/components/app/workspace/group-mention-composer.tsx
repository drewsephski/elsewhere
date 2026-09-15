"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import type { GroupParticipantSummary } from "@/lib/api-types";
import { Button } from "@/components/ui/button";
import { cn } from "cn";
import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

export interface MentionToken {
  botId: string;
  label: string;
  start: number;
  end: number;
}

interface GroupMentionComposerProps {
  participants: GroupParticipantSummary[];
  value: string;
  onChange: (value: string, mentions: MentionToken[]) => void;
  onSubmit: () => void;
  disabled?: boolean;
  pending?: boolean;
}

const EVERYONE_ID = "__everyone__";

export function GroupMentionComposer({
  participants,
  value,
  onChange,
  onSubmit,
  disabled,
  pending,
}: GroupMentionComposerProps) {
  const listboxId = useId();
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [mentionStart, setMentionStart] = useState<number | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const activeParticipants = useMemo(
    () => participants.filter((p) => !p.leftAt),
    [participants],
  );

  const options = useMemo(() => {
    const query =
      mentionStart === null
        ? ""
        : value.slice(mentionStart + 1).split(/\s/)[0]?.toLowerCase() ?? "";
    const base = [
      {
        id: EVERYONE_ID,
        name: "everyone",
        avatarId: DEFAULT_BOT_AVATAR_ID,
        subtitle: "All active participants",
      },
      ...activeParticipants.map((p) => ({
        id: p.botId,
        name: p.name,
        avatarId: p.avatarId ?? DEFAULT_BOT_AVATAR_ID,
        subtitle: "Participant",
      })),
    ];
    if (!query) {
      return base;
    }
    return base.filter((item) => item.name.toLowerCase().includes(query));
  }, [activeParticipants, mentionStart, value]);

  const parseMentions = useCallback(
    (text: string): MentionToken[] => {
      const mentions: MentionToken[] = [];
      for (const participant of activeParticipants) {
        const needle = `@${participant.name}`;
        let from = 0;
        while (true) {
          const idx = text.indexOf(needle, from);
          if (idx === -1) {
            break;
          }
          mentions.push({
            botId: participant.botId,
            label: participant.name,
            start: idx,
            end: idx + needle.length,
          });
          from = idx + needle.length;
        }
      }
      return mentions.sort((a, b) => a.start - b.start);
    },
    [activeParticipants],
  );

  const handleChange = useCallback(
    (next: string) => {
      onChange(next, parseMentions(next));
      const cursor = textareaRef.current?.selectionStart ?? next.length;
      const before = next.slice(0, cursor);
      const at = before.lastIndexOf("@");
      if (at >= 0 && !before.slice(at + 1).includes(" ")) {
        setMentionStart(at);
        setOpen(true);
        setActiveIndex(0);
      } else {
        setOpen(false);
        setMentionStart(null);
      }
    },
    [onChange, parseMentions],
  );

  const applySelection = useCallback(
    (optionId: string, optionName: string) => {
      if (mentionStart === null) {
        return;
      }
      const cursor = textareaRef.current?.selectionStart ?? value.length;
      const before = value.slice(0, mentionStart);
      const after = value.slice(cursor);
      const token = `@${optionName}`;
      const next = `${before}${token} ${after}`;
      const mentions = parseMentions(next);
      if (optionId === EVERYONE_ID) {
        onChange(next, mentions);
      } else {
        onChange(next, mentions);
      }
      setOpen(false);
      setMentionStart(null);
      requestAnimationFrame(() => {
        const el = textareaRef.current;
        if (!el) {
          return;
        }
        const pos = before.length + token.length + 1;
        el.focus();
        el.setSelectionRange(pos, pos);
      });
    },
    [mentionStart, onChange, parseMentions, value],
  );

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (!open || options.length === 0) {
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        onSubmit();
      }
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((i) => (i + 1) % options.length);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((i) => (i - 1 + options.length) % options.length);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      setOpen(false);
      return;
    }
    if (event.key === "Enter" || event.key === "Tab") {
      event.preventDefault();
      const option = options[activeIndex];
      if (option) {
        applySelection(option.id, option.name);
      }
      return;
    }
  };

  useEffect(() => {
    if (activeIndex >= options.length) {
      setActiveIndex(0);
    }
  }, [activeIndex, options.length]);

  const hasEveryone = value.toLowerCase().includes("@everyone");

  return (
    <div className="relative flex flex-1 flex-col gap-1">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(event) => handleChange(event.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled || pending}
        rows={1}
        placeholder="Message the group"
        className="max-h-32 min-h-[2.25rem] w-full resize-none rounded-full border border-border/80 bg-[#f5f3f8] px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-primary/20"
        aria-label="Group message"
        aria-autocomplete="list"
        aria-controls={open ? listboxId : undefined}
        aria-expanded={open}
      />
      <p className="px-2 text-[11px] text-muted-foreground">
        Write normally, or @mention a Bot to direct your message.
      </p>
      {open && options.length > 0 ? (
        <ul
          id={listboxId}
          role="listbox"
          className="absolute bottom-full left-0 z-20 mb-1 max-h-48 w-full overflow-y-auto rounded-xl border border-border bg-white py-1 shadow-lg"
        >
          {options.map((option, index) => (
            <li key={option.id} role="presentation">
              <button
                type="button"
                role="option"
                aria-selected={index === activeIndex}
                className={cn(
                  "flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-muted",
                  index === activeIndex && "bg-muted",
                )}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => applySelection(option.id, option.name)}
              >
                <BotCreatureAvatar
                  name={option.name}
                  avatarId={option.avatarId}
                  size="sm"
                />
                <span className="min-w-0 flex-1 truncate">
                  @{option.name}
                  <span className="block text-[11px] text-muted-foreground">
                    {option.subtitle}
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      <input type="hidden" value={hasEveryone ? "everyone" : "specific"} readOnly />
      <Button
        type="button"
        className="absolute right-0 top-0 hidden"
        onClick={onSubmit}
        disabled={pending || !value.trim()}
      >
        Send
      </Button>
    </div>
  );
}

export function buildGroupSendPayload(
  body: string,
  mentions: MentionToken[],
  participants: GroupParticipantSummary[],
) {
  const activeIds = new Set(
    participants.filter((p) => !p.leftAt).map((p) => p.botId),
  );
  const everyone = /\beveryone\b/i.test(body) && body.includes("@everyone");
  const recipientBotIds = everyone
    ? undefined
    : [...new Set(mentions.map((m) => m.botId).filter((id) => activeIds.has(id)))];
  const hasExplicitMentions = everyone || (recipientBotIds?.length ?? 0) > 0;
  return {
    body,
    routingMode: everyone
      ? ("everyone" as const)
      : hasExplicitMentions
        ? ("specific" as const)
        : ("auto" as const),
    mentionMode: everyone ? ("everyone" as const) : hasExplicitMentions ? ("specific" as const) : undefined,
    recipientBotIds: hasExplicitMentions && !everyone ? recipientBotIds : undefined,
  };
}
