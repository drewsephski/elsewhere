import { ComputersManager } from "@/components/app/computers-manager";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";
import type { ReactNode } from "react";

interface ComputersPageProps {
  headerAction?: ReactNode;
}

export default function ComputersPage({ headerAction }: ComputersPageProps) {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Computers"
        description="Persistent environments for your bots. Files stay on the computer between assignments."
        action={headerAction}
      />
      <ComputersManager />
    </div>
  );
}
