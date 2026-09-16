const stats = [
  { value: "Cloud", label: "Workspace live" },
  { value: "Soon", label: "macOS desktop app" },
  { value: "ChatGPT", label: "Subscription, not API keys" },
  { value: "Yours", label: "Bots, computers, history" },
] as const;

export function StatsBand() {
  return (
    <section className="mt-20 border-y border-border/80 bg-white/50 md:mt-28">
      <div className="landing-shell grid grid-cols-2 gap-8 py-12 md:grid-cols-4 md:py-16">
        {stats.map((stat) => (
          <div key={stat.label} className="text-center">
            <p className="text-[28px] font-medium tracking-tight text-brand-dark md:text-[38px] md:tracking-[-0.76px]">
              {stat.value}
            </p>
            <p className="mt-1 text-[14px] text-muted-foreground">{stat.label}</p>
          </div>
        ))}
      </div>
    </section>
  );
}
