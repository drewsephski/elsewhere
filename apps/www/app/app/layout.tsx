import { AppShellNav } from "@/components/app/app-shell-nav";
import { auth } from "@/lib/auth";
import { headers } from "next/headers";
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
    <div className="min-h-screen bg-brand-cream text-brand-dark">
      <header className="border-b border-brand-dark/10 bg-white/80 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-6">
          <span className="text-sm font-medium tracking-wide">Elsewhere</span>
          <span className="text-xs text-brand-dark/60">{session.user.email}</span>
        </div>
      </header>
      <div className="mx-auto max-w-6xl px-6 py-8">
        <AppShellNav />
        {children}
      </div>
    </div>
  );
}
