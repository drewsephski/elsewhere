import { ComputersManager } from "@/components/app/computers-manager";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function ComputersPage() {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Computers"
        description="Persistent environments for your bots. Files stay on the computer between assignments."
      />
      <ComputersManager />
    </div>
  );
}
