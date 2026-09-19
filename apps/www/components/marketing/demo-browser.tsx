"use client";

import {
  ArrowUpRight,
  ChevronLeft,
  ChevronRight,
  Plus,
  RefreshCw,
  X,
} from "@/components/icons/lucide";
import { BrowserPage } from "@/components/marketing/demo-browser-pages";
import { MacOSDock } from "@/components/ui/mac-os-dock";
import { BROWSER_DOCK_ICON_APPS } from "@/lib/browser-dock";
import {
  demoComputerForDockApp,
  demoDockAppIdForPage,
  type DemoBrowserPage,
  type DemoComputer,
  type DemoDockAppId,
} from "@/lib/marketing/landing";
import { cn } from "cn";
import Link from "next/link";
import { useMemo, useState, type ReactNode } from "react";

type DemoNavAppId = DemoDockAppId | "session";

interface DemoNavFrame {
  appId: DemoNavAppId;
  path: string;
}

interface DemoBrowserProps {
  computer: DemoComputer;
  controlHref: string;
}

export const SITE_CHROME: Record<
  DemoBrowserPage["kind"],
  { product: string; color: string; letter: string; host: string }
> = {
  mail: { product: "Gmail", color: "#ea4335", letter: "M", host: "mail.google.com" },
  calendar: { product: "Calendar", color: "#1a73e8", letter: "C", host: "calendar.google.com" },
  accounts: { product: "Inbound", color: "#7c3aed", letter: "A", host: "accounts.elsewhere" },
  github: { product: "GitHub", color: "#24292f", letter: "G", host: "github.com" },
  figma: { product: "Figma", color: "#a259ff", letter: "F", host: "figma.com" },
  expenses: { product: "Expensify", color: "#0d9488", letter: "E", host: "expensify.com" },
  sheet: { product: "Sheets", color: "#0f9d58", letter: "S", host: "docs.google.com" },
  ads: { product: "Ads", color: "#4285f4", letter: "A", host: "ads.google.com" },
};

export function locationFor(
  computer: DemoComputer,
  path: string,
): { url: string; title: string } {
  if (path === "ntp") return { url: "chrome://newtab", title: "New Tab" };
  if (path === "ntp-miss") return { url: "chrome://newtab", title: "Not found" };

  const { page, url, tab } = computer;
  switch (page.kind) {
    case "mail": {
      if (path.startsWith("mail:thread:")) {
        const id = path.slice("mail:thread:".length);
        const thread = page.threads.find((item) => item.id === id);
        return {
          url: `mail.google.com/mail/u/0/#inbox/${id}`,
          title: thread?.subject ?? "Mail",
        };
      }
      if (path === "mail:drafts") return { url: "mail.google.com/mail/u/0/#drafts", title: "Drafts" };
      if (path === "mail:starred") return { url: "mail.google.com/mail/u/0/#starred", title: "Starred" };
      if (path === "mail:sent") return { url: "mail.google.com/mail/u/0/#sent", title: "Sent" };
      if (path === "mail:compose") return { url: "mail.google.com/mail/u/0/#compose", title: "Compose" };
      return { url: "mail.google.com/mail/u/0/#inbox", title: "Inbox" };
    }
    case "calendar": {
      if (path.startsWith("cal:event:")) {
        const id = path.slice("cal:event:".length);
        const event = page.events.find((item) => item.id === id);
        return {
          url: `calendar.google.com/calendar/event?eid=${id}`,
          title: event?.title ?? "Event",
        };
      }
      if (path.startsWith("cal:day:")) {
        const day = path.slice("cal:day:".length);
        return { url: `calendar.google.com/calendar/u/0/r/day/2026/9/${day}`, title: `Sep ${day}` };
      }
      return { url: "calendar.google.com/calendar/u/0/r/week", title: "Week" };
    }
    case "accounts": {
      if (path.startsWith("account:")) {
        const name = path.slice("account:".length);
        return { url: `accounts.elsewhere/inbound/${name.toLowerCase()}`, title: name };
      }
      return { url, title: tab };
    }
    case "github": {
      if (path.startsWith("issue:")) {
        const number = path.slice("issue:".length);
        return { url: `github.com/${page.repo}/issues/${number}`, title: `#${number}` };
      }
      return { url: `github.com/${page.repo}/issues`, title: "Issues" };
    }
    case "figma": {
      if (path.startsWith("frame:")) {
        const label = path.slice("frame:".length);
        return { url: `figma.com/design/settings-redesign?node-id=${label.toLowerCase()}`, title: label };
      }
      return { url, title: tab };
    }
    case "expenses": {
      if (path.startsWith("receipt:")) {
        const merchant = path.slice("receipt:".length);
        return { url: `expensify.com/reports/sep-8/${merchant.toLowerCase()}`, title: merchant };
      }
      return { url, title: tab };
    }
    case "sheet": {
      if (path.startsWith("cell:")) {
        const [, row, col] = path.split(":");
        const letter = String.fromCharCode(65 + Number(col ?? 0));
        return { url: `${url}#gid=0&range=${letter}${Number(row ?? 0) + 1}`, title: `${letter}${Number(row ?? 0) + 1}` };
      }
      return { url, title: tab };
    }
    case "ads": {
      if (path.startsWith("campaign:")) {
        const name = path.slice("campaign:".length);
        return { url: `ads.google.com/campaigns/${encodeURIComponent(name)}`, title: name };
      }
      return { url, title: tab };
    }
  }
}

function initialNavFrame(computer: DemoComputer): DemoNavFrame {
  return {
    appId: demoDockAppIdForPage(computer.page.kind) ?? "session",
    path: "home",
  };
}

function computerForNavFrame(computer: DemoComputer, frame: DemoNavFrame): DemoComputer {
  if (frame.appId === "browser" || frame.appId === "session") {
    return computer;
  }
  return demoComputerForDockApp(frame.appId);
}

export function DemoBrowser({ computer, controlHref }: DemoBrowserProps) {
  const [stack, setStack] = useState<DemoNavFrame[]>(() => [initialNavFrame(computer)]);
  const [cursor, setCursor] = useState(0);
  const [reloading, setReloading] = useState(false);
  const [omniboxOpen, setOmniboxOpen] = useState(false);
  const [bookmarked, setBookmarked] = useState(true);
  const [closedHome, setClosedHome] = useState(false);

  const frame = stack[cursor] ?? initialNavFrame(computer);
  const path = frame.path;
  const activeComputer = computerForNavFrame(computer, frame);
  const location = useMemo(
    () => locationFor(activeComputer, path),
    [activeComputer, path],
  );
  const site = SITE_CHROME[activeComputer.page.kind];
  const onNtp = path === "ntp" || path === "ntp-miss" || closedHome;
  const activeDockAppId: DemoDockAppId | null = onNtp
    ? "browser"
    : demoDockAppIdForPage(activeComputer.page.kind);
  const canBack = cursor > 0;
  const canForward = cursor < stack.length - 1;

  function goTo(nextPath: string, appId: DemoNavAppId = frame.appId) {
    if (nextPath === path && appId === frame.appId && !closedHome) return;
    setClosedHome(false);
    setStack((current) => [...current.slice(0, cursor + 1), { appId, path: nextPath }]);
    setCursor((current) => current + 1);
  }

  function handleDockAppClick(appId: string) {
    const dockAppId = appId as DemoDockAppId;
    if (activeDockAppId === dockAppId) {
      if (dockAppId === "browser") {
        return;
      }
      goTo("ntp", "browser");
      return;
    }
    if (dockAppId === "browser") {
      goTo("ntp", "browser");
      return;
    }
    goTo("home", dockAppId);
  }

  function handleBack() {
    if (!canBack) return;
    setCursor((current) => current - 1);
  }

  function handleForward() {
    if (!canForward) return;
    setCursor((current) => current + 1);
  }

  function handleReload() {
    setReloading(true);
    window.setTimeout(() => setReloading(false), 520);
  }

  function handleNewTab() {
    goTo("ntp", "browser");
  }

  function handleCloseHomeTab() {
    setClosedHome(true);
    if (!onNtp) goTo("ntp", "browser");
  }

  function handleCloseNtpTab() {
    const homeApp = demoDockAppIdForPage(computer.page.kind) ?? "session";
    setClosedHome(false);
    goTo("home", homeApp);
  }

  function handleOmniboxSubmit(value: string) {
    const query = value.trim().toLowerCase();
    setOmniboxOpen(false);
    if (!query || query === location.url || query === `https://${location.url}`) {
      const homeApp = demoDockAppIdForPage(computer.page.kind) ?? "session";
      goTo("home", homeApp);
      return;
    }
    goTo("ntp-miss", "browser");
  }

  return (
    <section
      className="flex min-h-[280px] flex-1 flex-col overflow-hidden rounded-xl border border-white/10 bg-[#202124] shadow-[0_18px_40px_-20px_rgba(0,0,0,0.85)]"
      aria-label={`${location.title} — ${location.url}`}
    >
      <div className="flex shrink-0 items-end gap-1 bg-[#202124] px-1 pt-1">
        {!closedHome ? (
          <TabChip
            active={!onNtp}
            color={site.color}
            letter={site.letter}
            title={onNtp ? computer.tab : location.title}
            onSelect={() =>
              goTo(
                "home",
                frame.appId === "browser"
                  ? (demoDockAppIdForPage(computer.page.kind) ?? "session")
                  : frame.appId,
              )
            }
            onClose={handleCloseHomeTab}
          />
        ) : null}
        {onNtp ? (
          <TabChip
            active
            color="#9aa0a6"
            letter="+"
            title="New Tab"
            onSelect={() => goTo("ntp", "browser")}
            onClose={handleCloseNtpTab}
          />
        ) : null}
        <button
          type="button"
          onClick={handleNewTab}
          className="mb-1 inline-flex size-6 items-center justify-center rounded-full text-white/35 transition-colors hover:bg-white/10 hover:text-white"
          aria-label="New tab"
        >
          <Plus className="size-3" />
        </button>
      </div>

      <div className="flex shrink-0 items-center gap-1 bg-[#292a2d] px-1.5 py-1">
        <ChromeIconButton label="Back" disabled={!canBack} onClick={handleBack}>
          <ChevronLeft className="size-3.5" />
        </ChromeIconButton>
        <ChromeIconButton label="Forward" disabled={!canForward} onClick={handleForward}>
          <ChevronRight className="size-3.5" />
        </ChromeIconButton>
        <ChromeIconButton label="Reload" onClick={handleReload}>
          <RefreshCw className={cn("size-3", reloading && "animate-spin")} />
        </ChromeIconButton>

        {omniboxOpen ? (
          <OmniboxEditor
            defaultValue={`https://${location.url}`}
            onSubmit={handleOmniboxSubmit}
            onCancel={() => setOmniboxOpen(false)}
          />
        ) : (
          <div className="flex min-w-0 flex-1 items-center gap-1 rounded-full bg-[#1f1f1f] px-2 py-1 ring-1 ring-white/5 hover:ring-white/15">
            <button
              type="button"
              onClick={() => setOmniboxOpen(true)}
              className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
              aria-label={`Address bar, ${location.url}`}
            >
              <LockMark />
              <span className="min-w-0 flex-1 truncate font-mono text-[10.5px] tracking-tight">
                <span className="text-white/80">
                  {location.url.split("/")[0]}
                </span>
                {location.url.includes("/") ? (
                  <span className="text-white/40">
                    /{location.url.split("/").slice(1).join("/")}
                  </span>
                ) : null}
              </span>
            </button>
            <button
              type="button"
              aria-pressed={bookmarked}
              aria-label={bookmarked ? "Remove bookmark" : "Bookmark this tab"}
              onClick={() => setBookmarked((current) => !current)}
              className={cn(
                "inline-flex size-4 shrink-0 items-center justify-center rounded-sm",
                bookmarked ? "text-[#f9ab00]" : "text-white/30 hover:text-white/70",
              )}
            >
              <StarMark filled={bookmarked} />
            </button>
          </div>
        )}

        <Link
          href={controlHref}
          className="inline-flex shrink-0 items-center gap-1 rounded-full bg-white/8 px-2 py-1 text-[10.5px] font-medium text-white/80 transition-colors hover:bg-white/14 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-white"
        >
          Take control
          <ArrowUpRight className="size-3 text-white/45" aria-hidden />
        </Link>
      </div>

      <div className="relative h-0.5 bg-transparent">
        <span
          className={cn(
            "absolute inset-y-0 left-0 bg-[#8ab4f8] transition-all duration-500",
            reloading ? "w-full opacity-100" : "w-0 opacity-0",
          )}
          aria-hidden
        />
      </div>

      <div className="relative min-h-0 flex-1 overflow-hidden bg-white">
        {reloading ? (
          <div className="absolute inset-0 z-10 bg-white/55" aria-hidden />
        ) : null}
        {path === "ntp" || closedHome ? (
          <NewTabPage
            computer={computer}
            onOpenApp={(appId) => {
              setClosedHome(false);
              goTo("home", appId);
            }}
            onMiss={() => goTo("ntp-miss", "browser")}
          />
        ) : path === "ntp-miss" ? (
          <MissPage onBack={() => goTo("ntp", "browser")} />
        ) : (
          <BrowserPage page={activeComputer.page} path={path} goTo={goTo} />
        )}
        <div className="pointer-events-none absolute inset-x-0 bottom-1.5 z-20 flex justify-center px-3">
          <div className="pointer-events-auto w-full max-w-[220px]">
            <MacOSDock
              apps={BROWSER_DOCK_ICON_APPS}
              onAppClick={handleDockAppClick}
              openApps={activeDockAppId ? [activeDockAppId] : []}
              size="mini"
            />
          </div>
        </div>
      </div>
    </section>
  );
}

function TabChip({
  active,
  color,
  letter,
  title,
  onSelect,
  onClose,
}: {
  active: boolean;
  color: string;
  letter: string;
  title: string;
  onSelect: () => void;
  onClose: () => void;
}) {
  return (
    <div
      className={cn(
        "group relative flex min-w-0 flex-1 items-center gap-1.5 rounded-t-lg px-2 py-1.5",
        active ? "bg-[#292a2d]" : "bg-transparent hover:bg-white/6",
      )}
    >
      <button
        type="button"
        onClick={onSelect}
        className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
        aria-current={active ? "page" : undefined}
      >
        <span
          className="flex size-3.5 shrink-0 items-center justify-center rounded-[3px] text-[8px] font-bold text-white"
          style={{ backgroundColor: color }}
          aria-hidden
        >
          {letter}
        </span>
        <span className="truncate text-[11px] text-white/80">{title}</span>
      </button>
      <button
        type="button"
        onClick={onClose}
        className="inline-flex size-4 shrink-0 items-center justify-center rounded-full text-white/0 transition-colors group-hover:text-white/45 hover:bg-white/10 hover:text-white"
        aria-label={`Close ${title}`}
      >
        <X className="size-2.5" />
      </button>
    </div>
  );
}

function ChromeIconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className="inline-flex size-6 items-center justify-center rounded-full text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:text-white/20 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

function OmniboxEditor({
  defaultValue,
  onSubmit,
  onCancel,
}: {
  defaultValue: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(defaultValue);

  return (
    <form
      className="flex min-w-0 flex-1 items-center rounded-full bg-[#1f1f1f] px-2 py-1 ring-2 ring-[#8ab4f8]"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(value);
      }}
    >
      <label className="sr-only" htmlFor="demo-omnibox">
        Address bar
      </label>
      <input
        id="demo-omnibox"
        autoFocus
        value={value}
        onChange={(event) => setValue(event.target.value)}
        onBlur={onCancel}
        onKeyDown={(event) => {
          if (event.key === "Escape") onCancel();
        }}
        className="w-full bg-transparent font-mono text-[10.5px] text-white outline-none"
      />
    </form>
  );
}

function NewTabPage({
  computer,
  onOpenApp,
  onMiss,
}: {
  computer: DemoComputer;
  onOpenApp: (appId: DemoNavAppId) => void;
  onMiss: () => void;
}) {
  const homeApp = demoDockAppIdForPage(computer.page.kind) ?? "session";
  const site = SITE_CHROME[computer.page.kind];
  const [query, setQuery] = useState("");

  return (
    <div className="flex h-full flex-col items-center bg-[#202124] px-4 pt-8 text-white">
      <p className="text-[22px] font-medium tracking-tight text-white/90">Google</p>
      <form
        className="mt-4 flex w-full items-center gap-2 rounded-full bg-[#303134] px-3 py-2 ring-1 ring-white/10"
        onSubmit={(event) => {
          event.preventDefault();
          if (!query.trim()) {
            onOpenApp(homeApp);
            return;
          }
          onMiss();
        }}
      >
        <SearchMark />
        <label className="sr-only" htmlFor="ntp-search">
          Search Google or type a URL
        </label>
        <input
          id="ntp-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search Google or type a URL"
          className="w-full bg-transparent text-[12px] text-white outline-none placeholder:text-white/35"
        />
      </form>
      <div className="mt-6 grid w-full grid-cols-4 gap-2">
        <ShortcutTile
          color={site.color}
          letter={site.letter}
          label={site.product}
          onClick={() => onOpenApp(homeApp)}
        />
        <ShortcutTile
          color="#1a73e8"
          letter="C"
          label="Calendar"
          onClick={() => onOpenApp("calendar")}
        />
        <ShortcutTile
          color="#0f9d58"
          letter="D"
          label="Docs"
          onClick={() => onOpenApp("docs")}
        />
        <ShortcutTile color="#7c3aed" letter="E" label="Elsewhere" muted />
      </div>
    </div>
  );
}

function ShortcutTile({
  color,
  letter,
  label,
  muted,
  onClick,
}: {
  color: string;
  letter: string;
  label: string;
  muted?: boolean;
  onClick?: () => void;
}) {
  const className = cn(
    "flex flex-col items-center gap-1.5 rounded-xl px-1 py-2 text-white/70",
    muted ? "opacity-45" : "hover:bg-white/8 hover:text-white",
  );

  const inner = (
    <>
      <span
        className="flex size-9 items-center justify-center rounded-full text-[13px] font-bold text-white"
        style={{ backgroundColor: color }}
        aria-hidden
      >
        {letter}
      </span>
      <span className="truncate text-[10px]">{label}</span>
    </>
  );

  if (muted || !onClick) {
    return (
      <span className={className} aria-hidden>
        {inner}
      </span>
    );
  }

  return (
    <button type="button" onClick={onClick} className={className}>
      {inner}
    </button>
  );
}

function MissPage({ onBack }: { onBack: () => void }) {
  return (
    <div className="flex h-full flex-col bg-white px-4 py-5 text-[#202124]">
      <p className="text-[15px] font-medium">This site can’t be reached</p>
      <p className="mt-1 text-[12px] leading-5 text-[#5f6368]">
        This computer stays signed into the bot’s current tools. Use the shortcut to go back.
      </p>
      <button
        type="button"
        onClick={onBack}
        className="mt-4 self-start rounded-md bg-[#1a73e8] px-3 py-1.5 text-[12px] font-medium text-white hover:bg-[#1765cc]"
      >
        Back to safety
      </button>
    </div>
  );
}

function LockMark() {
  return (
    <svg viewBox="0 0 12 12" className="size-2.5 shrink-0 text-[#8ab4f8]" aria-hidden>
      <path
        fill="currentColor"
        d="M6 1.4A2.1 2.1 0 0 0 3.9 3.5V4.8H3.2a.8.8 0 0 0-.8.8v3.6a.8.8 0 0 0 .8.8h5.6a.8.8 0 0 0 .8-.8V5.6a.8.8 0 0 0-.8-.8H8.1V3.5A2.1 2.1 0 0 0 6 1.4Zm1.1 3.4H4.9V3.5a1.1 1.1 0 1 1 2.2 0v1.3Z"
      />
    </svg>
  );
}

function StarMark({ filled }: { filled: boolean }) {
  return (
    <svg viewBox="0 0 12 12" className="size-2.5" aria-hidden>
      <path
        d="M6 1.4 7.2 4l2.8.4-2 2 .5 2.8L6 7.9 3.5 9.2 4 6.4l-2-2 2.8-.4Z"
        fill={filled ? "currentColor" : "none"}
        stroke="currentColor"
        strokeLinejoin="round"
        strokeWidth="1.1"
      />
    </svg>
  );
}

function SearchMark() {
  return (
    <svg viewBox="0 0 16 16" className="size-3.5 shrink-0 text-white/40" aria-hidden>
      <circle cx="7" cy="7" r="4.2" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M10.2 10.2 13 13" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}
