"use client";

import { siteConfig } from "@elsewhere/brand";
import { GitHubMark } from "@/components/icons/github-mark";
import { ProductLogo } from "@/components/product-logo";
import { appRoutes } from "@/lib/app-routes";
import { cn } from "cn";
import Link from "next/link";
import { useCallback, useEffect, useId, useState } from "react";

const navLinks = [
  { href: "/#demo", label: "Product" },
  { href: "/#roster", label: "Bots" },
  { href: "/#selfhost", label: "Computers" },
] as const;

interface LandingHeaderProps {
  onGetStarted: () => void;
}

export function LandingHeader({ onGetStarted }: LandingHeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuId = useId();

  const closeMenu = useCallback(() => {
    setMenuOpen(false);
  }, []);

  useEffect(() => {
    document.body.style.overflow = menuOpen ? "hidden" : "";
    return () => {
      document.body.style.overflow = "";
    };
  }, [menuOpen]);

  useEffect(() => {
    if (!menuOpen) {
      return;
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setMenuOpen(false);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [menuOpen]);

  return (
    <header className="relative z-50">
      <div className="landing-shell relative z-50 flex h-[86px] items-center justify-between gap-6">
        <div className="flex min-w-0 items-center gap-8">
          <Link
            href="/"
            className="flex shrink-0 items-center gap-2"
            aria-label={`${siteConfig.productName} home`}
            onClick={closeMenu}
          >
            <ProductLogo size="md" className="pointer-events-none" priority />
            <span className="text-[20px] tracking-tight text-brand-dark">
              {siteConfig.productName.toLowerCase()}
            </span>
          </Link>
          <nav className="hidden items-center gap-6 lg:flex" aria-label="Primary">
            {navLinks.map((link) => (
              <a
                key={link.href}
                href={link.href}
                className="text-[15px] text-brand-dark/80 transition-opacity hover:opacity-70"
              >
                {link.label}
              </a>
            ))}
          </nav>
        </div>

        <div className="hidden items-center gap-2.5 lg:flex">
          <a
            href={siteConfig.links.docs}
            target="_blank"
            rel="noreferrer"
            className="inline-flex h-[42px] items-center gap-1.5 rounded-[14px] px-2.5 text-[14px] text-brand-dark/80 transition-opacity hover:opacity-70"
          >
            <GitHubMark className="size-4 shrink-0" aria-hidden />
            <span>Source</span>
          </a>
          <Link
            href={appRoutes.workspace}
            className="landing-btn-primary inline-flex h-[42px] items-center whitespace-nowrap rounded-[14px] px-[18px] text-[14px] font-medium"
          >
            Open workspace
          </Link>
        </div>

        <button
          type="button"
          className="relative z-50 h-10 w-10 shrink-0 lg:hidden"
          aria-label="Toggle menu"
          aria-expanded={menuOpen}
          aria-controls={menuId}
          onClick={() => setMenuOpen((open) => !open)}
        >
          <span
            className={cn(
              "absolute left-1/2 h-[2px] w-6 -translate-x-1/2 rounded bg-brand-dark transition-all duration-300",
              menuOpen ? "top-[18px] rotate-45" : "top-[14px]",
            )}
          />
          <span
            className={cn(
              "absolute left-1/2 h-[2px] w-6 -translate-x-1/2 rounded bg-brand-dark transition-all duration-300",
              menuOpen ? "top-[18px] -rotate-45" : "top-[22px]",
            )}
          />
        </button>
      </div>

      {menuOpen ? (
        <div
          id={menuId}
          className="fixed inset-0 z-40 bg-background lg:hidden"
          role="dialog"
          aria-modal="true"
          aria-label="Menu"
        >
          <div className="flex h-full flex-col items-center justify-center gap-8 px-6 pt-[86px]">
            {navLinks.map((link) => (
              <a
                key={link.href}
                href={link.href}
                className="text-3xl tracking-tight text-brand-dark"
                onClick={closeMenu}
              >
                {link.label}
              </a>
            ))}
            <a
              href={siteConfig.links.docs}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-2 text-3xl tracking-tight text-brand-dark"
              onClick={closeMenu}
            >
              <GitHubMark className="size-7 shrink-0" aria-hidden />
              Source
            </a>
            <Link
              href={appRoutes.workspace}
              className="text-3xl tracking-tight text-brand-dark"
              onClick={closeMenu}
            >
              Workspace
            </Link>
            <button
              type="button"
              className="landing-btn-primary mt-2 inline-flex h-12 items-center rounded-[14px] px-8 text-lg font-medium"
              onClick={() => {
                closeMenu();
                onGetStarted();
              }}
            >
              Get started
            </button>
          </div>
        </div>
      ) : null}
    </header>
  );
}
