"use client";

import { siteConfig } from "@elsewhere/brand";
import { ArrowRight, Triangle } from "lucide-react";
import {
  ProductMobileSection,
  ProductNavDropdown,
} from "@/components/product-nav-dropdown";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

const HERO_VIDEO_SRC =
  "https://d8j0ntlcm91z4.cloudfront.net/user_38xzZboKViGWJOttwIXH07lWA1P/hf_20260820_010308_b1636845-4c15-4ab6-b0c9-9a29bfb0c6e3.mp4";

const waitlistHref = `mailto:${siteConfig.contactEmail}?subject=${encodeURIComponent(siteConfig.waitlistMailSubject)}&body=${encodeURIComponent("Please add me to the cloud waitlist.")}`;

const capabilityWordmarks = [
  { label: "Sandboxes", className: "font-playfair font-bold" },
  { label: "Routines", className: "font-oswald font-medium uppercase" },
  { label: "Memory", className: "font-montserrat font-bold" },
  { label: "Approvals", className: "font-roboto-slab font-semibold uppercase" },
] as const;

function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);

  useEffect(() => {
    function handleScroll() {
      setScrolled(window.scrollY > 20);
    }
    handleScroll();
    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  useEffect(() => {
    document.body.style.overflow = menuOpen ? "hidden" : "";
    return () => {
      document.body.style.overflow = "";
    };
  }, [menuOpen]);

  const closeMenu = useCallback(() => {
    setMenuOpen(false);
  }, []);

  return (
    <>
      <header
        className={`fixed top-0 right-0 left-0 z-50 transition-[background-color,box-shadow,backdrop-filter] duration-300 ${
          scrolled ? "border-b border-border/60 bg-white/90 shadow-sm backdrop-blur-md" : "bg-transparent"
        }`}
      >
        <div className="relative mx-auto flex h-16 w-full max-w-7xl items-center px-6 md:h-20 lg:px-8">
          <div className="hidden shrink-0 animate-fade-down items-center gap-8 md:flex stagger-1">
            <ProductNavDropdown />
            <a
              href={siteConfig.links.docs}
              target="_blank"
              rel="noreferrer"
              className="text-sm tracking-wide text-brand-dark uppercase transition-opacity hover:opacity-70"
            >
              Source
            </a>
            <a
              href={waitlistHref}
              className="text-sm tracking-wide text-brand-dark uppercase transition-opacity hover:opacity-70"
            >
              Cloud
            </a>
          </div>

          <Link
            href="/"
            className="absolute top-1/2 left-1/2 flex -translate-x-1/2 -translate-y-1/2 animate-fade-down items-center gap-2 stagger-2"
            aria-label={`${siteConfig.productName} home`}
          >
            <Triangle
              className="pointer-events-none h-5 w-5 fill-primary text-primary"
              strokeWidth={0}
              aria-hidden
            />
            <span className="pointer-events-auto font-helvetica-neue text-xl tracking-tight text-brand-dark">
              {siteConfig.productName}
            </span>
          </Link>

          <a
            href={siteConfig.links.download}
            className="ml-auto hidden shrink-0 animate-fade-down items-center rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground shadow-sm transition-colors stagger-3 hover:bg-primary/90 md:inline-flex"
          >
            Download
          </a>

          <button
            type="button"
            className="relative z-50 ml-auto h-10 w-10 shrink-0 md:hidden"
            aria-label="Toggle menu"
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((open) => !open)}
          >
            <span
              className={`absolute left-1/2 h-[2px] w-6 -translate-x-1/2 rounded bg-brand-dark transition-all duration-300 ease-[cubic-bezier(0.68,-0.6,0.32,1.6)] ${
                menuOpen ? "top-[6px] translate-y-[5px] rotate-45" : "top-[6px]"
              }`}
            />
            <span
              className={`absolute left-1/2 h-[2px] w-6 -translate-x-1/2 rounded bg-brand-dark transition-all duration-300 ease-[cubic-bezier(0.68,-0.6,0.32,1.6)] ${
                menuOpen ? "top-[13px] -rotate-45" : "top-[13px]"
              }`}
            />
          </button>
        </div>
      </header>

      <div
        className={`fixed inset-0 z-40 bg-background transition-opacity duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] md:hidden ${
          menuOpen ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0"
        }`}
        aria-hidden={!menuOpen}
      >
        <div
          className={`flex h-full flex-col items-center justify-center gap-8 transition-[transform,opacity] delay-100 duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] ${
            menuOpen ? "translate-y-0 opacity-100" : "-translate-y-8 opacity-0"
          }`}
        >
          <ProductMobileSection onNavigate={closeMenu} />
          <a
            href={siteConfig.links.docs}
            target="_blank"
            rel="noreferrer"
            className="text-3xl tracking-tight text-brand-dark"
            onClick={closeMenu}
          >
            Source
          </a>
          <a href={waitlistHref} className="text-3xl tracking-tight text-brand-dark" onClick={closeMenu}>
            Cloud
          </a>
          <a
            href={siteConfig.links.download}
            className="mt-4 inline-flex items-center rounded-lg bg-primary px-8 py-3.5 text-lg font-medium text-primary-foreground shadow-sm"
            onClick={closeMenu}
          >
            Download
          </a>
        </div>
      </div>
    </>
  );
}

function CapabilityRow() {
  return (
    <div className="mt-8 w-full animate-fade-up md:mt-10 stagger-6">
      <p className="mb-6 text-left font-helvetica-neue text-xs tracking-[0.25em] text-brand-dark/50 uppercase md:mb-8">
        Built for
      </p>
      <div className="flex flex-wrap items-center justify-start gap-x-6 gap-y-3 md:gap-x-12 lg:gap-x-16">
        {capabilityWordmarks.map((item) => (
          <span
            key={item.label}
            className={`text-lg text-brand-dark/80 md:text-xl lg:text-2xl ${item.className}`}
          >
            {item.label}
          </span>
        ))}
      </div>
    </div>
  );
}

function Hero() {
  return (
    <section className="relative h-screen min-h-[700px] w-full overflow-hidden marketing-bg">
      <div className="absolute inset-0 z-0">
        <video
          className="h-full w-full -scale-x-100 object-cover object-bottom"
          src={HERO_VIDEO_SRC}
          autoPlay
          muted
          loop
          playsInline
        />
      </div>

      <div className="relative z-10 mx-auto flex w-full max-w-7xl flex-col items-start px-6 pt-28 md:pt-36 lg:px-8">
        <a
          href={waitlistHref}
          className="mb-5 inline-flex animate-fade-up items-center gap-2 rounded-full border border-brand-dark/15 bg-white/60 px-4 py-2 backdrop-blur-sm transition-colors hover:bg-white/80 md:mb-6 stagger-3"
        >
          <span className="text-sm text-brand-dark">
            macOS desktop is live — join the cloud waitlist.
          </span>
          <ArrowRight className="h-3.5 w-3.5 shrink-0 text-brand-dark" strokeWidth={2} aria-hidden />
        </a>

        <h1
          className="max-w-4xl animate-fade-up text-left font-helvetica-neue text-3xl leading-[1.05] font-light tracking-tight text-brand-dark sm:text-4xl md:text-5xl lg:text-6xl stagger-4"
        >
          {siteConfig.heroLines[0]}
          <br className="hidden sm:block" />
          {" "}
          {siteConfig.heroLines[1]}
        </h1>

        <p className="mt-5 max-w-2xl animate-fade-up text-base leading-relaxed text-brand-dark/75 md:text-lg stagger-5">
          {siteConfig.cloudPitch}
        </p>

        <CapabilityRow />
      </div>
    </section>
  );
}

export default function HomePage() {
  return (
    <div className="marketing-bg min-h-screen font-sans text-foreground">
      <Navbar />
      <Hero />
    </div>
  );
}
