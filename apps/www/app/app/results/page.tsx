import { ResultsPanel } from "@/components/app/results-panel";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function ResultsPage() {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Results"
        description="Finished work you can take with you. Saved independently of your bots' computers."
      />
      <ResultsPanel />
    </div>
  );
}
