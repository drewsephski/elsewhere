"use client";

import { Shimmer } from "@/components/ai-elements/shimmer";
import { cn } from "cn";

export function ChatThinkingLine({
  label,
  className,
}: {
  label: string;
  className?: string;
}) {
  return (
    <div role="status" aria-live="polite" className={cn("min-w-0", className)}>
      <Shimmer as="p" duration={1.8} className="text-[13px]">
        {label}
      </Shimmer>
    </div>
  );
}
