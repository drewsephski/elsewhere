const stats = [
  { value: "Cloud", label: "Workspace live" },
  { value: "Soon", label: "macOS desktop app" },
  { value: "ChatGPT", label: "Subscription, not API keys" },
  { value: "Yours", label: "Bots, computers, history" },
] as const;

export function StatsBand() {
  return (
    <section className="mt-20 border-y border-border/60 md:mt-28">
      <div className="landing-shell grid grid-cols-2 gap-8 py-12 md:grid-cols-4 md:gap-0 md:py-16">
        {stats.map((stat, index) => (
          <div
            key={stat.label}
            className={
              index === stats.length - 1
                ? "text-center md:px-4"
                : "text-center md:border-r md:border-border/60 md:px-4"
            }
          >
            <p className="text-[30px] font-medium tracking-tight text-brand-dark md:text-[40px] md:tracking-[-0.84px]">
              {stat.value}
            </p>
            <p className="mt-1.5 text-[13px] tracking-wide text-muted-foreground md:text-[14px]">
              {stat.label}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}
