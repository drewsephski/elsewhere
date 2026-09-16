interface LandingCtaProps {
  onGetStarted: () => void;
}

export function LandingCta({ onGetStarted }: LandingCtaProps) {
  return (
    <section className="landing-shell mt-20 py-8 text-center md:mt-24 md:py-12">
      <h2 className="text-[32px] leading-9 font-medium tracking-[-0.02em] text-brand-dark md:text-[42px] md:leading-[48px]">
        Meet your first bot
      </h2>
      <p className="mx-auto mt-3 max-w-xl text-[16px] leading-7 text-muted-foreground md:text-[17px]">
        Give Elsewhere something you have been putting off and let it handle the follow-through.
      </p>
      <div className="mt-8 flex flex-col items-stretch justify-center gap-3 sm:flex-row sm:items-center">
        <button
          type="button"
          onClick={onGetStarted}
          className="landing-btn-primary inline-flex h-[48px] items-center justify-center rounded-[14px] px-[22px] text-[16px] font-medium sm:h-[44px] sm:text-[14px]"
        >
          Get started
        </button>
        <a
          href="https://github.com/drewsephski/elsewhere"
          target="_blank"
          rel="noreferrer"
          className="landing-btn-ghost inline-flex h-[48px] items-center justify-center rounded-[14px] px-[22px] text-[16px] font-medium sm:h-[44px] sm:text-[14px]"
        >
          View on GitHub
        </a>
      </div>
    </section>
  );
}
