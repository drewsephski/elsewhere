import { AppShellNav } from "@/components/app/app-shell-nav";
import { auth } from "@/lib/auth";
import { siteConfig } from "@elsewhere/brand";
import { headers } from "next/headers";
import Link from "next/link";
import { redirect } from "next/navigation";

export default async function AppLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) {
    redirect("/sign-in");
  }

  return (
    <div className="app-shell-bg min-h-screen text-foreground">
      <header className="sticky top-0 z-30 border-b border-border/70 bg-white/90 backdrop-blur-md">
        <div className="flex h-12 items-center justify-between gap-4 px-4 lg:px-6">
          <Link href="/app" className="flex items-center gap-2.5">
            <span
              className="flex h-7 w-7 items-center justify-center rounded-full bg-primary text-xs font-semibold text-primary-foreground"
              aria-hidden
            >
              E
            </span>
            <span className="text-sm font-semibold tracking-tight">
              {siteConfig.productName}
            </span>
          </Link>

          <nav
            className="hidden items-center gap-6 text-sm text-muted-foreground md:flex"
            aria-label="Top navigation"
          >
            <Link href="/app" className="transition-colors hover:text-foreground">
              Dashboard
            </Link>
            <Link href="/" className="transition-colors hover:text-foreground">
              Home
            </Link>
          </nav>

          <span className="truncate text-xs text-muted-foreground">
            {session.user.email}
          </span>
        </div>
      </header>

      <div className="flex min-h-[calc(100vh-3rem)]">
        <aside
          className="hidden w-56 shrink-0 border-r border-border/70 bg-white/55 p-4 md:flex md:flex-col"
          aria-label="Sidebar"
        >
          <AppShellNav layout="sidebar" />
        </aside>

        <main className="min-w-0 flex-1 p-4 lg:p-6">
          <div className="mb-4 md:hidden">
            <AppShellNav layout="mobile" />
          </div>
          <div className="surface-panel min-h-[calc(100vh-7rem)] p-5 sm:p-6 md:min-h-[calc(100vh-6rem)]">
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
