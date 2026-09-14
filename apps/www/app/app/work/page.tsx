import { RecentRunsPanel } from "@/components/app/recent-runs-panel";
export default function WorkPage() {
  return <div className="space-y-6"><div><h1 className="text-2xl font-semibold">Work</h1><p className="mt-2 text-sm text-muted-foreground">Assignments, progress, and finished results. Your work stays here when you leave.</p></div><RecentRunsPanel limit={100} /></div>;
}
