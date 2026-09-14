"use client";

import { marketingFeatures, siteConfig } from "@elsewhere/brand";
import { ArrowUpRight, ChevronDown } from "lucide-react";
import { useCallback, useId, useState } from "react";

const waitlistHref = `mailto:${siteConfig.contactEmail}?subject=${encodeURIComponent(siteConfig.waitlistMailSubject)}&body=${encodeURIComponent("Please add me to the cloud waitlist.")}`;

const productLinks = [
  {
    label: "Download",
    href: siteConfig.links.download,
    external: true,
  },
  {
    label: "Cloud waitlist",
    href: waitlistHref,
    external: false,
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
              Desktop now · Cloud next
            </p>
          </div>

          <ul className="grid grid-cols-2 gap-px bg-brand-dark/[0.06] p-px">
            {marketingFeatures.map((feature) => (
              <li key={feature.title} className="bg-white/95">
                <div
                  role="menuitem"
                  className="group/item h-full px-3 py-2.5 transition-colors duration-200 hover:bg-brand-cream/90"
                >
                  <p className="text-[13px] leading-tight tracking-tight text-brand-dark">
                    {feature.title}
                  </p>
                  <p className="mt-1 line-clamp-2 text-[11px] leading-snug text-brand-dark/50">
                    {feature.description}
                  </p>
                </div>
              </li>
            ))}
          </ul>

          <div className="flex items-center divide-x divide-brand-dark/[0.06] border-t border-brand-dark/[0.06] bg-brand-cream/35">
            {productLinks.map((link) => (
              <a
                key={link.label}
                href={link.href}
                target={link.external ? "_blank" : undefined}
                rel={link.external ? "noreferrer" : undefined}
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
            ))}
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
            {marketingFeatures.map((feature) => (
              <li key={feature.title}>
                <p className="text-base text-brand-dark">{feature.title}</p>
                <p className="mt-1 text-sm leading-relaxed text-brand-dark/60">
                  {feature.description}
                </p>
              </li>
            ))}
          </ul>
          <div className="mt-4 flex flex-col gap-2 border-t border-brand-dark/10 pt-4">
            {productLinks.map((link) => (
              <a
                key={link.label}
                href={link.href}
                target={link.external ? "_blank" : undefined}
                rel={link.external ? "noreferrer" : undefined}
                className="text-sm tracking-wide text-brand-dark uppercase"
                onClick={onNavigate}
              >
                {link.label}
              </a>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
