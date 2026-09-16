"use client";

import { appRoutes } from "@/lib/app-routes";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ChevronRight } from "@/components/icons/lucide";
import Link from "next/link";

interface GetStartedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function GetStartedDialog({ open, onOpenChange }: GetStartedDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-w-[min(calc(100%-2rem),28rem)] gap-5 rounded-[22px] bg-white p-6 sm:p-7"
        overlayClassName="bg-black/45 backdrop-blur-[8px]"
      >
        <DialogHeader className="gap-2 pr-6">
          <p className="text-[13px] font-medium text-primary">Get started</p>
          <DialogTitle className="text-[28px] leading-8 font-medium tracking-[-0.02em]">
            How do you want to start?
          </DialogTitle>
          <DialogDescription className="text-[15px] leading-6">
            Open the cloud workspace now. The macOS desktop app is coming soon.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2.5">
          <Link
            href={appRoutes.workspace}
            className="group flex items-start justify-between gap-3 rounded-2xl border border-border bg-muted/40 px-4 py-3.5 text-left transition-colors hover:border-primary/25 hover:bg-accent"
            onClick={() => onOpenChange(false)}
          >
            <span>
              <span className="block text-[15px] font-medium text-foreground">
                Open workspace now
              </span>
              <span className="mt-1 block text-[13.5px] leading-5 text-muted-foreground">
                Cloud is live. Sign in, connect ChatGPT, create a computer.
              </span>
            </span>
            <ChevronRight className="mt-0.5 size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5" aria-hidden />
          </Link>
          <div className="flex items-start justify-between gap-3 rounded-2xl border border-border bg-muted/25 px-4 py-3.5 text-left">
            <span>
              <span className="block text-[15px] font-medium text-foreground">
                Desktop for macOS
              </span>
              <span className="mt-1 block text-[13.5px] leading-5 text-muted-foreground">
                Local computers on your Mac. Same bots, same workspace.
              </span>
            </span>
            <span className="mt-0.5 shrink-0 rounded-full border border-border bg-white px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
              Coming soon
            </span>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
