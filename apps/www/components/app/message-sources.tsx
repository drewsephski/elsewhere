"use client";

import {
  Source,
  Sources,
  SourcesContent,
  SourcesTrigger,
} from "@/components/ai-elements/sources";
import { extractMarkdownSources } from "@/lib/markdown-sources";

export function MessageSources({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  const sources = extractMarkdownSources(text);
  if (sources.length === 0) {
    return null;
  }

  return (
    <Sources className={className}>
      <SourcesTrigger count={sources.length} />
      <SourcesContent>
        {sources.map((source) => (
          <Source href={source.href} key={source.href} title={source.title} />
        ))}
      </SourcesContent>
    </Sources>
  );
}
