import { howItWorksSteps } from "@elsewhere/brand";

export function HowItWorksSection() {
  return (
    <section id="how-it-works" className="scroll-mt-20 border-y border-border/60 bg-muted/10">
      <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6 sm:py-28">
        <p className="font-mono text-[0.7rem] uppercase tracking-[0.2em] text-primary">How it works</p>
        <h2 className="mt-3 max-w-2xl text-3xl font-semibold tracking-tight sm:text-4xl">
          From worker definition to finished artifact
        </h2>
        <p className="mt-4 max-w-2xl text-sm leading-relaxed text-muted-foreground">
          Elsewhere is built for coordinated autonomous agents with real computers: not a chat window
          with plugins bolted on.
        </p>
        <ol className="mt-14 grid gap-6 md:grid-cols-3">
          {howItWorksSteps.map((item) => (
            <li
              key={item.step}
              className="relative rounded-2xl border border-border/70 bg-background/60 p-6 shadow-sm"
            >
              <span className="font-mono text-xs font-medium text-primary">{item.step}</span>
              <h3 className="mt-3 text-lg font-medium tracking-tight">{item.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">{item.detail}</p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}
