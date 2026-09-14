import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import type { ReactNode } from "react";

interface WorkspaceEmptyStateProps {
  title: string;
  description: string;
  icon?: ReactNode;
  children?: ReactNode;
}

export function WorkspaceEmptyState({
  title,
  description,
  icon,
  children,
}: WorkspaceEmptyStateProps) {
  return (
    <Empty className="border-border/80 bg-white/40">
      <EmptyHeader>
        {icon ? <EmptyMedia variant="icon">{icon}</EmptyMedia> : null}
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{description}</EmptyDescription>
      </EmptyHeader>
      {children ? <EmptyContent>{children}</EmptyContent> : null}
    </Empty>
  );
}
