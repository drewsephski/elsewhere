"use client";

import { BrowserPreviewView } from "./browser-preview-view";
import { cn } from "cn";
import type { ReactNode } from "react";

interface ComputerBrowserPreviewProps {
  className?: string;
  /** Text shown directly beneath the screen card (e.g. "Personal’s screen"). */
  caption?: ReactNode;
}

/** Embedded live browser preview in the Computer rail (requires BrowserPreviewProvider). */
export function ComputerBrowserPreview({ className, caption }: ComputerBrowserPreviewProps) {
  return <BrowserPreviewView variant="embedded" className={cn(className)} caption={caption} />;
}
