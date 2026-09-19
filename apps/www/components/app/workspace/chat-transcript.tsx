"use client";

import {
  Conversation,
  ConversationContent,
} from "@/components/ai-elements/conversation";
import { Message, MessageContent } from "@/components/ai-elements/message";
import { Tool, ToolContent, ToolHeader } from "@/components/ai-elements/tool";
import { formatMessageTime } from "@/lib/format";
import { cn } from "cn";
import type { ReactNode, RefObject, UIEventHandler } from "react";
import type { ToolActivity } from "@/contexts/active-run-context";

export function ChatTranscript(props: {
  children: ReactNode;
  scrollRef: RefObject<HTMLDivElement | null>;
  onScroll: UIEventHandler<HTMLDivElement>;
  centered?: boolean;
}) {
  return (
    <div
      ref={props.scrollRef}
      onScroll={props.onScroll}
      className="h-full min-h-0 overflow-y-auto"
    >
      <Conversation className="relative min-h-full overflow-visible" initial="instant" resize="smooth">
        <ConversationContent
          className={cn(
            "mx-auto flex min-h-full max-w-3xl flex-col gap-4 p-0 px-3 py-4 sm:gap-5 sm:px-5",
            props.centered ? "justify-center" : null,
          )}
        >
          {props.children}
        </ConversationContent>
      </Conversation>
    </div>
  );
}

export function RunUserMessage(props: {
  text: string;
  sentAt?: string;
  extra?: ReactNode;
}) {
  return (
    <div className="flex w-full flex-col items-stretch gap-2">
      {props.sentAt ? (
        <p className="text-center text-[11px] text-muted-foreground/80">
          {formatMessageTime(props.sentAt)}
        </p>
      ) : null}
      <Message from="user" className="max-w-full">
        <MessageContent className="max-w-[min(78%,36rem)] rounded-xl bg-chat-user px-3.5 py-2 text-[13px] leading-relaxed text-chat-user-foreground group-[.is-user]:bg-chat-user group-[.is-user]:px-3.5 group-[.is-user]:py-2 group-[.is-user]:text-chat-user-foreground">
          {props.text.trim() ? (
            <p className="whitespace-pre-wrap break-words">{props.text}</p>
          ) : null}
          {props.extra}
        </MessageContent>
      </Message>
    </div>
  );
}

export function RunAssistantMessage(props: {
  children: ReactNode;
  leading?: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <Message from="assistant" className="max-w-[min(88%,40rem)]">
      <div className="flex w-full items-start gap-2">
        {props.leading ? <div className="shrink-0 pt-0.5">{props.leading}</div> : null}
        <MessageContent className="min-w-0 flex-1 rounded-xl bg-chat-assistant px-3.5 py-2.5 text-[13px] leading-relaxed text-foreground group-[.is-assistant]:bg-chat-assistant">
          {props.children}
        </MessageContent>
      </div>
      {props.footer ? (
        <div className={props.leading ? "ml-[2.25rem]" : undefined}>{props.footer}</div>
      ) : null}
    </Message>
  );
}

export function RunToolActivity(props: { tool: ToolActivity }) {
  const state =
    props.tool.state === "running"
      ? "input-available"
      : props.tool.state === "error"
        ? "output-error"
        : "output-available";
  return (
    <div className="px-2 py-0.5">
      <Tool
        className={cn(
          "mb-0 w-fit max-w-full bg-muted/10",
          props.tool.state === "error" && "border-destructive/30",
          props.tool.state === "running" && "border-border/60",
        )}
        defaultOpen={props.tool.state === "error"}
      >
        <ToolHeader
          title={props.tool.label}
          type="dynamic-tool"
          state={state}
          toolName={props.tool.toolName}
        />
        {props.tool.technical ? (
          <ToolContent className="space-y-2 px-2 py-1.5">
            <p className="font-mono text-xs text-muted-foreground">{props.tool.technical}</p>
          </ToolContent>
        ) : null}
      </Tool>
    </div>
  );
}
