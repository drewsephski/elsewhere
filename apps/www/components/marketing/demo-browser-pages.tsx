"use client";

import type {
  DemoAccountRow,
  DemoBrowserPage,
  DemoCampaign,
  DemoIssueRow,
  DemoMailThread,
  DemoReceipt,
} from "@/lib/marketing/landing";
import { cn } from "cn";
import { useMemo, useState } from "react";

interface BrowserPageProps {
  page: DemoBrowserPage;
  path: string;
  goTo: (path: string) => void;
}

export function BrowserPage({ page, path, goTo }: BrowserPageProps) {
  switch (page.kind) {
    case "mail":
      return <MailPage page={page} path={path} goTo={goTo} />;
    case "calendar":
      return <CalendarPage page={page} path={path} goTo={goTo} />;
    case "accounts":
      return <AccountsPage page={page} path={path} goTo={goTo} />;
    case "github":
      return <GithubPage page={page} path={path} goTo={goTo} />;
    case "figma":
      return <FigmaPage page={page} path={path} goTo={goTo} />;
    case "expenses":
      return <ExpensesPage page={page} path={path} goTo={goTo} />;
    case "sheet":
      return <SheetPage page={page} path={path} goTo={goTo} />;
    case "ads":
      return <AdsPage page={page} path={path} goTo={goTo} />;
  }
}

function MailPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "mail" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const [threads, setThreads] = useState(page.threads);
  const [sentIds, setSentIds] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [compose, setCompose] = useState({ to: "", subject: "", body: "" });

  const folder = path === "mail:drafts" || path === "mail:starred" || path === "mail:sent"
    ? path.slice("mail:".length)
    : "inbox";
  const threadId = path.startsWith("mail:thread:") ? path.slice("mail:thread:".length) : null;
  const composing = path === "mail:compose";
  const selected = threads.find((thread) => thread.id === threadId);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return threads.filter((thread) => {
      if (folder === "starred" && !thread.starred) return false;
      if (folder === "drafts" && thread.state !== "held" && thread.state !== "draft") return false;
      if (folder === "sent" && !(sentIds.includes(thread.id) || thread.id.startsWith("sent-"))) {
        return false;
      }
      if (folder === "inbox" && (thread.state === "draft" || thread.id.startsWith("sent-"))) return false;
      if (!needle) return true;
      return `${thread.from} ${thread.subject} ${thread.snippet}`.toLowerCase().includes(needle);
    });
  }, [folder, query, sentIds, threads]);

  function handleStar(id: string) {
    setThreads((current) =>
      current.map((thread) =>
        thread.id === id ? { ...thread, starred: !thread.starred } : thread,
      ),
    );
  }

  function handleSendDraft(thread: DemoMailThread) {
    setSentIds((current) => (current.includes(thread.id) ? current : [...current, thread.id]));
    setThreads((current) =>
      current.map((item) =>
        item.id === thread.id
          ? { ...item, state: "read", draft: undefined, snippet: "Sent · " + (item.draft ?? "") }
          : item,
      ),
    );
  }

  if (composing) {
    return (
      <div className="flex h-full flex-col bg-white text-[#202124]">
        <MailChrome query={query} onQueryChange={setQuery} />
        <div className="flex items-center gap-2 border-b border-[#e8eaed] px-2 py-1.5">
          <button
            type="button"
            onClick={() => goTo("home")}
            className="text-[11px] text-[#1a73e8]"
          >
            ← Inbox
          </button>
          <p className="text-[12px] font-medium">New message</p>
        </div>
        <label className="sr-only" htmlFor="compose-to">To</label>
        <input
          id="compose-to"
          value={compose.to}
          onChange={(event) => setCompose((current) => ({ ...current, to: event.target.value }))}
          placeholder="To"
          className="border-b border-[#e8eaed] px-3 py-1.5 text-[12px] outline-none"
        />
        <label className="sr-only" htmlFor="compose-subject">Subject</label>
        <input
          id="compose-subject"
          value={compose.subject}
          onChange={(event) => setCompose((current) => ({ ...current, subject: event.target.value }))}
          placeholder="Subject"
          className="border-b border-[#e8eaed] px-3 py-1.5 text-[12px] outline-none"
        />
        <label className="sr-only" htmlFor="compose-body">Message</label>
        <textarea
          id="compose-body"
          value={compose.body}
          onChange={(event) => setCompose((current) => ({ ...current, body: event.target.value }))}
          placeholder="Write in their voice…"
          className="min-h-0 flex-1 resize-none px-3 py-2 text-[12px] outline-none"
        />
        <div className="flex items-center gap-2 border-t border-[#e8eaed] px-3 py-2">
          <button
            type="button"
            disabled={!compose.to.trim() || !compose.subject.trim()}
            onClick={() => {
              const id = `sent-${Date.now()}`;
              setSentIds((current) => [...current, id]);
              setThreads((current) => [
                {
                  id,
                  from: "You",
                  email: "you@elsewhere.dev",
                  subject: compose.subject,
                  snippet: compose.body,
                  body: compose.body,
                  time: "Now",
                  state: "read",
                },
                ...current,
              ]);
              setCompose({ to: "", subject: "", body: "" });
              goTo("mail:sent");
            }}
            className="rounded-full bg-[#0b57d0] px-3 py-1 text-[11px] font-medium text-white disabled:opacity-40"
          >
            Send
          </button>
        </div>
      </div>
    );
  }

  if (selected) {
    return (
      <div className="flex h-full flex-col bg-white text-[#202124]">
        <MailChrome query={query} onQueryChange={setQuery} />
        <div className="flex items-center gap-2 border-b border-[#e8eaed] px-2 py-1.5">
          <button
            type="button"
            onClick={() => goTo("home")}
            className="inline-flex size-6 items-center justify-center rounded-full text-[#5f6368] hover:bg-[#f1f3f4]"
            aria-label="Back to inbox"
          >
            ←
          </button>
          <p className="min-w-0 flex-1 truncate text-[12.5px] font-medium">{selected.subject}</p>
          <button
            type="button"
            onClick={() => handleStar(selected.id)}
            className={cn("text-[13px]", selected.starred ? "text-[#f4b400]" : "text-[#80868b]")}
            aria-label={selected.starred ? "Unstar" : "Star"}
            aria-pressed={Boolean(selected.starred)}
          >
            {selected.starred ? "★" : "☆"}
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-3 py-2.5">
          <div className="flex items-start gap-2">
            <span className="flex size-7 shrink-0 items-center justify-center rounded-full bg-[#d3e3fd] text-[11px] font-medium text-[#0b57d0]">
              {initials(selected.from)}
            </span>
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline justify-between gap-2">
                <p className="truncate text-[12px] font-medium">{selected.from}</p>
                <p className="shrink-0 text-[10px] text-[#80868b]">{selected.time}</p>
              </div>
              <p className="text-[10.5px] text-[#80868b]">to you · {selected.email}</p>
            </div>
          </div>
          <p className="mt-3 whitespace-pre-wrap text-[12px] leading-5 text-[#3c4043]">
            {selected.body}
          </p>
          {selected.draft ? (
            <div className="relative mt-3 rounded-lg border border-[#fde293] bg-[#fff8e1] px-2.5 py-2">
              <p className="text-[10px] font-medium text-[#b06000]">Draft · held for your read</p>
              <p className="mt-1 text-[12px] leading-5 text-[#3c4043]">{selected.draft}</p>
              <PagePointer />
            </div>
          ) : selected.state === "read" && selected.id === "nora" ? (
            <p className="mt-3 text-[11px] font-medium text-[#137333]">Sent.</p>
          ) : null}
        </div>
        {selected.draft ? (
          <div className="flex shrink-0 gap-2 border-t border-[#e8eaed] bg-white px-3 py-2">
            <button
              type="button"
              onClick={() => handleSendDraft(selected)}
              className="rounded-full bg-[#0b57d0] px-2.5 py-1 text-[11px] font-medium text-white"
            >
              Send
            </button>
            <button
              type="button"
              onClick={() => goTo("home")}
              className="rounded-full px-2.5 py-1 text-[11px] text-[#5f6368] hover:bg-black/5"
            >
              Keep parked
            </button>
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <div className="flex h-full bg-white text-[#202124]">
      <nav className="flex w-9 shrink-0 flex-col items-center gap-1 border-r border-[#e8eaed] bg-[#f8fafd] py-2" aria-label="Gmail folders">
        <button
          type="button"
          onClick={() => goTo("mail:compose")}
          className="mb-1 flex size-7 items-center justify-center rounded-2xl bg-[#c2e7ff] text-[#001d35] shadow-[0_1px_3px_rgba(0,0,0,0.15)]"
          aria-label="Compose"
        >
          ✎
        </button>
        <MailNavIcon label="Inbox" active={folder === "inbox"} onClick={() => goTo("home")} mark="—" />
        <MailNavIcon label="Starred" active={folder === "starred"} onClick={() => goTo("mail:starred")} mark="☆" />
        <MailNavIcon label="Drafts" active={folder === "drafts"} onClick={() => goTo("mail:drafts")} mark="▭" />
        <MailNavIcon label="Sent" active={folder === "sent"} onClick={() => goTo("mail:sent")} mark="↗" />
      </nav>
      <div className="flex min-w-0 flex-1 flex-col">
        <MailChrome query={query} onQueryChange={setQuery} />
        <div className="flex items-center justify-between border-b border-[#e8eaed] px-2 py-1">
          <p className="text-[11px] font-medium capitalize">{folder}</p>
          <p className="text-[10px] text-[#5f6368]">{page.draftCount} drafts parked</p>
        </div>
        <ul className="min-h-0 flex-1 overflow-y-auto">
          {visible.map((thread) => {
            const unread = thread.state === "held" || thread.state === "unread";
            return (
              <li key={thread.id} className="relative">
                <div
                  className={cn(
                    "flex items-start gap-1 border-b border-[#f1f3f4] px-1 py-1.5 hover:shadow-[inset_1px_0_0_#dadce0]",
                    unread ? "bg-white" : "bg-[#f2f6fc]",
                    thread.active && path === "home" && "bg-[#d3e3fd]",
                  )}
                >
                  <button
                    type="button"
                    onClick={() => handleStar(thread.id)}
                    className={cn(
                      "mt-0.5 size-5 shrink-0 text-[12px]",
                      thread.starred ? "text-[#f4b400]" : "text-[#dadce0] hover:text-[#f4b400]",
                    )}
                    aria-label={thread.starred ? "Unstar" : "Star"}
                    aria-pressed={Boolean(thread.starred)}
                  >
                    {thread.starred ? "★" : "☆"}
                  </button>
                  <button
                    type="button"
                    onClick={() => goTo(`mail:thread:${thread.id}`)}
                    className="min-w-0 flex-1 text-left"
                  >
                    <span className="flex items-baseline justify-between gap-2">
                      <span className={cn("truncate text-[11.5px]", unread ? "font-bold" : "font-medium text-[#3c4043]")}>
                        {thread.from}
                      </span>
                      <span className="shrink-0 text-[10px] text-[#80868b]">{thread.time}</span>
                    </span>
                    <span className={cn("block truncate text-[11px]", unread ? "font-semibold" : "text-[#5f6368]")}>
                      {thread.subject}
                    </span>
                    <span className="block truncate text-[10.5px] text-[#80868b]">{thread.snippet}</span>
                  </button>
                </div>
                {thread.active && path === "home" ? <PagePointer /> : null}
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}

function MailChrome({
  query,
  onQueryChange,
}: {
  query: string;
  onQueryChange: (value: string) => void;
}) {
  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-[#e8eaed] bg-[#f6f8fc] px-2 py-1">
      <GmailMark />
      <label className="flex min-w-0 flex-1 items-center gap-1.5 rounded-full bg-[#eaf1fb] px-2.5 py-1">
        <span className="text-[11px] text-[#5f6368]" aria-hidden>
          ⌕
        </span>
        <input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Search mail"
          className="w-full bg-transparent text-[11.5px] text-[#202124] outline-none placeholder:text-[#80868b]"
          aria-label="Search mail"
        />
      </label>
    </div>
  );
}

function MailNavIcon({
  label,
  active,
  onClick,
  mark,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  mark: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex size-7 items-center justify-center rounded-full text-[11px]",
        active ? "bg-[#d3e3fd] text-[#0b57d0]" : "text-[#444746] hover:bg-[#e8eaed]",
      )}
    >
      {mark}
    </button>
  );
}

function CalendarPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "calendar" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const selectedDay = path.startsWith("cal:day:")
    ? path.slice("cal:day:".length)
    : path.startsWith("cal:event:")
      ? page.events.find((event) => event.id === path.slice("cal:event:".length))?.date ?? page.selectedDay
      : page.selectedDay;
  const eventId = path.startsWith("cal:event:") ? path.slice("cal:event:".length) : null;
  const selectedEvent = page.events.find((event) => event.id === eventId);
  const dayEvents = page.events.filter((event) => event.date === selectedDay);
  const weekdays = ["S", "M", "T", "W", "T", "F", "S"];

  return (
    <div className="flex h-full flex-col bg-white text-[#202124]">
      <div className="flex items-center justify-between border-b border-[#e8eaed] px-2.5 py-1.5">
        <div>
          <p className="text-[12px] font-medium">{page.range}</p>
          <p className="text-[10px] text-[#5f6368]">Week · Google Calendar</p>
        </div>
        <button
          type="button"
          onClick={() => goTo(`cal:day:${page.today}`)}
          className="rounded-full border border-[#dadce0] px-2 py-0.5 text-[10.5px] font-medium text-[#1a73e8] hover:bg-[#f1f3f4]"
        >
          Today
        </button>
      </div>
      <div className="grid grid-cols-7 border-b border-[#e8eaed] px-1 py-1.5">
        {page.days.map((day, index) => {
          const isToday = day === page.today;
          const isSelected = day === selectedDay;
          const hasEvents = page.events.some((event) => event.date === day);
          return (
            <button
              key={day}
              type="button"
              onClick={() => goTo(`cal:day:${day}`)}
              className="flex flex-col items-center gap-0.5"
              aria-pressed={isSelected}
              aria-label={`September ${day}`}
            >
              <span className="text-[9px] text-[#70757a]">{weekdays[index]}</span>
              <span
                className={cn(
                  "flex size-6 items-center justify-center rounded-full text-[11px]",
                  isSelected && "bg-[#1a73e8] font-medium text-white",
                  !isSelected && isToday && "text-[#1a73e8] font-medium",
                  !isSelected && !isToday && "text-[#3c4043]",
                )}
              >
                {day}
              </span>
              <span
                className={cn(
                  "size-1 rounded-full",
                  hasEvents ? "bg-[#1a73e8]" : "bg-transparent",
                )}
                aria-hidden
              />
            </button>
          );
        })}
      </div>
      {selectedEvent ? (
        <div className="relative min-h-0 flex-1 overflow-y-auto px-3 py-3">
          <button
            type="button"
            onClick={() => goTo(`cal:day:${selectedEvent.date}`)}
            className="text-[11px] text-[#1a73e8]"
          >
            ← {selectedEvent.weekday}
          </button>
          <div className="mt-2 flex items-start gap-2">
            <span className="mt-1 size-3 rounded-sm" style={{ backgroundColor: selectedEvent.color }} aria-hidden />
            <div>
              <p className="text-[15px] font-medium">{selectedEvent.title}</p>
              <p className="mt-1 text-[12px] text-[#5f6368]">
                {selectedEvent.weekday}, Sep {selectedEvent.date} · {selectedEvent.time}
              </p>
              <p className="mt-2 text-[12px] leading-5 text-[#3c4043]">{selectedEvent.detail}</p>
            </div>
          </div>
          {selectedEvent.active ? <PagePointer /> : null}
        </div>
      ) : (
        <ul className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
          {dayEvents.length === 0 ? (
            <li className="px-1 py-3 text-[12px] text-[#80868b]">Nothing on this day.</li>
          ) : (
            dayEvents.map((event) => (
              <li key={event.id} className="relative">
                <button
                  type="button"
                  onClick={() => goTo(`cal:event:${event.id}`)}
                  className="mb-1.5 flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left hover:bg-[#f1f3f4]"
                >
                  <span
                    className="mt-0.5 h-8 w-1 rounded-full"
                    style={{ backgroundColor: event.color }}
                    aria-hidden
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[10px] text-[#5f6368]">{event.time}</span>
                    <span className="block truncate text-[12.5px] font-medium">{event.title}</span>
                    <span className="block truncate text-[10.5px] text-[#80868b]">{event.detail}</span>
                  </span>
                </button>
                {event.active && path === "home" ? <PagePointer /> : null}
              </li>
            ))
          )}
        </ul>
      )}
    </div>
  );
}

function AccountsPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "accounts" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const selectedName = path.startsWith("account:") ? path.slice("account:".length) : null;
  const selected = page.rows.find((row) => row.name === selectedName);

  if (selected) {
    return (
      <AccountDetail row={selected} onBack={() => goTo("home")} />
    );
  }

  return (
    <div className="flex h-full flex-col bg-[#fafafa] text-[#202124]">
      <div className="border-b border-[#eee] bg-white px-2.5 py-1.5">
        <p className="text-[10px] tracking-wide text-[#80868b] uppercase">Elsewhere inbound</p>
        <p className="text-[12px] font-medium">{page.heading}</p>
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {page.rows.map((row) => (
          <li key={row.name} className="relative">
            <button
              type="button"
              onClick={() => goTo(`account:${row.name}`)}
              className={cn(
                "flex w-full items-center gap-2 border-b border-[#f0f0f0] bg-white px-2.5 py-2 text-left hover:bg-[#f5f3ff]",
                row.active && path === "home" && "bg-[#f5f3ff]",
              )}
            >
              <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-[#f5f3ff] text-[10px] font-medium text-primary">
                {initials(row.name)}
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline justify-between gap-2">
                  <span className="truncate text-[11.5px] font-medium">{row.name}</span>
                  <span className="font-mono text-[10.5px] text-[#5f6368]">{row.score}</span>
                </span>
                <span className="mt-0.5 block truncate text-[10px] text-[#80868b]">
                  {row.domain} · {row.note}
                </span>
              </span>
              <StatusPill
                label={row.state}
                tone={row.state === "Skip" ? "muted" : row.state === "Draft" ? "good" : "hold"}
              />
            </button>
            {row.active && path === "home" ? <PagePointer /> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function AccountDetail({
  row,
  onBack,
}: {
  row: DemoAccountRow;
  onBack: () => void;
}) {
  return (
    <div className="relative flex h-full flex-col bg-white px-3 py-2.5 text-[#202124]">
      <button type="button" onClick={onBack} className="self-start text-[11px] text-primary">
        ← Inbound
      </button>
      <div className="mt-2 flex items-start justify-between gap-2">
        <div>
          <p className="text-[15px] font-medium">{row.name}</p>
          <p className="text-[11px] text-[#80868b]">{row.domain}</p>
        </div>
        <StatusPill
          label={row.state}
          tone={row.state === "Skip" ? "muted" : row.state === "Draft" ? "good" : "hold"}
        />
      </div>
      <p className="mt-2 font-mono text-[12px] text-[#3c4043]">Intent {row.score}</p>
      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-[#eee]">
        <span
          className="block h-full rounded-full bg-primary"
          style={{ width: `${Number(row.score) * 100}%` }}
        />
      </div>
      <p className="mt-3 text-[10px] tracking-wide text-[#80868b] uppercase">Signals</p>
      <ul className="mt-1 space-y-1">
        {row.signals.map((signal) => (
          <li key={signal} className="text-[12px] text-[#3c4043]">
            · {signal}
          </li>
        ))}
      </ul>
      {row.active ? <PagePointer /> : null}
    </div>
  );
}

function GithubPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "github" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const [filter, setFilter] = useState<"open" | "closed">("open");
  const issueNumber = path.startsWith("issue:") ? Number(path.slice("issue:".length)) : null;
  const selected = page.issues.find((issue) => issue.number === issueNumber);
  const visible = page.issues.filter((issue) =>
    filter === "open" ? issue.state === "reproduced" : issue.state === "blocked",
  );

  if (selected) {
    return <IssueDetail issue={selected} repo={page.repo} onBack={() => goTo("home")} />;
  }

  return (
    <div className="flex h-full flex-col bg-white text-[#1f2328]">
      <div className="border-b border-[#d0d7de] bg-[#f6f8fa] px-2.5 py-1.5">
        <p className="font-mono text-[10.5px] text-[#656d76]">{page.repo}</p>
        <div className="mt-1 flex gap-1 text-[11px]">
          <span className="rounded-md px-1.5 py-0.5 text-[#656d76]">Code</span>
          <span className="rounded-md bg-white px-1.5 py-0.5 font-medium ring-1 ring-[#d0d7de]">
            Issues
          </span>
          <span className="rounded-md px-1.5 py-0.5 text-[#656d76]">PRs</span>
        </div>
      </div>
      <div className="flex gap-2 border-b border-[#d0d7de] px-2.5 py-1.5 text-[11px]">
        <button
          type="button"
          onClick={() => setFilter("open")}
          className={cn("font-medium", filter === "open" ? "text-[#1f2328]" : "text-[#656d76]")}
        >
          {page.issues.filter((issue) => issue.state === "reproduced").length} Open
        </button>
        <button
          type="button"
          onClick={() => setFilter("closed")}
          className={cn(filter === "closed" ? "font-medium text-[#1f2328]" : "text-[#656d76]")}
        >
          {page.issues.filter((issue) => issue.state === "blocked").length} Blocked
        </button>
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {visible.map((issue) => (
          <li key={issue.number} className="relative">
            <button
              type="button"
              onClick={() => goTo(`issue:${issue.number}`)}
              className={cn(
                "flex w-full items-start gap-2 border-b border-[#d8dee4] px-2.5 py-2 text-left hover:bg-[#f6f8fa]",
                issue.active && path === "home" && "bg-[#ddf4ff]",
              )}
            >
              <span
                className={cn(
                  "mt-1 size-2 shrink-0 rounded-full",
                  issue.state === "reproduced" ? "bg-[#1a7f37]" : "bg-[#9a6700]",
                )}
                aria-hidden
              />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[12px] font-semibold">{issue.title}</span>
                <span className="mt-0.5 flex flex-wrap gap-1">
                  {issue.labels.map((label) => (
                    <span
                      key={label}
                      className="rounded-full bg-[#ddf4ff] px-1.5 py-px text-[9.5px] text-[#0969da]"
                    >
                      {label}
                    </span>
                  ))}
                </span>
                <span className="mt-0.5 block text-[10px] text-[#656d76]">
                  #{issue.number} · {issue.comments} {issue.comments === 1 ? "comment" : "comments"}
                </span>
              </span>
            </button>
            {issue.active && path === "home" ? <PagePointer /> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function IssueDetail({
  issue,
  repo,
  onBack,
}: {
  issue: DemoIssueRow;
  repo: string;
  onBack: () => void;
}) {
  const [comment, setComment] = useState("");
  const [posted, setPosted] = useState<string[]>([]);

  return (
    <div className="relative flex h-full flex-col bg-white text-[#1f2328]">
      <div className="border-b border-[#d0d7de] px-2.5 py-1.5">
        <button type="button" onClick={onBack} className="text-[11px] text-[#0969da]">
          ← {repo}
        </button>
        <p className="mt-1 text-[13.5px] font-semibold">
          {issue.title}{" "}
          <span className="font-normal text-[#656d76]">#{issue.number}</span>
        </p>
        <span
          className={cn(
            "mt-1 inline-flex rounded-full px-2 py-0.5 text-[10px] font-medium text-white",
            issue.state === "reproduced" ? "bg-[#1a7f37]" : "bg-[#9a6700]",
          )}
        >
          {issue.state === "reproduced" ? "Open" : "Blocked"}
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2.5 py-2">
        <p className="text-[12px] leading-5">{issue.body}</p>
        {posted.map((item) => (
          <p key={item} className="mt-2 rounded-md border border-[#d0d7de] bg-[#f6f8fa] px-2 py-1.5 text-[11.5px]">
            {item}
          </p>
        ))}
        <form
          className="mt-3"
          onSubmit={(event) => {
            event.preventDefault();
            if (!comment.trim()) return;
            setPosted((current) => [...current, comment.trim()]);
            setComment("");
          }}
        >
          <label className="sr-only" htmlFor={`issue-comment-${issue.number}`}>
            Comment
          </label>
          <textarea
            id={`issue-comment-${issue.number}`}
            value={comment}
            onChange={(event) => setComment(event.target.value)}
            placeholder="Leave a comment"
            rows={2}
            className="w-full rounded-md border border-[#d0d7de] px-2 py-1.5 text-[11.5px] outline-none focus:border-[#0969da]"
          />
          <button
            type="submit"
            disabled={!comment.trim()}
            className="mt-1 rounded-md bg-[#1a7f37] px-2 py-1 text-[11px] font-medium text-white disabled:opacity-40"
          >
            Comment
          </button>
        </form>
      </div>
      {issue.active ? <PagePointer /> : null}
    </div>
  );
}

function FigmaPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "figma" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const selectedLabel = path.startsWith("frame:")
    ? path.slice("frame:".length)
    : page.frames.find((frame) => frame.active)?.label ?? page.frames[0]?.label;
  const selected = page.frames.find((frame) => frame.label === selectedLabel) ?? page.frames[0];

  return (
    <div className="flex h-full flex-col bg-[#1e1e1e] text-white">
      <div className="flex items-center gap-1.5 border-b border-white/8 px-2 py-1.5 text-white/55">
        {["→", "□", "T", "●"].map((tool) => (
          <span
            key={tool}
            className="flex size-5 items-center justify-center rounded-[4px] text-[10px] hover:bg-white/8"
            aria-hidden
          >
            {tool}
          </span>
        ))}
        <p className="ml-1 truncate text-[10.5px] text-white/45">{page.file}</p>
      </div>
      <div className="flex min-h-0 flex-1 items-end justify-center gap-2 overflow-x-auto px-3 pb-2 pt-3">
        {page.frames.map((frame) => {
          const active = frame.label === selected?.label;
          return (
            <button
              key={frame.label}
              type="button"
              onClick={() => goTo(`frame:${frame.label}`)}
              className={cn(
                "relative flex h-[78%] w-[31%] min-w-[86px] flex-col overflow-hidden rounded-[10px] bg-white text-left text-[#202124] shadow-[0_10px_24px_-12px_rgba(0,0,0,0.8)]",
                active && "ring-2 ring-[#0d99ff] ring-offset-2 ring-offset-[#1e1e1e]",
              )}
              aria-pressed={active}
            >
              <span className="bg-[#f6f8fc] px-1.5 py-1 text-[8px] font-medium tracking-wide text-[#80868b] uppercase">
                {frame.size} · {frame.label}
              </span>
              <span className="flex min-h-0 flex-1 flex-col gap-px px-1.5 py-1">
                {frame.rows.map((row) => (
                  <span
                    key={row}
                    className={cn(
                      "rounded-[3px] px-1 py-0.5 text-[8.5px]",
                      frame.label === "Danger" && row.startsWith("Delete")
                        ? "bg-[#fce8e6] text-[#c5221f]"
                        : "bg-[#f1f3f4] text-[#3c4043]",
                    )}
                  >
                    {row}
                  </span>
                ))}
                <span className="mt-auto rounded-[3px] bg-[#0b57d0] py-0.5 text-center text-[8px] text-white">
                  Save
                </span>
              </span>
              {active ? (
                <>
                  <span className="absolute -top-1 -left-1 size-1.5 bg-[#0d99ff]" aria-hidden />
                  <span className="absolute -top-1 -right-1 size-1.5 bg-[#0d99ff]" aria-hidden />
                  <span className="absolute -bottom-1 -left-1 size-1.5 bg-[#0d99ff]" aria-hidden />
                  <span className="absolute -right-1 -bottom-1 size-1.5 bg-[#0d99ff]" aria-hidden />
                </>
              ) : null}
              {frame.active && path === "home" ? <PagePointer /> : null}
            </button>
          );
        })}
      </div>
      {selected ? (
        <div className="border-t border-white/8 px-3 py-1.5">
          <p className="text-[10.5px] text-white/80">
            {selected.label} · {selected.note}
          </p>
        </div>
      ) : null}
    </div>
  );
}

function ExpensesPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "expenses" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const [receipts, setReceipts] = useState(page.receipts);
  const merchant = path.startsWith("receipt:") ? path.slice("receipt:".length) : null;
  const selected = receipts.find((receipt) => receipt.merchant === merchant);
  const total = receipts.reduce((sum, receipt) => sum + Number(receipt.amount.replace(/[^0-9.]/g, "")), 0);

  function handleCategorize(target: DemoReceipt, category: string) {
    setReceipts((current) =>
      current.map((receipt) =>
        receipt.merchant === target.merchant
          ? { ...receipt, category, state: "Matched", note: undefined }
          : receipt,
      ),
    );
  }

  if (selected) {
    return (
      <div className="relative flex h-full flex-col bg-white px-3 py-2.5 text-[#202124]">
        <button type="button" onClick={() => goTo("home")} className="self-start text-[11px] text-[#0d9488]">
          ← {page.report}
        </button>
        <p className="mt-2 text-[15px] font-medium">{selected.merchant}</p>
        <p className="font-mono text-[18px] tabular-nums">{selected.amount}</p>
        <p className="text-[11px] text-[#80868b]">
          {selected.date} · {selected.category}
        </p>
        {selected.note ? <p className="mt-2 text-[12px] text-[#92400e]">{selected.note}</p> : null}
        {selected.state === "Flagged" ? (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {["Lodging", "Meals", "Travel"].map((category) => (
              <button
                key={category}
                type="button"
                onClick={() => handleCategorize(selected, category)}
                className="rounded-full border border-[#dadce0] px-2 py-1 text-[11px] hover:bg-[#f1f3f4]"
              >
                {category}
              </button>
            ))}
          </div>
        ) : (
          <p className="mt-3 text-[11px] font-medium text-[#137333]">Matched · {selected.category}</p>
        )}
        {selected.active ? <PagePointer /> : null}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-white text-[#202124]">
      <div className="border-b border-[#e8eaed] px-2.5 py-1.5">
        <p className="text-[12px] font-medium">{page.report}</p>
        <p className="text-[10.5px] text-[#5f6368]">
          ${total.toFixed(2)} · {receipts.length} receipts
        </p>
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {receipts.map((receipt) => (
          <li key={receipt.merchant} className="relative">
            <button
              type="button"
              onClick={() => goTo(`receipt:${receipt.merchant}`)}
              className={cn(
                "flex w-full items-center gap-2 border-b border-[#f1f3f4] px-2.5 py-2 text-left hover:bg-[#f8fafc]",
                receipt.active && path === "home" && "bg-[#fff7ed]",
              )}
            >
              <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-[#ecfdf5] text-[10px] font-medium text-[#0f766e]">
                {receipt.merchant.slice(0, 1)}
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline justify-between gap-2">
                  <span className="truncate text-[11.5px] font-medium">{receipt.merchant}</span>
                  <span className="font-mono text-[11px] tabular-nums">{receipt.amount}</span>
                </span>
                <span className="text-[10px] text-[#80868b]">
                  {receipt.date} · {receipt.category}
                </span>
              </span>
              <StatusPill
                label={receipt.state}
                tone={receipt.state === "Flagged" ? "warn" : "good"}
              />
            </button>
            {receipt.active && path === "home" ? <PagePointer /> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function SheetPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "sheet" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const defaultCell = page.rows.findIndex((row) => row.active);
  const parsed = path.startsWith("cell:") ? path.slice("cell:".length).split(":") : null;
  const activeRow = parsed ? Number(parsed[0]) : defaultCell < 0 ? 0 : defaultCell;
  const activeCol = parsed ? Number(parsed[1]) : 0;
  const active = page.rows[activeRow];
  const formula = active
    ? [active.name, active.score, active.status][activeCol] ?? active.name
    : "";

  return (
    <div className="flex h-full flex-col bg-white text-[#202124]">
      <div className="flex items-center gap-2 bg-[#188038] px-2 py-1.5 text-white">
        <span className="size-3.5 rounded-[3px] bg-white/90" aria-hidden />
        <p className="truncate text-[11px] font-medium">{page.title}</p>
      </div>
      <div className="flex items-center gap-1.5 border-b border-[#e8eaed] bg-[#f8f9fa] px-2 py-1">
        <span className="text-[11px] text-[#80868b]">fx</span>
        <p className="truncate font-mono text-[11px]">{formula}</p>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full border-collapse text-left">
          <caption className="sr-only">{page.title}</caption>
          <thead>
            <tr className="bg-[#f8f9fa]">
              <th className="w-6 border-b border-r border-[#e8eaed] px-1 py-1 text-center text-[9px] font-normal text-[#80868b]">
                #
              </th>
              {page.columns.map((column, index) => (
                <th
                  key={column}
                  className={cn(
                    "border-b border-r border-[#e8eaed] px-1.5 py-1 text-[10px] font-medium",
                    activeCol === index && "bg-[#d2e3fc]",
                  )}
                >
                  <span className="text-[#80868b]">{String.fromCharCode(65 + index)} </span>
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {page.rows.map((row, rowIndex) => {
              const values = [row.name, row.score, row.status];
              return (
                <tr key={row.name} className={row.active && path === "home" ? "bg-[#e8f0fe]" : "bg-white"}>
                  <td className="border-b border-r border-[#e8eaed] px-1 py-1 text-center text-[10px] text-[#80868b]">
                    {rowIndex + 1}
                  </td>
                  {values.map((value, colIndex) => {
                    const isActive = activeRow === rowIndex && activeCol === colIndex;
                    return (
                      <td
                        key={`${row.name}-${colIndex}`}
                        className="relative border-b border-r border-[#e8eaed] p-0"
                      >
                        <button
                          type="button"
                          onClick={() => goTo(`cell:${rowIndex}:${colIndex}`)}
                          className={cn(
                            "block w-full truncate px-1.5 py-1 text-left text-[11px]",
                            colIndex === 1 && "font-mono tabular-nums",
                            colIndex === 0 && "font-medium",
                            isActive && "ring-2 ring-inset ring-[#1a73e8]",
                          )}
                        >
                          {colIndex === 2 ? (
                            <StatusPill label={value} tone={value === "Short" ? "good" : "muted"} />
                          ) : (
                            value
                          )}
                        </button>
                        {row.active && colIndex === 0 && path === "home" ? <PagePointer /> : null}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function AdsPage({
  page,
  path,
  goTo,
}: {
  page: Extract<DemoBrowserPage, { kind: "ads" }>;
  path: string;
  goTo: (path: string) => void;
}) {
  const [campaigns, setCampaigns] = useState(page.campaigns);
  const selectedName = path.startsWith("campaign:") ? path.slice("campaign:".length) : null;
  const selected = campaigns.find((campaign) => campaign.name === selectedName);

  function handleToggle(target: DemoCampaign) {
    if (target.state === "Held") return;
    setCampaigns((current) =>
      current.map((campaign) =>
        campaign.name === target.name
          ? { ...campaign, state: campaign.state === "Active" ? "Paused" : "Active" }
          : campaign,
      ),
    );
  }

  if (selected) {
    return (
      <div className="relative flex h-full flex-col bg-white px-3 py-2.5 text-[#202124]">
        <button type="button" onClick={() => goTo("home")} className="self-start text-[11px] text-[#1a73e8]">
          ← Campaigns
        </button>
        <p className="mt-2 text-[14px] font-medium">{selected.name}</p>
        <div className="mt-2 grid grid-cols-3 gap-2 text-center">
          <Metric label="Spend" value={selected.spend} />
          <Metric label="Clicks" value={selected.clicks} />
          <Metric label="Conv" value={selected.conv} />
        </div>
        <div className="mt-3 flex items-center gap-2">
          <StatusPill
            label={selected.state}
            tone={
              selected.state === "Active" ? "good" : selected.state === "Paused" ? "bad" : "hold"
            }
          />
          {selected.state === "Held" ? (
            <p className="text-[11px] text-[#5f6368]">Left untouched on purpose.</p>
          ) : (
            <button
              type="button"
              onClick={() => handleToggle(selected)}
              className="rounded-full border border-[#dadce0] px-2 py-1 text-[11px] hover:bg-[#f1f3f4]"
            >
              {selected.state === "Active" ? "Pause" : "Enable"}
            </button>
          )}
        </div>
        {selected.active ? <PagePointer /> : null}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-white text-[#202124]">
      <div className="border-b border-[#e8eaed] px-2.5 py-1.5">
        <p className="text-[12px] font-medium">{page.heading}</p>
        <div className="mt-1.5 flex h-8 items-end gap-1">
          {[40, 55, 48, 72, 64, 88, 70].map((height, index) => (
            <span
              key={index}
              className="flex-1 rounded-t-sm bg-[#d2e3fc]"
              style={{ height: `${height}%` }}
              aria-hidden
            />
          ))}
        </div>
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {campaigns.map((campaign) => (
          <li key={campaign.name} className="relative">
            <button
              type="button"
              onClick={() => goTo(`campaign:${campaign.name}`)}
              className={cn(
                "flex w-full items-center gap-2 border-b border-[#f1f3f4] px-2.5 py-2 text-left hover:bg-[#f8fafc]",
                campaign.active && path === "home" && "bg-[#e8f0fe]",
              )}
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[11.5px] font-medium">{campaign.name}</span>
                <span className="text-[10px] text-[#80868b]">
                  {campaign.spend} · {campaign.clicks} clicks
                </span>
              </span>
              <span
                className={cn(
                  "font-mono text-[10.5px] tabular-nums",
                  campaign.state === "Active" ? "text-[#137333]" : "text-[#80868b]",
                )}
              >
                {campaign.change}
              </span>
              <StatusPill
                label={campaign.state}
                tone={
                  campaign.state === "Active"
                    ? "good"
                    : campaign.state === "Paused"
                      ? "bad"
                      : "hold"
                }
              />
            </button>
            {campaign.active && path === "home" ? <PagePointer /> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-[#f8fafc] py-2">
      <p className="text-[10px] text-[#80868b]">{label}</p>
      <p className="font-mono text-[13px]">{value}</p>
    </div>
  );
}

function StatusPill({
  label,
  tone,
}: {
  label: string;
  tone: "good" | "warn" | "bad" | "hold" | "muted";
}) {
  return (
    <span
      className={cn(
        "shrink-0 rounded-full px-1.5 py-0.5 text-[9.5px] font-medium",
        tone === "good" && "bg-[#e6f4ea] text-[#137333]",
        tone === "warn" && "bg-[#fef3c7] text-[#92400e]",
        tone === "bad" && "bg-[#fce8e6] text-[#c5221f]",
        tone === "hold" && "bg-[#f5f3ff] text-primary",
        tone === "muted" && "bg-[#f1f3f4] text-[#5f6368]",
      )}
    >
      {label}
    </span>
  );
}

function PagePointer() {
  return (
    <span
      className="pointer-events-none absolute top-1/2 right-1.5 z-10 -translate-y-1/3"
      aria-hidden
    >
      <span className="absolute top-0.5 left-0.5 size-2.5 animate-ping rounded-full bg-primary/40 motion-reduce:hidden" />
      <svg
        width="13"
        height="17"
        viewBox="0 0 13 17"
        className="relative drop-shadow-[0_1px_1px_rgba(0,0,0,0.45)]"
      >
        <path
          d="M1.1 1.1v13.2l3.6-3.5 2.3 5.2 2.05-.95-2.4-5.15H12Z"
          fill="#fff"
          stroke="#111"
          strokeLinejoin="round"
          strokeWidth="1.15"
        />
      </svg>
    </span>
  );
}

function GmailMark() {
  return (
    <svg viewBox="0 0 24 24" className="size-4 shrink-0" aria-hidden>
      <path fill="#4285F4" d="M1.5 6.75v10.5A2.25 2.25 0 0 0 3.75 19.5h2.1V9.18L12 13.5l6.15-4.32V19.5h2.1a2.25 2.25 0 0 0 2.25-2.25V6.75c0-.9-.54-1.71-1.35-2.04L12 11.04 2.85 4.71A2.24 2.24 0 0 0 1.5 6.75Z" />
      <path fill="#34A853" d="M20.25 19.5h-2.1V9.18L12 13.5v6h8.25Z" opacity=".0" />
      <path fill="#EA4335" d="M3.75 4.5h16.5c.27 0 .54.05.78.15L12 11.04 2.97 4.65c.24-.1.51-.15.78-.15Z" />
      <path fill="#FBBC04" d="M21.03 4.65A2.24 2.24 0 0 0 20.25 4.5h-2.1v4.68l2.88-2.02V6.75c0-.78-.4-1.5-1-1.9Z" />
    </svg>
  );
}

function initials(name: string) {
  return name
    .split(" ")
    .slice(0, 2)
    .map((part) => part[0])
    .join("")
    .toUpperCase();
}
