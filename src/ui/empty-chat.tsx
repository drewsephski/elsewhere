import { Button } from "@/components/ui/button";
import { Bot, KeyRound, Plus } from "@/components/icons/lucide";

interface EmptyChatProps {
  apiKeyConfigured: boolean;
  onCreateBot: () => void;
  onOpenSettings: () => void;
}

export function EmptyChat({
  apiKeyConfigured,
  onCreateBot,
  onOpenSettings,
}: EmptyChatProps) {
  return (
    <div className="flex h-full min-h-[280px] flex-col items-center justify-center px-6 text-center">
      <div
        className="mb-4 flex size-14 items-center justify-center rounded-2xl bg-primary/10 text-primary ring-1 ring-primary/20"
        aria-hidden
      >
        <Bot className="size-7" />
      </div>
      <h2 className="text-lg font-semibold tracking-tight">Start a conversation</h2>
      <p className="mt-2 max-w-sm text-sm text-muted-foreground">
        {apiKeyConfigured
          ? "Create a bot or pick one from the sidebar. Each bot keeps its own instructions and chat history."
          : "Add your OpenAI API key first, then create a bot to chat locally on your Mac."}
      </p>
      <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
        {!apiKeyConfigured && (
          <Button type="button" variant="outline" onClick={onOpenSettings} className="gap-2">
            <KeyRound className="size-4" />
            Add API key
          </Button>
        )}
        <Button type="button" onClick={onCreateBot} className="gap-2">
          <Plus className="size-4" />
          New bot
        </Button>
      </div>
    </div>
  );
}
