"use client";

import { siteConfig } from "@elsewhere/brand";
import { AGENT_SETUP_PROMPT } from "@/lib/marketing/landing";
import { Check, Files } from "@/components/icons/lucide";
import { useState } from "react";

interface LandingHeroProps {
  onGetStarted: () => void;
}

export function LandingHero({ onGetStarted }: LandingHeroProps) {
  const [copied, setCopied] = useState(false);

  async function handleCopySetup() {
    try {
      await navigator.clipboard.writeText(AGENT_SETUP_PROMPT);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopied(false);
    }
  }

  return (
    <section className="landing-shell pt-14 text-center md:pt-16">
      <a
        href="/#selfhost"
        className="mb-[30px] inline-flex items-center gap-2.5 rounded-full border border-[#ddd6fe] bg-[linear-gradient(#f5f3ff,#ede9fe)] px-3.5 py-1.5 pr-3.5 text-[13px] text-primary"
      >
        <span className="rounded-full bg-white px-2 py-0.5 text-[12px] font-medium text-primary shadow-sm">
          Cloud workspace
        </span>
        <span>ChatGPT</span>
      </a>

      <h1 className="mx-auto max-w-4xl text-[32px] leading-[1.05] font-medium tracking-[-0.02em] text-brand-dark sm:text-[42px] sm:leading-[1.08] md:text-[62px] md:leading-[65px] md:tracking-[-1.24px]">
        {siteConfig.heroLines[0]}
        <br />
        {siteConfig.heroLines[1]}
      </h1>

      <p className="mx-auto mt-5 max-w-[640px] text-[16px] leading-7 text-muted-foreground md:mt-6 md:text-[19px] md:leading-8">
        {siteConfig.cloudPitch}
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
          href={siteConfig.links.docs}
          target="_blank"
          rel="noreferrer"
          className="landing-btn-ghost inline-flex h-[48px] items-center justify-center rounded-[14px] px-[22px] text-[16px] font-medium sm:h-[44px] sm:text-[14px]"
        >
          View on GitHub
        </a>
      </div>

      <button
        type="button"
        onClick={() => void handleCopySetup()}
        className="mt-5 inline-flex items-center gap-2 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
        aria-label="Copy setup instructions for your agent"
      >
        <span className="font-mono text-primary/70">$</span>
        <span>Set up with your agent</span>
        {copied ? (
          <Check className="size-3.5 text-success" aria-hidden />
        ) : (
          <Files className="size-3.5" aria-hidden />
        )}
      </button>
    </section>
  );
}
