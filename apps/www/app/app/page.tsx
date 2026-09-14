import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { RecentRunsPanel } from "@/components/app/recent-runs-panel";

export default function AppDashboardPage() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">Your workspace</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          A place for your bots to work, and for you to see what they’ve done.
        </p>
      </div>
      <div className="grid gap-6 lg:grid-cols-2">
        <ProviderStatusCard />
        <RecentRunsPanel />
      </div>
    </div>
  );
}
