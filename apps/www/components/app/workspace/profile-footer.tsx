"use client";

import { authClient } from "@/lib/auth-client";
import { appRoutes } from "@/lib/app-routes";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "cn";
import { LayoutGrid, LogOut, Plug, Settings2, Sparkles } from "@/components/icons/lucide";
import Link from "next/link";
import { useRouter } from "next/navigation";

interface ProfileFooterProps {
  email: string;
  onOpenSettings?: () => void;
  compact?: boolean;
}

const footerRowClass =
  "flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left text-[13px] text-foreground transition-colors hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 aria-expanded:bg-surface-active";

/** Nav footer: work history link plus the account row that opens the account menu. */
export function ProfileFooter({ email, onOpenSettings, compact }: ProfileFooterProps) {
  const router = useRouter();

  async function handleSignOut() {
    await authClient.signOut();
    router.push("/sign-in");
    router.refresh();
  }

  const initial = email.trim().charAt(0).toUpperCase() || "U";

  return (
    <div className={cn("space-y-px", compact && "space-y-0")}>
      <Link href={appRoutes.work} className={footerRowClass}>
        <LayoutGrid className="size-4 shrink-0 text-muted-foreground" aria-hidden />
        <span className="truncate">Work history</span>
      </Link>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={<button type="button" className={footerRowClass} aria-label="Account menu" />}
        >
          <span
            className="flex size-5 shrink-0 items-center justify-center rounded-full bg-info text-[10px] font-semibold text-white"
            aria-hidden
          >
            {initial}
          </span>
          <span className="min-w-0 flex-1 truncate">{email}</span>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" side="top" sideOffset={6} className="w-56">
          <DropdownMenuLabel className="truncate">{email}</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem render={<Link href={appRoutes.computers} />}>
            <Plug className="size-4" aria-hidden />
            Computers
          </DropdownMenuItem>
          <DropdownMenuItem render={<Link href={appRoutes.skills} />}>
            <Sparkles className="size-4" aria-hidden />
            Skills
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
  );
}
