"use client";

import { BrowserPreviewView, type BrowserPreviewHandle } from "./browser-preview-view";
import { cn } from "cn";
import { forwardRef, type ReactNode } from "react";

interface ComputerBrowserPreviewProps {
  className?: string;
  /** Text shown directly beneath the screen card (e.g. "Personal’s screen"). */
  caption?: ReactNode;
}

/** Embedded live browser preview in the Computer rail (requires BrowserPreviewProvider). */
export const ComputerBrowserPreview = forwardRef<BrowserPreviewHandle, ComputerBrowserPreviewProps>(
  function ComputerBrowserPreview({ className, caption }, ref) {
    return (
      <BrowserPreviewView
        ref={ref}
        variant="embedded"
        className={cn(className)}
        caption={caption}
      />
    );
  },
);
