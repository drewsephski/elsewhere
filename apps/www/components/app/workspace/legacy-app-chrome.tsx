"use client";

import { AppShellNav } from "@/components/app/app-shell-nav";
import { ProductLogo } from "@/components/product-logo";
import Link from "next/link";
import { siteConfig } from "@elsewhere/brand";
import { DesktopTitlebar } from "@/components/app/desktop-titlebar";

interface LegacyAppChromeProps {
  children: React.ReactNode;
  userEmail: string;
}

/** Management pages (work, approvals, results…) share the workspace window shell. */
export function LegacyAppChrome({ children, userEmail }: LegacyAppChromeProps) {
  return (
    <div className="workspace-window flex h-[100dvh] flex-col overflow-hidden text-foreground">
      <DesktopTitlebar />
      <div className="workspace-window-frame flex min-h-0 flex-1 flex-col overflow-hidden">
        <header className="shrink-0 border-b border-border">
          <div className="flex h-11 items-center justify-between gap-4 px-3 tauri-traffic-safe-l lg:px-4">
            <Link
              href="/app"
              className="flex items-center gap-2 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              <ProductLogo size="md" className="opacity-90" />
              <span className="text-[13px] font-medium tracking-tight">
                {siteConfig.productName}
              </span>
            </Link>
            <span className="truncate text-[11px] text-muted-foreground">{userEmail}</span>
          </div>
        </header>

        <div className="flex min-h-0 flex-1">
          <aside
            className="hidden w-[min(100%,15rem)] shrink-0 border-r border-border bg-surface px-2 py-3 md:flex md:flex-col"
            aria-label="Manage navigation"
          >
            <AppShellNav layout="sidebar" showBackToChat />
          </aside>
          <main className="min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto bg-background">
            <div className="mb-3 px-4 pt-3 md:hidden">
              <AppShellNav layout="mobile" showBackToChat />
            </div>
            <div className="mx-auto w-full min-w-0 max-w-6xl px-4 py-4 sm:px-6 sm:py-5">
              {children}
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
