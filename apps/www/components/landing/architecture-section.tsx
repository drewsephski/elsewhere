const architectureLayers = [
  {
    name: "Clients",
    detail: "macOS app today; web and mobile clients on the same APIs.",
    width: "w-full",
  },
  {
    name: "Control plane",
    detail: "Auth, bot config, billing, and policy — API-first.",
    width: "w-[92%]",
  },
  {
    name: "Agent sandboxes",
    detail: "One VM per bot with terminal, files, and browser tooling.",
    width: "w-[84%]",
  },
  {
    name: "Durable memory",
    detail: "Messages, files, and embeddings with strict tenant isolation.",
    width: "w-[76%]",
  },
] as const;

export function ArchitectureSection() {
  return (
    <section id="architecture" className="scroll-mt-20">
      <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6">
        <div className="grid gap-12 lg:grid-cols-2 lg:items-start">
          <div>
            <p className="font-mono text-[0.7rem] uppercase tracking-[0.2em] text-primary">
              Architecture
            </p>
            <h2 className="mt-3 text-3xl font-semibold tracking-tight sm:text-4xl">
              One story from laptop to cloud
            </h2>
            <p className="mt-4 text-sm leading-relaxed text-muted-foreground">
              Bots keep compute and state in sync without mixing tenants or secrets. The marketing
              site, desktop app, and future cloud API all describe the same isolation model.
            </p>
            <dl className="mt-10 space-y-4 border-t border-border/60 pt-8">
              {architectureLayers.map((layer) => (
                <div key={layer.name} className="grid gap-1 sm:grid-cols-[7rem_1fr]">
                  <dt className="font-mono text-xs text-muted-foreground">{layer.name}</dt>
                  <dd className="text-sm text-foreground/90">{layer.detail}</dd>
                </div>
              ))}
            </dl>
          </div>
          <div
            className="relative rounded-2xl border border-border/70 bg-card/30 p-6 sm:p-8"
            aria-label="Layered architecture diagram"
            role="img"
          >
            <div className="absolute inset-0 topo-lines rounded-2xl opacity-40" aria-hidden />
            <div className="relative flex flex-col items-center gap-3 py-4">
              {architectureLayers.map((layer, index) => (
                <div
                  key={layer.name}
                  className={`${layer.width} rounded-lg border border-border/80 bg-background/80 px-4 py-3 shadow-sm transition-transform hover:-translate-y-0.5`}
                  style={{ zIndex: architectureLayers.length - index }}
                >
                  <p className="font-mono text-[0.65rem] uppercase tracking-wider text-primary">
                    {layer.name}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">{layer.detail}</p>
                </div>
              ))}
            </div>
            <p className="relative mt-6 text-center font-mono text-[0.65rem] text-muted-foreground">
              tenant boundary at every layer
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
