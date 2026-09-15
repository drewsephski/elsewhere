import { SkillsManager } from "@/components/app/skills-manager";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function SkillsPage() {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Skills"
        description="Reusable Agent Skill packages for your bots — versioned, attachable, and snapshotted on every run."
      />
      <SkillsManager />
    </div>
  );
}
