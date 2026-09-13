import type { Bot } from "@/lib/definitions";
import { BotNav } from "@/ui/bot-nav";
import { cn } from "@/lib/utils";

interface BotSidebarProps {
  bots: Bot[];
  selectedBotId: string | null;
  onSelectBot: (id: string) => void;
  onCreateBot: () => void;
  onOpenSettings: () => void;
  className?: string;
}

export function BotSidebar(props: BotSidebarProps) {
  return (
    <aside
      className={cn(
        "hidden h-full w-[min(100%,18rem)] shrink-0 border-r border-border/70 md:flex md:w-72 xl:w-80",
        props.className,
      )}
    >
      <BotNav {...props} className="w-full" />
    </aside>
  );
}
