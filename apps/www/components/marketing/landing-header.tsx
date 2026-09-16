"use client";

import { siteConfig } from "@elsewhere/brand";
import { ProductLogo } from "@/components/product-logo";
import { appRoutes } from "@/lib/app-routes";
import { cn } from "cn";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

const navLinks = [
  { href: "/#demo", label: "Product" },
  { href: "/#roster", label: "Bots" },
  { href: "/#selfhost", label: "Computers" },
  { href: siteConfig.links.docs, label: "Source", external: true },
] as const;

interface LandingHeaderProps {
  onGetStarted: () => void;
}

export function LandingHeader({ onGetStarted }: LandingHeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);

  const closeMenu = useCallback(() => {
    setMenuOpen(false);
  }, []);

  useEffect(() => {
    document.body.style.overflow = menuOpen ? "hidden" : "";
    return () => {
      document.body.style.overflow = "";
    };
  }, [menuOpen]);

  return (
    <>
      <header className="relative z-50">
        <div className="landing-shell flex h-[86px] items-center justify-between gap-6">
          <div className="flex min-w-0 items-center gap-8">
            <Link
              href="/"
              className="flex shrink-0 items-center gap-2"
              aria-label={`${siteConfig.productName} home`}
            >
              <ProductLogo size="md" className="pointer-events-none" priority />
              <span className="text-[20px] tracking-tight text-brand-dark">
                {siteConfig.productName.toLowerCase()}
              </span>
            </Link>
            <nav className="hidden items-center gap-6 lg:flex" aria-label="Primary">
              {navLinks.map((link) =>
                "external" in link && link.external ? (
                  <a
                    key={link.href}
                    href={link.href}
                    target="_blank"
                    rel="noreferrer"
                    className="text-[15px] text-brand-dark/80 transition-opacity hover:opacity-70"
                  >
                    {link.label}
                  </a>
                ) : (
                  <a
                    key={link.href}
                    href={link.href}
                    className="text-[15px] text-brand-dark/80 transition-opacity hover:opacity-70"
                  >
                    {link.label}
                  </a>
                ),
              )}
            </nav>
          </div>

          <div className="hidden items-center gap-2.5 lg:flex">
            <a
              href={siteConfig.links.docs}
              target="_blank"
              rel="noreferrer"
              className="inline-flex h-[42px] items-center gap-1.5 rounded-full border border-border bg-white px-3.5 text-[14px] text-brand-dark/80 transition-colors hover:bg-muted"
            >
              <span aria-hidden>★</span>
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
      </header>

      <div
        className={cn(
          "fixed inset-0 z-40 bg-background transition-opacity duration-500 lg:hidden",
          menuOpen ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0",
        )}
        aria-hidden={!menuOpen}
      >
        <div
          className={cn(
            "flex h-full flex-col items-center justify-center gap-8 px-6 transition-[transform,opacity] duration-500",
            menuOpen ? "translate-y-0 opacity-100" : "-translate-y-8 opacity-0",
          )}
        >
          {navLinks.map((link) =>
            "external" in link && link.external ? (
              <a
                key={link.href}
                href={link.href}
                target="_blank"
                rel="noreferrer"
                className="text-3xl tracking-tight text-brand-dark"
                onClick={closeMenu}
              >
                {link.label}
              </a>
            ) : (
              <a
                key={link.href}
                href={link.href}
                className="text-3xl tracking-tight text-brand-dark"
                onClick={closeMenu}
              >
                {link.label}
              </a>
            ),
          )}
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
    </>
  );
}
