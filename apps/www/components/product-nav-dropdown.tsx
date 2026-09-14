"use client";

import { marketingFeatures, siteConfig } from "@elsewhere/brand";
import { marketingFeatureAppHrefs, appRoutes } from "@/lib/app-routes";
import { ArrowUpRight, ChevronDown } from "lucide-react";
import Link from "next/link";
import { useCallback, useId, useState } from "react";

const productLinks = [
  {
    label: "Open workspace",
    href: appRoutes.workspace,
    external: false,
  },
  {
    label: "Download",
    href: siteConfig.links.download,
    external: true,
  },
] as const;

export function ProductNavDropdown() {
  const menuId = useId();
  const [open, setOpen] = useState(false);

  const handleOpen = useCallback(() => {
    setOpen(true);
  }, []);

  const handleClose = useCallback(() => {
    setOpen(false);
  }, []);

  const handleBlur = useCallback((event: React.FocusEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      setOpen(false);
    }
  }, []);

  return (
    <div
      className="relative"
      onMouseEnter={handleOpen}
      onMouseLeave={handleClose}
      onFocus={handleOpen}
      onBlur={handleBlur}
    >
      <button
        type="button"
        className="flex items-center gap-1 text-sm tracking-wide text-brand-dark uppercase transition-opacity hover:opacity-70"
        aria-expanded={open}
        aria-haspopup="true"
        aria-controls={menuId}
      >
        Product
        <ChevronDown
          className={`h-3.5 w-3.5 stroke-[2] transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
            open ? "rotate-180" : ""
          }`}
          aria-hidden
        />
      </button>

      <div
        id={menuId}
        role="menu"
        aria-hidden={!open}
        className={`absolute top-full left-0 z-50 pt-4 transition-[opacity,transform,visibility] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
          open
            ? "visible translate-y-0 opacity-100"
            : "invisible -translate-y-1 opacity-0 pointer-events-none"
        }`}
      >
        <div className="w-[min(calc(100vw-2rem),40rem)] overflow-hidden rounded-xl border border-brand-dark/[0.08] bg-white/92 shadow-[0_16px_48px_-20px_rgba(45,58,46,0.24)] ring-1 ring-brand-dark/[0.04] backdrop-blur-xl">
          <div className="flex items-center justify-between gap-4 border-b border-brand-dark/[0.06] px-4 py-2.5">
            <p className="text-[10px] tracking-[0.18em] text-brand-dark/45 uppercase">
              Platform
            </p>
            <p className="truncate text-[11px] text-brand-dark/50">
              Desktop now · Cloud workspace live
            </p>
          </div>

          <ul className="grid grid-cols-2 gap-px bg-brand-dark/[0.06] p-px">
            {marketingFeatures.map((feature, index) => {
              const href = marketingFeatureAppHrefs[index] ?? appRoutes.workspace;
              return (
                <li key={feature.title} className="bg-white/95">
                  <Link
                    href={href}
                    role="menuitem"
                    className="group/item flex h-full flex-col px-3 py-2.5 transition-colors duration-200 hover:bg-brand-cream/90"
                  >
                    <p className="flex items-center gap-1 text-[13px] leading-tight tracking-tight text-brand-dark">
                      {feature.title}
                      <ArrowUpRight
                        className="h-3 w-3 text-brand-dark/25 transition-transform group-hover/item:translate-x-0.5 group-hover/item:-translate-y-0.5 group-hover/item:text-primary"
                        strokeWidth={2}
                        aria-hidden
                      />
                    </p>
                    <p className="mt-1 line-clamp-2 text-[11px] leading-snug text-brand-dark/50">
                      {feature.description}
                    </p>
                  </Link>
                </li>
              );
            })}
          </ul>

          <div className="flex items-center divide-x divide-brand-dark/[0.06] border-t border-brand-dark/[0.06] bg-brand-cream/35">
            {productLinks.map((link) =>
              link.external ? (
                <a
                  key={link.label}
                  href={link.href}
                  target="_blank"
                  rel="noreferrer"
                  role="menuitem"
                  className="group/link flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-[11px] tracking-[0.12em] text-brand-dark uppercase transition-colors hover:bg-white/70"
                >
                  {link.label}
                  <ArrowUpRight
                    className="h-3 w-3 text-brand-dark/30 transition-transform duration-200 group-hover/link:translate-x-0.5 group-hover/link:-translate-y-0.5 group-hover/link:text-primary"
                    strokeWidth={2}
                    aria-hidden
                  />
                </a>
              ) : (
                <Link
                  key={link.label}
                  href={link.href}
                  role="menuitem"
                  className="group/link flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-[11px] tracking-[0.12em] text-brand-dark uppercase transition-colors hover:bg-white/70"
                >
                  {link.label}
                  <ArrowUpRight
                    className="h-3 w-3 text-brand-dark/30 transition-transform duration-200 group-hover/link:translate-x-0.5 group-hover/link:-translate-y-0.5 group-hover/link:text-primary"
                    strokeWidth={2}
                    aria-hidden
                  />
                </Link>
              ),
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

interface ProductMobileSectionProps {
  onNavigate?: () => void;
}

export function ProductMobileSection({ onNavigate }: ProductMobileSectionProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="flex w-full max-w-sm flex-col items-center gap-4">
      <button
        type="button"
        className="flex items-center gap-2 text-3xl tracking-tight text-brand-dark"
        aria-expanded={expanded}
        onClick={() => setExpanded((value) => !value)}
      >
        Product
        <ChevronDown
          className={`h-6 w-6 transition-transform duration-300 ${expanded ? "rotate-180" : ""}`}
          aria-hidden
        />
      </button>

      <div
        className={`w-full overflow-hidden transition-[max-height,opacity] duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] ${
          expanded ? "max-h-[640px] opacity-100" : "max-h-0 opacity-0"
        }`}
      >
        <div className="rounded-2xl border border-brand-dark/10 bg-white/60 p-4 text-left backdrop-blur-sm">
          <ul className="space-y-3">
            {marketingFeatures.map((feature, index) => {
              const href = marketingFeatureAppHrefs[index] ?? appRoutes.workspace;
              return (
                <li key={feature.title}>
                  <Link
                    href={href}
                    className="block text-base text-brand-dark underline-offset-2 hover:underline"
                    onClick={onNavigate}
                  >
                    {feature.title}
                  </Link>
                  <p className="mt-1 text-sm leading-relaxed text-brand-dark/60">
                    {feature.description}
                  </p>
                </li>
              );
            })}
          </ul>
          <div className="mt-4 flex flex-col gap-2 border-t border-brand-dark/10 pt-4">
            {productLinks.map((link) =>
              link.external ? (
                <a
                  key={link.label}
                  href={link.href}
                  target="_blank"
                  rel="noreferrer"
                  className="text-sm tracking-wide text-brand-dark uppercase"
                  onClick={onNavigate}
                >
                  {link.label}
                </a>
              ) : (
                <Link
                  key={link.label}
                  href={link.href}
                  className="text-sm tracking-wide text-brand-dark uppercase"
                  onClick={onNavigate}
                >
                  {link.label}
                </Link>
              ),
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
