import { ROSTER_BOTS } from "@/lib/marketing/landing";

export function BotRoster() {
  return (
    <section id="roster" className="landing-shell mt-20 md:mt-28">
      <p className="text-[13px] font-medium text-primary">Bot templates</p>
      <h2 className="mt-2 max-w-2xl text-[32px] leading-9 font-medium tracking-[-0.02em] text-brand-dark md:text-[42px] md:leading-[48px] md:tracking-[-0.84px]">
        Give each bot a job
      </h2>
      <p className="mt-3 max-w-xl text-[16px] leading-7 text-muted-foreground md:text-[18px] md:leading-8">
        Start a bot with a role, a computer, and instructions. A few questions about the work, how you write, and where it lives. Then it gets going.
      </p>
      <div className="mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {ROSTER_BOTS.map((bot) => (
          <article key={bot.id} className="landing-card flex flex-col px-6 pt-8 pb-6">
            <div className="mb-6 flex justify-center">
              <span className="flex h-[104px] w-[104px] items-end justify-center" role="img" aria-label={bot.name}>
                <img
                  src={bot.image}
                  alt=""
                  width={280}
                  height={320}
                  decoding="async"
                  className="max-h-full w-auto max-w-full object-contain object-bottom"
                  aria-hidden
                />
              </span>
            </div>
            <h3 className="text-[15.5px] font-semibold text-brand-dark">{bot.name}</h3>
            <p className="mt-1.5 text-[14px] leading-5 text-muted-foreground">
              {bot.description}
            </p>
          </article>
        ))}
      </div>
    </section>
  );
}
