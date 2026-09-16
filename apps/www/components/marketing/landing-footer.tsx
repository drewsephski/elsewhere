import { siteConfig } from "@elsewhere/brand";
import { ProductLogo } from "@/components/product-logo";
import { appRoutes } from "@/lib/app-routes";
import Link from "next/link";

const footerLinks = [
  { href: "/#demo", label: "Product" },
  { href: "/#roster", label: "Bots" },
  { href: "/#selfhost", label: "Computers" },
  { href: appRoutes.workspace, label: "Workspace" },
  { href: siteConfig.links.docs, label: "GitHub", external: true },
] as const;

export function LandingFooter() {
  return (
    <footer className="mt-8 border-t border-border/80">
      <div className="landing-shell flex flex-col gap-8 py-10 md:flex-row md:items-center md:justify-between">
        <Link href="/" className="flex items-center gap-2" aria-label={`${siteConfig.productName} home`}>
          <ProductLogo size="sm" />
          <span className="text-[15px] tracking-tight text-brand-dark">
            {siteConfig.productName.toLowerCase()}
          </span>
          <span className="text-[13px] text-muted-foreground">© {new Date().getFullYear()}</span>
        </Link>
        <nav className="flex flex-wrap items-center gap-x-5 gap-y-2" aria-label="Footer">
          {footerLinks.map((link) =>
            "external" in link && link.external ? (
              <a
                key={link.href}
                href={link.href}
                target="_blank"
                rel="noreferrer"
                className="text-[14px] text-muted-foreground transition-colors hover:text-foreground"
              >
                {link.label}
              </a>
            ) : (
              <Link
                key={link.href}
                href={link.href}
                className="text-[14px] text-muted-foreground transition-colors hover:text-foreground"
              >
                {link.label}
              </Link>
            ),
          )}
          <a
            href={`mailto:${siteConfig.contactEmail}`}
            className="text-[14px] text-muted-foreground transition-colors hover:text-foreground"
          >
            Support
          </a>
        </nav>
      </div>
    </footer>
  );
}
