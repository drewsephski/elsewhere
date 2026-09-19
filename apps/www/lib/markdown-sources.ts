export type MarkdownSource = {
  href: string;
  title: string;
};

function normalizeHref(href: string): string {
  return href.replace(/[.,;:]+$/g, "");
}

function hostnameLabel(href: string): string {
  try {
    return new URL(href).hostname.replace(/^www\./, "");
  } catch {
    return href;
  }
}

export function extractMarkdownSources(text: string): MarkdownSource[] {
  const seen = new Set<string>();
  const sources: MarkdownSource[] = [];

  function add(href: string, title: string) {
    const url = normalizeHref(href);
    if (!url || seen.has(url)) {
      return;
    }
    seen.add(url);
    sources.push({
      href: url,
      title: title.trim() || hostnameLabel(url),
    });
  }

  for (const match of text.matchAll(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/gi)) {
    const title = match[1];
    const href = match[2];
    if (title && href) {
      add(href, title);
    }
  }

  const withoutMarkdownLinks = text.replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/gi, " ");
  for (const match of withoutMarkdownLinks.matchAll(/https?:\/\/[^\s)<\]]+/gi)) {
    add(match[0], "");
  }

  return sources;
}
