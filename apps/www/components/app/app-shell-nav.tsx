"use client";

import { authClient } from "@/lib/auth-client";
import { appRoutes, appShellNavLinks } from "@/lib/app-routes";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "cn";
import {
  BriefcaseBusiness,
  CalendarClock,
  CheckCircle2,
  Files,
  LogOut,
  MessageSquare,
  Monitor,
  Plug,
} from "@/components/icons/lucide";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";

const linkIcons = {
  [appRoutes.work]: BriefcaseBusiness,
  [appRoutes.approvals]: CheckCircle2,
  [appRoutes.results]: Files,
  [appRoutes.routines]: CalendarClock,
  [appRoutes.computers]: Monitor,
  [appRoutes.connectors]: Plug,
} as const;

interface AppShellNavProps {
  layout?: "sidebar" | "mobile";
  showBackToChat?: boolean;
}

export function AppShellNav({
  layout = "sidebar",
  showBackToChat = false,
}: AppShellNavProps) {
  const pathname = usePathname();
  const router = useRouter();
  const isMobile = layout === "mobile";

  async function handleSignOut() {
    await authClient.signOut();
    router.push("/sign-in");
    router.refresh();
  }

  return (
    <nav
      className={cn(
        isMobile
          ? "flex gap-1 overflow-x-auto pb-1"
          : "flex h-full flex-col gap-1",
      )}
      aria-label="App navigation"
    >
      {showBackToChat && !isMobile ? (
        <Link
          href={appRoutes.workspace}
          className={cn(
            buttonVariants({ variant: "outline" }),
            "mb-3 w-full justify-start gap-2 rounded-lg text-[13px]",
          )}
        >
          <MessageSquare className="size-4 shrink-0" aria-hidden />
          Back to chat
        </Link>
      ) : null}

      {showBackToChat && isMobile ? (
        <Link
          href={appRoutes.workspace}
          className="inline-flex shrink-0 items-center gap-2 rounded-lg border border-border bg-card px-3 py-2 text-[13px] font-medium"
        >
          <MessageSquare className="size-4" aria-hidden />
          Chat
        </Link>
      ) : null}

      {!isMobile ? (
        <p className="mb-1.5 px-2 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground/80">
          Manage
        </p>
      ) : null}

      {appShellNavLinks.map((link) => {
        const active =
          pathname === link.href || pathname.startsWith(`${link.href}/`);
        const Icon = linkIcons[link.href];

        return (
          <Link
            key={link.href}
            href={link.href}
            className={cn(
              "group relative flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-[13px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
              isMobile && "shrink-0 whitespace-nowrap px-3 py-2",
              active
                ? "bg-surface-active text-foreground"
                : "text-muted-foreground hover:bg-surface-hover hover:text-foreground",
            )}
            aria-current={active ? "page" : undefined}
          >
            <Icon
              className={cn("h-4 w-4 shrink-0", active ? "text-foreground" : "")}
              aria-hidden
            />
            {link.label}
          </Link>
        );
      })}

      <button
        type="button"
        onClick={() => void handleSignOut()}
        className={cn(
          "flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-[13px] font-medium text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
          isMobile ? "ml-auto shrink-0 px-3 py-2" : "mt-auto",
        )}
      >
        <LogOut className="h-4 w-4 shrink-0" aria-hidden />
        Sign out
      </button>
    </nav>
  );
}
