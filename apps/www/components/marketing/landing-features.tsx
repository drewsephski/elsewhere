"use client";

import { CalendarClock, CheckCircle2, Monitor } from "@/components/icons/lucide";

const featureCards = [
  {
    title: "Agent computers",
    description:
      "Each bot gets a dedicated Linux environment for terminal, files, and browser work — in the cloud now, on your Mac soon.",
    Icon: Monitor,
  },
  {
    title: "Always-on routines",
    description:
      "Schedule skills and background jobs on infrastructure built for long-running agents, not one-shot chat sessions.",
    Icon: CalendarClock,
  },
  {
    title: "Approvals you control",
    description:
      "See what a bot may do alone and what it must ask about. Every action lands in an audit trail you own.",
    Icon: CheckCircle2,
  },
] as const;

export function LandingFeatures() {
  return (
    <section id="selfhost" className="landing-shell mt-20 md:mt-28">
      <p className="text-[13px] font-medium text-primary">Self-hosted</p>
      <h2 className="mt-2 max-w-2xl text-[32px] leading-9 font-medium tracking-[-0.02em] text-brand-dark md:text-[42px] md:leading-[48px] md:tracking-[-0.84px]">
        The computer is yours
      </h2>
      <p className="mt-3 max-w-xl text-[16px] leading-7 text-muted-foreground md:text-[18px] md:leading-8">
        Run Elsewhere in the cloud today. A Mac app is coming for local computers. Your ChatGPT subscription, your bots, your data.
      </p>
      <div className="mt-10 grid gap-4 md:grid-cols-3">
        {featureCards.map((feature) => (
          <article key={feature.title} className="landing-card p-7">
            <feature.Icon className="mb-4 size-5 text-primary" aria-hidden />
            <h3 className="text-[20px] leading-7 font-medium tracking-[-0.4px] text-brand-dark">
              {feature.title}
            </h3>
            <p className="mt-2 text-[15px] leading-6 text-muted-foreground">
              {feature.description}
            </p>
          </article>
        ))}
      </div>
    </section>
  );
}
