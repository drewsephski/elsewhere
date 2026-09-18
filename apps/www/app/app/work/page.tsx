import { RecentRunsPanel } from "@/components/app/recent-runs-panel";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function WorkPage() {
  return (
    <div className="min-w-0 space-y-5">
      <WorkspacePageHeader
        title="Work"
        description="Assignments, progress, and finished results. Archive items you are done with—they stay recoverable under Archived."
      />
      <RecentRunsPanel limit={100} />
    </div>
  );
}
