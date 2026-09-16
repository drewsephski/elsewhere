"use client";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";
import { cn } from "cn";

const markdownComponents: Components = {
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="font-medium text-link underline-offset-2 hover:underline"
    >
      {children}
    </a>
  ),
};

export function MarkdownContent({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  if (!text.trim()) {
    return <p className="text-sm text-muted-foreground">(Empty file)</p>;
  }

  return (
    <div className={cn("prose-chat text-foreground", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {text}
      </ReactMarkdown>
    </div>
  );
}
