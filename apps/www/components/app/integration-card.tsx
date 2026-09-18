import { StatusPill, type StatusTone } from "@/components/app/status-pill";
import { cn } from "cn";
import type { ReactNode } from "react";

export interface IntegrationCardProps {
  name: string;
  description: string;
  logo: ReactNode;
  statusLabel: string;
  statusTone: StatusTone;
  meta?: ReactNode;
  actions?: ReactNode;
  className?: string;
}

export function IntegrationCard({
  name,
  description,
  logo,
  statusLabel,
  statusTone,
  meta,
  actions,
  className,
}: IntegrationCardProps) {
  return (
    <article
      className={cn(
        "flex h-full flex-col rounded-xl bg-card p-3.5 ring-1 ring-foreground/10 transition-colors hover:bg-muted/20 hover:ring-foreground/16",
        className,
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-background ring-1 ring-foreground/10">
          {logo}
        </div>
        <StatusPill tone={statusTone}>{statusLabel}</StatusPill>
      </div>
      <h3 className="mt-3 text-sm font-medium tracking-tight">{name}</h3>
      <p className="mt-1 text-[13px] leading-5 text-muted-foreground">{description}</p>
      {meta ? <div className="mt-2 text-[13px] text-foreground/90">{meta}</div> : null}
      {actions ? (
        <div className="mt-auto flex flex-wrap items-center gap-2 pt-3.5">{actions}</div>
      ) : null}
    </article>
  );
}
