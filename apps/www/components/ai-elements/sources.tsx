"use client";

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ChevronDown, ExternalLink, FileText } from "@/components/icons/lucide";
import { cn } from "cn";
import type { ComponentProps } from "react";

export type SourcesProps = ComponentProps<typeof Collapsible>;

export function Sources({ className, ...props }: SourcesProps) {
  return (
    <Collapsible
      className={cn("not-prose text-xs text-muted-foreground", className)}
      {...props}
    />
  );
}

export type SourcesTriggerProps = ComponentProps<typeof CollapsibleTrigger> & {
  count: number;
};

export function SourcesTrigger({
  className,
  count,
  children,
  ...props
}: SourcesTriggerProps) {
  return (
    <CollapsibleTrigger
      className={cn(
        "flex items-center gap-1.5 rounded-md py-0.5 text-left text-[12px] text-muted-foreground transition-colors hover:text-foreground",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        className,
      )}
      {...props}
    >
      {children ?? (
        <>
          <FileText className="size-3.5" aria-hidden />
          Used {count} {count === 1 ? "source" : "sources"}
          <ChevronDown className="size-3.5" aria-hidden />
        </>
      )}
    </CollapsibleTrigger>
  );
}

export type SourcesContentProps = ComponentProps<typeof CollapsibleContent>;

export function SourcesContent({ className, ...props }: SourcesContentProps) {
  return (
    <CollapsibleContent
      className={cn("mt-2 flex w-fit flex-col gap-1.5", className)}
      {...props}
    />
  );
}

export type SourceProps = ComponentProps<"a">;

export function Source({ href, title, children, className, ...props }: SourceProps) {
  return (
    <a
      className={cn(
        "flex max-w-full items-center gap-1.5 rounded-md text-[12px] text-foreground/80 transition-colors hover:text-foreground",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        className,
      )}
      href={href}
      rel="noreferrer"
      target="_blank"
      {...props}
    >
      {children ?? (
        <>
          <ExternalLink className="size-3.5 shrink-0" aria-hidden />
          <span className="truncate">{title}</span>
        </>
      )}
    </a>
  );
}
