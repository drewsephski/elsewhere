import { RoutinesManager } from "@/components/app/routines-manager";
export default function RoutinesPage() {
  return <div className="space-y-6"><div><h1 className="text-2xl font-semibold">Routines</h1><p className="mt-2 text-sm text-muted-foreground">Give recurring work a home. Your bots keep the schedule.</p></div><RoutinesManager /></div>;
}
