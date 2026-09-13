import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { PanelRightOpen } from "@/components/icons/lucide";
import { cn } from "@/lib/utils";

interface ContextSidebarExpandTabProps {
  onExpand: () => void;
}

export function ContextSidebarExpandTab({
  onExpand,
}: ContextSidebarExpandTabProps) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            onClick={onExpand}
            className={cn(
              "absolute top-[42%] left-0 z-30 flex h-7 w-6 -translate-x-[55%] -translate-y-1/2 items-center justify-center",
              "rounded-md border border-white/50 bg-white/55 shadow-sm backdrop-blur-md",
              "text-muted-foreground transition-colors hover:bg-white/75 hover:text-foreground",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 focus-visible:ring-offset-1",
            )}
            aria-label="Open context panel"
          />
        }
      >
        <PanelRightOpen className="size-3.5" strokeWidth={2.25} aria-hidden />
      </TooltipTrigger>
      <TooltipContent side="left" sideOffset={8}>
        Open context panel
      </TooltipContent>
    </Tooltip>
  );
}
