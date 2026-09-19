"use client";

import type { ActivityLine } from "@/lib/run-activity";
import { useState } from "react";

function splitActivityHeadline(headline: string): { lead: string; tail?: string } {
  const match = headline.match(/^(.+?)\s*(\([^)]+\))\s*$/);
  if (!match) {
    return { lead: headline };
  }
  return { lead: match[1].trim(), tail: match[2] };
}

export function RunActivityLine({ line }: { line: ActivityLine }) {
  const [showTechnical, setShowTechnical] = useState(false);
  const { lead, tail } = splitActivityHeadline(line.headline);

  return (
    <div className="flex justify-center px-2 py-0.5">
      <div className="max-w-lg text-center">
        <div className="inline-flex max-w-full flex-wrap items-baseline justify-center gap-x-1.5 gap-y-0.5">
          <span className="text-sm font-medium leading-snug text-foreground/85">{lead}</span>
          {tail ? (
            <span className="text-sm leading-snug text-muted-foreground">{tail}</span>
          ) : null}
          {line.technical ? (
            <button
              type="button"
              className="text-[9px] leading-none text-muted-foreground/55 underline-offset-2 hover:text-muted-foreground hover:underline"
              onClick={() => setShowTechnical((open) => !open)}
              aria-expanded={showTechnical}
            >
              {showTechnical ? "Hide" : "Details"}
            </button>
          ) : null}
        </div>
        {showTechnical && line.technical ? (
          <p className="mt-0.5 font-mono text-[9px] leading-tight text-muted-foreground/70">
            {line.technical}
          </p>
        ) : null}
      </div>
    </div>
  );
}
