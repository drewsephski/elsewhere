const stats = [
  { value: "Cloud", label: "Workspace live" },
  { value: "Soon", label: "macOS desktop app" },
  { value: "ChatGPT", label: "Subscription, not API keys" },
  { value: "Yours", label: "Bots, computers, history" },
] as const;

export function StatsBand() {
  return (
    <section className="mt-20 border-y border-border md:mt-28">
      <div className="landing-shell grid grid-cols-2 gap-8 py-12 md:grid-cols-4 md:gap-6 md:py-16">
        {stats.map((stat) => (
          <div key={stat.label} className="text-center md:border-l md:border-border md:first:border-l-0 md:pl-6 md:first:pl-0">
            <p className="text-[28px] font-semibold tracking-tight text-brand-dark md:text-[40px] md:tracking-[-0.8px]">
              {stat.value}
            </p>
            <p className="mt-1.5 text-[14px] font-medium text-muted-foreground">{stat.label}</p>
          </div>
        ))}
      </div>
    </section>
  );
}
