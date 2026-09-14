"use client";

import { authClient } from "@/lib/auth-client";
import { appRoutes } from "@/lib/app-routes";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "cn";
import { LogOut, MoreHorizontal, Plug, Settings2 } from "@/components/icons/lucide";
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
      <div className="flex items-center gap-2 rounded-xl px-1 py-1">
        <span
          className="flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-semibold text-foreground sm:size-9 sm:text-sm"
          aria-hidden
        >
          {initial}
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">Account</p>
          <p className="truncate text-xs text-muted-foreground">{email}</p>
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="size-8 shrink-0 px-0"
                aria-label="Account menu"
              />
            }
          >
            <MoreHorizontal className="size-4" aria-hidden />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-48">
            <DropdownMenuItem render={<Link href={appRoutes.computers} />}>
              <Plug className="size-4" aria-hidden />
              Computers
            </DropdownMenuItem>
            {onOpenSettings ? (
              <DropdownMenuItem onClick={onOpenSettings}>
                <Settings2 className="size-4" aria-hidden />
                ChatGPT connection
              </DropdownMenuItem>
            ) : null}
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive" onClick={() => void handleSignOut()}>
              <LogOut className="size-4" aria-hidden />
              Sign out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
