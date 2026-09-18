import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { Button } from "@desktop/components/ui/button";
import { KeyRound, Plus } from "@desktop/components/icons/lucide";

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
    <div className="mx-auto flex max-w-sm flex-col items-center px-6 py-16 text-center">
      <BotCreatureAvatar name="Assistant" avatarId={DEFAULT_BOT_AVATAR_ID} size="2xl" />
      <p className="mt-4 text-[15px] font-medium tracking-tight text-foreground">
        {apiKeyConfigured ? "What should your bot work on?" : "Connect OpenAI to start chatting"}
      </p>
      <p className="mt-1.5 max-w-xs text-[13px] leading-relaxed text-muted-foreground">
        {apiKeyConfigured
          ? "Create a bot or pick one from the sidebar. Each bot keeps its own instructions and chat history."
          : "Add your OpenAI API key first, then create a bot to chat locally on your Mac."}
      </p>
      <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
        {!apiKeyConfigured && (
          <Button type="button" variant="outline" onClick={onOpenSettings} className="gap-2 rounded-full">
            <KeyRound className="size-4" />
            Add API key
          </Button>
        )}
        <Button type="button" onClick={onCreateBot} className="gap-2 rounded-full">
          <Plus className="size-4" />
          New bot
        </Button>
      </div>
    </div>
  );
}
