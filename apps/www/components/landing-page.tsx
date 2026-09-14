import { siteConfig } from "@elsewhere/brand";
import { ArchitectureSection } from "@/components/landing/architecture-section";
import { CtaSection } from "@/components/landing/cta-section";
import { FeatureBento } from "@/components/landing/feature-bento";
import { RoadmapSection } from "@/components/landing/roadmap-section";
import { SandboxPreview } from "@/components/landing/sandbox-preview";
import { SiteFooter } from "@/components/landing/site-footer";
import { SiteHeader } from "@/components/landing/site-header";
import { buttonVariants } from "@/components/ui/button";
import { appRoutes } from "@/lib/app-routes";
import { cn } from "cn";
import Link from "next/link";

export function LandingPage() {
  return (
    <div className="min-h-screen">
      <SiteHeader />
      <main>
        <section className="relative overflow-hidden border-b border-border/60">
          <div className="topo-lines pointer-events-none absolute inset-0 opacity-[0.35]" aria-hidden />
          <div className="relative mx-auto grid max-w-6xl gap-12 px-4 py-16 sm:px-6 sm:py-24 lg:grid-cols-2 lg:items-center lg:gap-16 lg:py-28">
            <div>
              <p className="font-mono text-[0.7rem] uppercase tracking-[0.22em] text-muted-foreground">
                Agent infrastructure
              </p>
              <h1 className="mt-4 text-4xl font-semibold tracking-[-0.03em] sm:text-5xl sm:leading-[1.08] lg:text-[3.25rem]">
                {siteConfig.tagline}
              </h1>
              <p className="mt-5 max-w-lg text-base leading-relaxed text-muted-foreground sm:text-lg">
                {siteConfig.cloudPitch}
              </p>
              <div className="mt-9 flex flex-col gap-3 sm:flex-row sm:items-center">
                <Link href={appRoutes.workspace} className={cn(buttonVariants({ size: "lg" }))}>
                  Open workspace
                </Link>
                <a
                  href={siteConfig.links.download}
                  className={cn(buttonVariants({ size: "lg", variant: "outline" }))}
                >
                  Download for macOS
                </a>
              </div>
              <p className="mt-6 font-mono text-[0.7rem] text-muted-foreground">
                Desktop ships now · Managed sandboxes in development
              </p>
            </div>
            <SandboxPreview />
          </div>
        </section>

        <FeatureBento />
        <div className="py-20 sm:py-28">
          <RoadmapSection />
        </div>
        <ArchitectureSection />
        <CtaSection />
      </main>
      <SiteFooter />
    </div>
  );
}
