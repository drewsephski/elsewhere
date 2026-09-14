"use client";

import { authClient } from "@/lib/auth-client";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";

const links = [
  { href: "/app", label: "Overview" },
  { href: "/app/bots", label: "Bots" },
  { href: "/app/computers", label: "Computers" },
  { href: "/app/approvals", label: "Approvals" },
] as const;

export function AppShellNav() {
  const pathname = usePathname();
  const router = useRouter();

  async function handleSignOut() {
    await authClient.signOut();
    router.push("/sign-in");
    router.refresh();
  }

  return (
    <nav className="mb-8 flex flex-wrap items-center gap-4 border-b border-brand-dark/10 pb-4">
      {links.map((link) => {
        const active = pathname === link.href || pathname.startsWith(`${link.href}/`);
        return (
          <Link
            key={link.href}
            href={link.href}
            className={`text-sm uppercase tracking-wide ${
              active ? "text-brand-dark" : "text-brand-dark/50 hover:text-brand-dark/80"
            }`}
          >
            {link.label}
          </Link>
        );
      })}
      <button
        type="button"
        onClick={handleSignOut}
        className="ml-auto text-sm text-brand-dark/60 underline-offset-4 hover:underline"
      >
        Sign out
      </button>
    </nav>
  );
}
