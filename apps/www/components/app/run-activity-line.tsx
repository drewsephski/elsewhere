"use client";

import type { ActivityLine } from "@/lib/run-activity";
import { useState } from "react";

export function RunActivityLine({ line }: { line: ActivityLine }) {
  const [showTechnical, setShowTechnical] = useState(false);

  return (
    <div className="flex justify-center px-2">
      <div className="max-w-md text-center">
        <p className="text-[11px] leading-snug text-muted-foreground">{line.headline}</p>
        {line.technical ? (
          <button
            type="button"
            className="mt-0.5 text-[10px] text-muted-foreground/70 underline-offset-2 hover:text-muted-foreground hover:underline"
            onClick={() => setShowTechnical((open) => !open)}
            aria-expanded={showTechnical}
          >
            {showTechnical ? "Hide technical detail" : "Technical detail"}
          </button>
        ) : null}
        {showTechnical && line.technical ? (
          <p className="mt-0.5 font-mono text-[10px] text-muted-foreground/80">{line.technical}</p>
        ) : null}
      </div>
    </div>
  );
}
