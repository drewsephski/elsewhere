"use client";

import { Button } from "@/components/ui/button";
import { cn } from "cn";
import type { ComponentProps } from "react";

export type SuggestionsProps = ComponentProps<"div">;

export function Suggestions({ className, children, ...props }: SuggestionsProps) {
  return (
    <div
      className={cn("flex flex-wrap gap-1.5", className)}
      {...props}
    >
      {children}
    </div>
  );
}

export type SuggestionProps = Omit<ComponentProps<typeof Button>, "onClick"> & {
  suggestion: string;
  onClick?: (suggestion: string) => void;
};

export function Suggestion({
  suggestion,
  onClick,
  className,
  variant = "outline",
  size = "sm",
  children,
  ...props
}: SuggestionProps) {
  function handleClick() {
    onClick?.(suggestion);
  }

  return (
    <Button
      className={cn(
        "h-7 max-w-full cursor-pointer rounded-full border-border bg-card px-3 text-[12px] font-normal text-muted-foreground hover:bg-surface-hover hover:text-foreground",
        className,
      )}
      onClick={handleClick}
      size={size}
      type="button"
      variant={variant}
      {...props}
    >
      {children || suggestion}
    </Button>
  );
}
