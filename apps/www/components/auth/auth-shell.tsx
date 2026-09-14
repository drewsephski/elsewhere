import { ProductLogo } from "@/components/product-logo";
import { siteConfig } from "@elsewhere/brand";
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
        <div className="flex items-center gap-2.5">
          <ProductLogo size="md" />
          <span className="text-sm font-semibold tracking-tight text-foreground">
            {siteConfig.productName}
          </span>
        </div>
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
