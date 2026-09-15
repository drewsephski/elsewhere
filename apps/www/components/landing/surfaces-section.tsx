import { productSurfaces } from "@elsewhere/brand";
import { appRoutes } from "@/lib/app-routes";
import Link from "next/link";

const surfaceHrefs: Record<string, string> = {
  Bots: appRoutes.workspace,
  Computers: appRoutes.computers,
  Approvals: appRoutes.approvals,
  Routines: appRoutes.routines,
  Connectors: appRoutes.connectors,
};

export function SurfacesSection() {
  return (
    <section aria-label="Product surfaces" className="border-b border-border/60">
      <div className="mx-auto max-w-6xl px-4 py-12 sm:px-6">
        <p className="font-mono text-[0.65rem] uppercase tracking-[0.2em] text-muted-foreground">
          Surfaces in the workspace
        </p>
        <ul className="mt-6 flex flex-wrap gap-2">
          {productSurfaces.map((surface) => {
            const href = surfaceHrefs[surface.label] ?? appRoutes.workspace;
            return (
              <li key={surface.label}>
                <Link
                  href={href}
                  className="group inline-flex flex-col rounded-xl border border-border/70 bg-card/50 px-4 py-3 transition-colors hover:border-primary/35 hover:bg-card"
                >
                  <span className="text-sm font-medium text-foreground">{surface.label}</span>
                  <span className="text-xs text-muted-foreground group-hover:text-foreground/80">
                    {surface.detail}
                  </span>
                </Link>
              </li>
            );
          })}
        </ul>
      </div>
    </section>
  );
}
