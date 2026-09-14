import { BotsManager } from "@/components/app/bots-manager";

export default function BotsPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-medium">Bots</h1>
      <BotsManager />
    </div>
  );
}
