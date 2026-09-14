"use client";

import { authClient } from "@/lib/auth-client";
import { cn } from "cn";
import {
  Bot,
  CheckCircle2,
  LayoutDashboard,
  Monitor,
  LogOut,
} from "lucide-react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";

const links = [
  { href: "/app", label: "Overview", icon: LayoutDashboard },
  { href: "/app/bots", label: "Bots", icon: Bot },
  { href: "/app/computers", label: "Computers", icon: Monitor },
  { href: "/app/approvals", label: "Approvals", icon: CheckCircle2 },
] as const;

interface AppShellNavProps {
  layout?: "sidebar" | "mobile";
}

export function AppShellNav({ layout = "sidebar" }: AppShellNavProps) {
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
      {!isMobile ? (
        <p className="mb-3 px-3 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
          Workspace
        </p>
      ) : null}

      {links.map((link) => {
        const active =
          pathname === link.href || pathname.startsWith(`${link.href}/`);
        const Icon = link.icon;

        return (
          <Link
            key={link.href}
            href={link.href}
            className={cn(
              "relative flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
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
              strokeWidth={1.75}
              aria-hidden
            />
            {link.label}
          </Link>
        );
      })}

      <button
        type="button"
        onClick={handleSignOut}
        className={cn(
          "flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted/80 hover:text-foreground",
          isMobile ? "ml-auto shrink-0" : "mt-auto",
        )}
      >
        <LogOut className="h-4 w-4 shrink-0" strokeWidth={1.75} aria-hidden />
        Sign out
      </button>
    </nav>
  );
}
