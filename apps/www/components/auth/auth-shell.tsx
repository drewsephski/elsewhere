import Link from "next/link";
import type { ReactNode } from "react";

interface AuthShellProps {
  title: string;
  description: string;
  children: ReactNode;
}

export function AuthShell({ title, description, children }: AuthShellProps) {
  return (
    <main className="app-shell-bg flex min-h-screen flex-col items-center justify-center px-6">
      <div className="surface-panel w-full max-w-md p-8">
        <p className="text-xs font-medium uppercase tracking-[0.2em] text-primary">Elsewhere</p>
        <h1 className="mt-2 text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
        <p className="mt-2 text-sm text-muted-foreground">{description}</p>
        {children}
        <Link
          href="/"
          className="mt-6 block text-center text-xs text-muted-foreground hover:text-foreground"
        >
          Back to home
        </Link>
      </div>
    </main>
  );
}
