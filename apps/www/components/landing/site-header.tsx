import Link from "next/link";
import { siteConfig } from "@gptbot/brand";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "cn";

const navItems = [
  { href: "#platform", label: "Platform" },
  { href: "#roadmap", label: "Roadmap" },
  { href: "#architecture", label: "Architecture" },
] as const;

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-50 border-b border-border/50 bg-background/70 backdrop-blur-xl supports-[backdrop-filter]:bg-background/55">
      <div className="mx-auto flex h-[3.25rem] max-w-6xl items-center justify-between gap-4 px-4 sm:px-6">
        <Link
          href="/"
          className="group flex items-center gap-2.5 text-foreground"
          aria-label={`${siteConfig.productName} home`}
        >
          <span
            className="relative flex size-8 items-center justify-center overflow-hidden rounded-md border border-border/80 bg-card font-mono text-xs font-medium text-foreground shadow-[inset_0_1px_0_oklch(1_0_0/6%)]"
            aria-hidden
          >
            <span className="text-primary">▸</span>
          </span>
          <span className="text-sm font-medium tracking-tight">{siteConfig.productName}</span>
        </Link>
        <nav
          className="hidden items-center gap-1 rounded-full border border-border/60 bg-muted/30 p-0.5 text-sm md:flex"
          aria-label="Primary"
        >
          {navItems.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className="rounded-full px-3.5 py-1.5 text-muted-foreground transition-colors hover:bg-background/80 hover:text-foreground"
            >
              {item.label}
            </a>
          ))}
        </nav>
        <div className="flex items-center gap-1.5 sm:gap-2">
          <a
            href={siteConfig.links.docs}
            target="_blank"
            rel="noreferrer"
            className={cn(buttonVariants({ variant: "ghost", size: "sm" }), "hidden sm:inline-flex")}
          >
            Source
          </a>
          <a
            href={siteConfig.links.download}
            className={cn(buttonVariants({ size: "sm", variant: "outline" }))}
          >
            Download
          </a>
          <a href="#early-access" className={cn(buttonVariants({ size: "sm" }))}>
            Waitlist
          </a>
        </div>
      </div>
    </header>
  );
}
