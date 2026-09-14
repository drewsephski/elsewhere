import { cloudRoadmapPhases } from "@gptbot/brand";
import { cn } from "cn";

export function RoadmapSection() {
  return (
    <section id="roadmap" className="scroll-mt-20 border-y border-border/60 bg-muted/15">
      <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6">
        <p className="font-mono text-[0.7rem] uppercase tracking-[0.2em] text-primary">Roadmap</p>
        <h2 className="mt-3 max-w-2xl text-3xl font-semibold tracking-tight sm:text-4xl">
          Local proof, then cloud scale
        </h2>
        <p className="mt-4 max-w-2xl text-sm leading-relaxed text-muted-foreground">
          We ship the desktop app first so guest protocol, VM lifecycle, and agent runtime are
          proven before we operate them as a managed platform.
        </p>
        <ol className="mt-14 grid gap-8 md:grid-cols-3 md:gap-6">
          {cloudRoadmapPhases.map((phase, index) => (
            <li key={phase.label} className="relative">
              {index < cloudRoadmapPhases.length - 1 ? (
                <span
                  className="absolute left-[calc(50%+3rem)] top-5 hidden h-px w-[calc(100%-6rem)] bg-border md:block"
                  aria-hidden
                />
              ) : null}
              <div
                className={cn(
                  "relative rounded-2xl border border-border/70 bg-background/50 p-6",
                  index === 0 && "md:border-primary/40 md:shadow-[0_0_0_1px_oklch(0.62_0.19_265/0.15)]",
                )}
              >
                <span className="inline-block rounded-md bg-primary/15 px-2 py-0.5 font-mono text-[0.65rem] font-medium uppercase tracking-wider text-primary">
                  {phase.label}
                </span>
                <h3 className="mt-4 text-lg font-medium">{phase.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">{phase.detail}</p>
              </div>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
