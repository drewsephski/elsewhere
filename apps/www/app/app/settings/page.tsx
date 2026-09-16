import { PermissionPolicyEditor } from "@/components/app/permission-policy-editor";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";

export default function SettingsPage() {
  return (
    <div className="space-y-8">
      <WorkspacePageHeader
        title="Settings"
        description="Choose what Bots may do on their own. Individual Bots can still override these defaults."
      />
      <section className="surface-card max-w-xl">
        <PermissionPolicyEditor
          endpoint="/v1/settings/permission-policies"
          mode="owner"
        />
      </section>
    </div>
  );
}
