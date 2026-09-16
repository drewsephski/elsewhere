import type { ReactNode } from "react";

interface WorkspacePageHeaderProps {
  title: string;
  description: string;
  action?: ReactNode;
}

export function WorkspacePageHeader({
  title,
  description,
  action,
}: WorkspacePageHeaderProps) {
  return (
    <header className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
      <div className="min-w-0 space-y-2">
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        <p className="max-w-2xl text-sm leading-relaxed text-muted-foreground">
          {description}
        </p>
      </div>
      {action ? <div className="shrink-0 sm:pt-0.5">{action}</div> : null}
    </header>
  );
}
