import { marketingFeatures } from "@elsewhere/brand";
import { marketingFeatureAppHrefs, appRoutes } from "@/lib/app-routes";
import { cn } from "cn";
import Link from "next/link";

const bentoLayout = [
  "sm:col-span-2 sm:row-span-2 lg:col-span-2 lg:row-span-2",
  "lg:col-start-3 lg:row-start-1",
  "lg:col-start-3 lg:row-start-2",
  "sm:col-span-2 lg:col-span-3 lg:row-start-3",
] as const;

const featureMarks = ["vm", "job", "sync", "audit"] as const;

export function FeatureBento() {
  return (
    <section id="platform" className="scroll-mt-20 py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="flex flex-col gap-6 border-b border-border/60 pb-10 md:flex-row md:items-end md:justify-between">
          <div className="max-w-xl">
            <p className="font-mono text-[0.7rem] uppercase tracking-[0.2em] text-primary">
              Platform
            </p>
            <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
              Built for workers with computers, not chat sessions
            </h2>
          </div>
          <p className="max-w-md text-sm leading-relaxed text-muted-foreground md:text-right">
            Isolated compute, durable state, and streaming UX. Same architecture on your Mac today
            and on the managed sandbox path.
          </p>
        </div>
        <ul className="mt-10 grid gap-3 sm:grid-cols-2 lg:grid-cols-3 lg:grid-rows-3">
          {marketingFeatures.map((feature, index) => {
            const href = marketingFeatureAppHrefs[index] ?? appRoutes.workspace;
            return (
            <li
              key={feature.title}
              className={cn(
                "group relative overflow-hidden rounded-2xl border border-border/70 bg-card/40 transition-colors hover:border-primary/30 hover:bg-card/70",
                index < bentoLayout.length ? bentoLayout[index] : "",
              )}
            >
              <Link href={href} className="absolute inset-0 z-10 rounded-2xl" aria-label={feature.title} />
              <div
                className="pointer-events-none absolute -right-8 -top-8 size-32 rounded-full bg-primary/5 blur-2xl transition-opacity group-hover:opacity-100 opacity-0"
                aria-hidden
              />
              <span
                className="inline-flex size-9 items-center justify-center rounded-lg border border-border/80 bg-background/60 font-mono text-[0.6rem] uppercase tracking-wider text-primary"
                aria-hidden
              >
                {featureMarks[index] ?? "·"}
              </span>
              <h3 className="mt-4 text-lg font-medium tracking-tight">{feature.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">{feature.description}</p>
            </li>
          );
          })}
        </ul>
      </div>
    </section>
  );
}
