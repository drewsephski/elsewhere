import { ApprovalsDataGrid } from "@/components/app/approvals-data-grid";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function ApprovalsPage() {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Pending approvals"
        description="Mutating computer tools stay blocked until you approve or deny."
      />
      <ApprovalsDataGrid />
    </div>
  );
}
