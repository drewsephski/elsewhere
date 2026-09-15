import { siteConfig } from "@elsewhere/brand";
import { appRoutes } from "@/lib/app-routes";
import Link from "next/link";

export function SiteFooter() {
  const year = new Date().getFullYear();

  return (
    <footer className="border-t border-border/60">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 px-4 py-12 sm:flex-row sm:items-center sm:justify-between sm:px-6">
        <div>
          <p className="text-sm font-medium text-foreground">{siteConfig.productName}</p>
          <p className="mt-1 text-xs text-muted-foreground">
            AI workers with real computers · © {year}
          </p>
        </div>
        <div className="flex flex-wrap gap-x-6 gap-y-2 text-sm text-muted-foreground">
          <a href={siteConfig.links.docs} className="hover:text-foreground" target="_blank" rel="noreferrer">
            Source
          </a>
          <Link href={appRoutes.workspace} className="hover:text-foreground">
            Workspace
          </Link>
          <a href={siteConfig.links.download} className="hover:text-foreground">
            Download
          </a>
          <a href={`mailto:${siteConfig.contactEmail}`} className="hover:text-foreground">
            Contact
          </a>
        </div>
      </div>
    </footer>
  );
}
