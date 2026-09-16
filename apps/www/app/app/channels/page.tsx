import { ChannelsManager } from "@/components/app/channels-manager";

export default function ChannelsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Channels</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Talk to your Bots from the apps you already use.
        </p>
      </div>
      <ChannelsManager />
    </div>
  );
}
