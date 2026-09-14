import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { RecentRunsPanel } from "@/components/app/recent-runs-panel";

export default function AppDashboardPage() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">Control plane</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Signed-in Elsewhere account · resources scoped to your user id · cloud-host JWT auth.
        </p>
      </div>
      <div className="grid gap-6 lg:grid-cols-2">
        <ProviderStatusCard />
        <RecentRunsPanel />
      </div>
    </div>
  );
}
