import { ConnectorsManager } from "@/components/app/connectors-manager";

export default function ConnectorsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Connectors</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Link external services so Bots can use structured read APIs instead of browser automation.
        </p>
      </div>
      <ConnectorsManager />
    </div>
  );
}
