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
            "mb-3 w-full justify-start gap-2",
          )}
        >
          <MessageSquare className="size-4 shrink-0" aria-hidden />
          Back to chat
        </Link>
      ) : null}

      {showBackToChat && isMobile ? (
        <Link
          href={appRoutes.workspace}
          className="inline-flex shrink-0 items-center gap-2 rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium"
        >
          <MessageSquare className="size-4" aria-hidden />
          Chat
        </Link>
      ) : null}

      {!isMobile ? (
        <p className="mb-2 px-3 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
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
              "group relative flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
              isMobile && "shrink-0 whitespace-nowrap",
              active
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-muted/80 hover:text-foreground",
            )}
          >
            {active && !isMobile ? (
              <span
                className="absolute top-1/2 left-0 h-5 w-0.5 -translate-y-1/2 rounded-full bg-primary"
                aria-hidden
              />
            ) : null}
            <Icon
              className={cn("h-4 w-4 shrink-0", active ? "text-primary" : "")}
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
          "flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted/80 hover:text-foreground",
          isMobile ? "ml-auto shrink-0" : "mt-auto",
        )}
      >
        <LogOut className="h-4 w-4 shrink-0" aria-hidden />
        Sign out
      </button>
    </nav>
  );
}
