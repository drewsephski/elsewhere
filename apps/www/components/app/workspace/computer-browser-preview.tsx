"use client";

import { BrowserPreviewView } from "./browser-preview-view";
import { cn } from "cn";

interface ComputerBrowserPreviewProps {
  className?: string;
}

/** Embedded live browser preview in the Computer rail (requires BrowserPreviewProvider). */
export function ComputerBrowserPreview({ className }: ComputerBrowserPreviewProps) {
  return <BrowserPreviewView variant="embedded" className={cn(className)} />;
}
