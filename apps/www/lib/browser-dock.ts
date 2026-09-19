export interface BrowserDockApp {
  id: string;
  name: string;
  icon: string;
  url: string;
  hosts: string[];
}

export const BROWSER_DOCK_APPS: BrowserDockApp[] = [
  {
    id: "browser",
    name: "Browser",
    icon: "/app-icons/browser.svg",
    url: "https://www.google.com",
    hosts: ["google.com", "www.google.com"],
  },
  {
    id: "mail",
    name: "Mail",
    icon: "/app-icons/mail.svg",
    url: "https://mail.google.com",
    hosts: ["mail.google.com"],
  },
  {
    id: "calendar",
    name: "Calendar",
    icon: "/app-icons/calendar.svg",
    url: "https://calendar.google.com",
    hosts: ["calendar.google.com"],
  },
  {
    id: "github",
    name: "GitHub",
    icon: "/app-icons/github.svg",
    url: "https://github.com",
    hosts: ["github.com"],
  },
  {
    id: "docs",
    name: "Docs",
    icon: "/app-icons/docs.svg",
    url: "https://docs.google.com",
    hosts: ["docs.google.com"],
  },
  {
    id: "figma",
    name: "Figma",
    icon: "/app-icons/figma.svg",
    url: "https://figma.com",
    hosts: ["figma.com", "www.figma.com"],
  },
];

export const BROWSER_DOCK_ICON_APPS = BROWSER_DOCK_APPS.map(({ id, name, icon }) => ({
  id,
  name,
  icon,
}));

function normalizeHost(host: string): string {
  return host.trim().toLowerCase().replace(/^www\./, "");
}

export function hostnameFromBrowserUrl(url: string | null | undefined): string | null {
  if (!url) {
    return null;
  }
  try {
    return new URL(url).hostname;
  } catch {
    return null;
  }
}

export function isNeutralBrowserUrl(url: string | null | undefined): boolean {
  if (!url) {
    return true;
  }
  const trimmed = url.trim().toLowerCase();
  return (
    trimmed === "about:blank" ||
    trimmed === "about:newtab" ||
    trimmed.startsWith("chrome://newtab") ||
    trimmed.startsWith("chrome://new-tab")
  );
}

export function dockAppIdForUrl(url: string | null | undefined): string | null {
  if (isNeutralBrowserUrl(url)) {
    return null;
  }
  const hostname = hostnameFromBrowserUrl(url);
  if (!hostname) {
    return null;
  }
  const normalized = normalizeHost(hostname);
  const match = BROWSER_DOCK_APPS.find((app) =>
    app.hosts.some((host) => normalizeHost(host) === normalized),
  );
  return match?.id ?? null;
}

export function browserDockAppById(appId: string): BrowserDockApp | null {
  return BROWSER_DOCK_APPS.find((app) => app.id === appId) ?? null;
}
