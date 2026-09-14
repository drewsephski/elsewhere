"use client";

import { marketingFeatures, siteConfig } from "@elsewhere/brand";
import { marketingFeatureAppHrefs, appRoutes } from "@/lib/app-routes";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "cn";
import { ArrowUpRight, ChevronDown } from "@/components/icons/lucide";
import Link from "next/link";
import { useState } from "react";

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
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            type="button"
            variant="ghost"
            className="gap-1 text-sm tracking-wide text-brand-dark uppercase hover:opacity-70"
          />
        }
      >
        Product
        <ChevronDown className="size-3.5 stroke-[2]" aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="start"
        sideOffset={12}
        className="w-[min(calc(100vw-2rem),40rem)] overflow-hidden rounded-xl border border-brand-dark/[0.08] bg-white/92 p-0 shadow-[0_16px_48px_-20px_rgba(45,58,46,0.24)] ring-1 ring-brand-dark/[0.04]"
      >
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
                  className="group/item flex h-full flex-col px-3 py-2.5 transition-colors duration-200 hover:bg-brand-cream/90"
                >
                  <p className="flex items-center gap-1 text-[13px] leading-tight tracking-tight text-brand-dark">
                    {feature.title}
                    <ArrowUpRight
                      className="size-3 text-brand-dark/25 transition-transform group-hover/item:translate-x-0.5 group-hover/item:-translate-y-0.5 group-hover/item:text-primary"
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
                className="group/link flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-[11px] tracking-[0.12em] text-brand-dark uppercase transition-colors hover:bg-white/70"
              >
                {link.label}
                <ArrowUpRight
                  className="size-3 text-brand-dark/30 transition-transform duration-200 group-hover/link:translate-x-0.5 group-hover/link:-translate-y-0.5 group-hover/link:text-primary"
                  aria-hidden
                />
              </a>
            ) : (
              <Link
                key={link.label}
                href={link.href}
                className="group/link flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-[11px] tracking-[0.12em] text-brand-dark uppercase transition-colors hover:bg-white/70"
              >
                {link.label}
                <ArrowUpRight
                  className="size-3 text-brand-dark/30 transition-transform duration-200 group-hover/link:translate-x-0.5 group-hover/link:-translate-y-0.5 group-hover/link:text-primary"
                  aria-hidden
                />
              </Link>
            ),
          )}
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

interface ProductMobileSectionProps {
  onNavigate?: () => void;
}

export function ProductMobileSection({ onNavigate }: ProductMobileSectionProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <Collapsible
      open={expanded}
      onOpenChange={setExpanded}
      className="flex w-full max-w-sm flex-col items-center gap-4"
    >
      <CollapsibleTrigger
        className="flex items-center gap-2 text-3xl tracking-tight text-brand-dark"
      >
        Product
        <ChevronDown
          className={cn(
            "size-6 transition-transform duration-300",
            expanded && "rotate-180",
          )}
          aria-hidden
        />
      </CollapsibleTrigger>

      <CollapsibleContent className="w-full">
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
      </CollapsibleContent>
    </Collapsible>
  );
}
