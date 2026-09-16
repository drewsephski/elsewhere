"use client";

import { ArrowUp } from "@/components/icons/lucide";
import { cn } from "cn";
import {
  forwardRef,
  type ButtonHTMLAttributes,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";

interface ChatComposerFrameProps {
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  /** Leading control (e.g. "+" menu). */
  leading?: ReactNode;
  /** The input itself; defaults are provided by `ChatComposerTextarea`. */
  children: ReactNode;
  canSend: boolean;
  pending?: boolean;
  sendLabel?: string;
  className?: string;
}

/**
 * Full-width pill composer: generous padding, leading "+" slot, trailing round
 * send control. Wrap any textarea (plain or mention-aware) in it.
 */
export function ChatComposerFrame({
  onSubmit,
  leading,
  children,
  canSend,
  pending = false,
  sendLabel = "Send message",
  className,
}: ChatComposerFrameProps) {
  return (
    <form
      onSubmit={onSubmit}
      className={cn(
        "flex w-full items-end gap-1.5 rounded-[1.375rem] border border-border bg-card px-2 py-1.5 transition-colors focus-within:border-ring/40",
        className,
      )}
    >
      {leading ? <div className="flex shrink-0 items-center self-end pb-0.5">{leading}</div> : null}
      <div className="flex min-w-0 flex-1 items-center">{children}</div>
      <button
        type="submit"
        disabled={!canSend || pending}
        data-ready={canSend && !pending}
        className={cn(
          "flex size-8 shrink-0 items-center justify-center self-end rounded-full text-foreground transition-colors",
          "bg-surface-active hover:bg-secondary disabled:opacity-45 disabled:hover:bg-surface-active",
          "data-[ready=true]:bg-primary data-[ready=true]:text-primary-foreground data-[ready=true]:hover:bg-primary/90",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
        )}
        aria-label={pending ? "Sending…" : sendLabel}
      >
        <ArrowUp className={cn("size-4", pending && "animate-pulse")} aria-hidden />
      </button>
    </form>
  );
}

type ChatComposerTextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement> & {
  /** Submit on Enter (Shift+Enter inserts a newline). Defaults to true. */
  submitOnEnter?: boolean;
};

export const ChatComposerTextarea = forwardRef<HTMLTextAreaElement, ChatComposerTextareaProps>(
  function ChatComposerTextarea(
    { className, submitOnEnter = true, onKeyDown, rows = 1, ...props },
    ref,
  ) {
    function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
      onKeyDown?.(event);
      if (event.defaultPrevented) {
        return;
      }
      if (submitOnEnter && event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        event.currentTarget.form?.requestSubmit();
      }
    }

    return (
      <textarea
        ref={ref}
        rows={rows}
        onKeyDown={handleKeyDown}
        className={cn(
          "max-h-40 min-h-8 w-full resize-none bg-transparent px-1.5 py-1.5 text-[13px] leading-5 text-foreground outline-none placeholder:text-muted-foreground disabled:opacity-60",
          className,
        )}
        {...props}
      />
    );
  },
);

type ComposerIconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  /** Optional when used as a `render` element (children are injected by the primitive). */
  children?: ReactNode;
};

/** Quiet round icon button for composer/header chrome. */
export const ComposerIconButton = forwardRef<HTMLButtonElement, ComposerIconButtonProps>(
  function ComposerIconButton({ label, children, className, type = "button", ...props }, ref) {
    return (
      <button
        ref={ref}
        type={type}
        aria-label={label}
        title={label}
        className={cn(
          "flex size-8 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground disabled:opacity-40 disabled:hover:bg-transparent",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60 aria-expanded:bg-surface-active aria-expanded:text-foreground",
          className,
        )}
        {...props}
      >
        {children}
      </button>
    );
  },
);
