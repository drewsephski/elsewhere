import { siteConfig } from "@elsewhere/brand";
import Link from "next/link";
import { appRoutes } from "@/lib/app-routes";

const trustItems = [
  {
    label: "Open source",
    detail: "Inspect the control plane and client on GitHub.",
    href: siteConfig.links.docs,
    external: true,
  },
  {
    label: "macOS desktop",
    detail: "Native shell around the same workspace UI.",
    href: siteConfig.links.download,
    external: true,
  },
  {
    label: "Cloud workspace",
    detail: "Sign in, create computers, delegate from the browser.",
    href: appRoutes.signIn,
    external: false,
  },
] as const;

export function TrustStrip() {
  return (
    <section aria-label="Trust and availability" className="border-b border-border/60 bg-muted/20">
      <div className="mx-auto grid max-w-6xl gap-6 px-4 py-10 sm:grid-cols-3 sm:px-6">
        {trustItems.map((item) => {
          const linkClass =
            "block rounded-xl border border-border/60 bg-background/50 p-4 transition-colors hover:border-primary/30 hover:bg-background";
          if (item.external) {
            return (
              <a
                key={item.label}
                href={item.href}
                target="_blank"
                rel="noreferrer"
                className={linkClass}
              >
                <p className="text-sm font-medium">{item.label}</p>
                <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{item.detail}</p>
              </a>
            );
          }
          return (
            <Link key={item.label} href={item.href} className={linkClass}>
              <p className="text-sm font-medium">{item.label}</p>
              <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{item.detail}</p>
            </Link>
          );
        })}
      </div>
    </section>
  );
}
