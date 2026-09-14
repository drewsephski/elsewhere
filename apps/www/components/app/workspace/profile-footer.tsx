"use client";

import { authClient } from "@/lib/auth-client";
import { cn } from "cn";
import { LogOut, Plug, Settings2 } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";

interface ProfileFooterProps {
  email: string;
  onOpenSettings?: () => void;
  compact?: boolean;
}

export function ProfileFooter({ email, onOpenSettings, compact }: ProfileFooterProps) {
  const router = useRouter();

  async function handleSignOut() {
    await authClient.signOut();
    router.push("/sign-in");
    router.refresh();
  }

  const initial = email.trim().charAt(0).toUpperCase() || "U";

  return (
    <div className={cn("space-y-2", compact && "space-y-1")}>
      <div className="flex items-center gap-2.5 rounded-xl px-1 py-1">
        <span
          className="flex size-9 shrink-0 items-center justify-center rounded-full bg-muted text-sm font-semibold text-foreground"
          aria-hidden
        >
          {initial}
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">Account</p>
          <p className="truncate text-xs text-muted-foreground">{email}</p>
        </div>
      </div>
      <div className="flex flex-wrap gap-1">
        <Link
          href="/app/computers"
          className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-muted-foreground hover:bg-muted/80 hover:text-foreground"
        >
          <Plug className="size-3.5" aria-hidden />
          Plugins
        </Link>
        {onOpenSettings ? (
          <button
            type="button"
            onClick={onOpenSettings}
            className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-muted-foreground hover:bg-muted/80 hover:text-foreground"
          >
            <Settings2 className="size-3.5" aria-hidden />
            Connection
          </button>
        ) : null}
        <button
          type="button"
          onClick={() => void handleSignOut()}
          className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-muted-foreground hover:bg-muted/80 hover:text-foreground"
        >
          <LogOut className="size-3.5" aria-hidden />
          Sign out
        </button>
      </div>
    </div>
  );
}
