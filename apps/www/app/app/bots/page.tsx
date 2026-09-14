import { BotsManager } from "@/components/app/bots-manager";

export default function BotsPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold tracking-tight">Bots</h1>
      <BotsManager />
    </div>
  );
}
