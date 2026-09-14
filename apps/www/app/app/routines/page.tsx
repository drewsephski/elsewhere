import { RoutinesManager } from "@/components/app/routines-manager";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function RoutinesPage() {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Routines"
        description="Give recurring work a home. Your bots keep the schedule."
      />
      <RoutinesManager />
    </div>
  );
}
