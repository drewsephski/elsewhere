import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@desktop/components/ui/tooltip";
import { cn } from "@desktop/lib/utils";
import { PanelLeftOpen } from "@desktop/components/icons/lucide";

interface SidebarExpandTabProps {
  onExpand: () => void;
}

export function SidebarExpandTab({ onExpand }: SidebarExpandTabProps) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            onClick={onExpand}
            className={cn(
              "absolute top-[42%] right-0 z-30 flex h-7 w-6 -translate-y-1/2 translate-x-[55%] items-center justify-center",
              "rounded-md border border-white/50 bg-white/55 shadow-sm backdrop-blur-md",
              "text-muted-foreground transition-colors hover:bg-white/75 hover:text-foreground",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 focus-visible:ring-offset-1",
            )}
            aria-label="Open assistants sidebar"
          />
        }
      >
        <PanelLeftOpen className="size-3.5" strokeWidth={2.25} aria-hidden />
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={8}>
        Open sidebar
      </TooltipContent>
    </Tooltip>
  );
}
