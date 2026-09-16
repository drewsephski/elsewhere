import { ConnectorsManager } from "@/components/app/connectors-manager";

export default function ConnectorsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Integrations</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Apps and tools your Bots can use.
        </p>
      </div>
      <ConnectorsManager />
    </div>
  );
}
