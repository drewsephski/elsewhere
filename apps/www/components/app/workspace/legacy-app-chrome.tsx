"use client";

import { AppShellNav } from "@/components/app/app-shell-nav";
import { ProductLogo } from "@/components/product-logo";
import Link from "next/link";
import { siteConfig } from "@elsewhere/brand";

interface LegacyAppChromeProps {
  children: React.ReactNode;
  userEmail: string;
}

export function LegacyAppChrome({ children, userEmail }: LegacyAppChromeProps) {
  return (
    <div className="app-shell-bg flex min-h-[100dvh] flex-col overflow-hidden text-foreground">
      <header className="shrink-0 border-b border-border/70 bg-white/55 backdrop-blur-md">
        <div className="flex h-12 items-center justify-between gap-4 px-4 lg:px-5">
          <Link href="/app" className="flex items-center gap-2.5">
            <ProductLogo size="md" />
            <span className="text-sm font-semibold tracking-tight">
              {siteConfig.productName}
            </span>
          </Link>
          <span className="truncate text-xs text-muted-foreground">{userEmail}</span>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <aside
          className="hidden w-[min(100%,16rem)] shrink-0 border-r border-border/70 bg-white/55 backdrop-blur-md px-3 py-4 md:flex md:flex-col"
          aria-label="Manage navigation"
        >
          <AppShellNav layout="sidebar" showBackToChat />
        </aside>
        <main className="min-h-0 min-w-0 flex-1 overflow-y-auto">
          <div className="mb-3 px-4 pt-3 md:hidden">
            <AppShellNav layout="mobile" showBackToChat />
          </div>
          <div className="mx-auto w-full max-w-6xl px-4 py-6 sm:px-6 sm:py-8">{children}</div>
        </main>
      </div>
    </div>
  );
}
