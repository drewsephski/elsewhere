"use client";

import { GetStartedDialog } from "@/components/marketing/get-started-dialog";
import { LandingHeader } from "@/components/marketing/landing-header";
import { LandingHero } from "@/components/marketing/landing-hero";
import { ProductDemo } from "@/components/marketing/product-demo";
import { LandingFeatures } from "@/components/marketing/landing-features";
import { BotRoster } from "@/components/marketing/bot-roster";
import { CompareSection } from "@/components/marketing/compare-section";
import { StatsBand } from "@/components/marketing/stats-band";
import { LandingCta } from "@/components/marketing/landing-cta";
import { LandingFooter } from "@/components/marketing/landing-footer";
import { useState } from "react";

export function LandingPage() {
  const [startedOpen, setStartedOpen] = useState(false);

  function handleGetStarted() {
    setStartedOpen(true);
  }

  return (
    <div className="marketing-bg min-h-screen text-foreground">
      <LandingHeader onGetStarted={handleGetStarted} />
      <main>
        <LandingHero onGetStarted={handleGetStarted} />
        <ProductDemo onGetStarted={handleGetStarted} />
        <LandingFeatures />
        <BotRoster />
        <CompareSection />
        <StatsBand />
        <LandingCta onGetStarted={handleGetStarted} />
      </main>
      <LandingFooter />
      <GetStartedDialog open={startedOpen} onOpenChange={setStartedOpen} />
    </div>
  );
}
