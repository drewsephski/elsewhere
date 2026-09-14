import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@desktop/components/ui/tooltip";
import { cn } from "@desktop/lib/utils";

interface SidebarEdgeHandleProps {
  resizing: boolean;
  onPointerDown: (event: React.PointerEvent<HTMLButtonElement>) => void;
}

export function SidebarEdgeHandle({
  resizing,
  onPointerDown,
}: SidebarEdgeHandleProps) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            onPointerDown={onPointerDown}
            className={cn(
              "absolute top-1/2 right-0 z-30 flex h-5 w-2 -translate-y-1/2 translate-x-1/2 cursor-col-resize touch-none items-center justify-center rounded-full border border-border/70 bg-white shadow-sm select-none hover:bg-muted/20",
              "after:absolute after:-inset-2 after:content-['']",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-offset-1 focus-visible:ring-offset-white",
              resizing && "transition-none",
            )}
            aria-label="Drag to resize sidebar"
          />
        }
      >
        <span
          className="flex flex-col items-center gap-0.5 py-0.5"
          aria-hidden
        >
          <span className="h-1.5 w-px rounded-full bg-muted-foreground/40" />
          <span className="size-0.5 rounded-full bg-muted-foreground/55" />
          <span className="size-0.5 rounded-full bg-muted-foreground/55" />
          <span className="size-0.5 rounded-full bg-muted-foreground/55" />
          <span className="h-1.5 w-px rounded-full bg-muted-foreground/40" />
        </span>
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={10}>
        Drag to resize
      </TooltipContent>
    </Tooltip>
  );
}
