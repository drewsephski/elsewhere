"use client";

import {
  ArrowUp,
  CalendarClock,
  Check,
  ChevronDown,
  Monitor,
  Plus,
  Search,
  X,
} from "@/components/icons/lucide";
import { DemoBrowser } from "@/components/marketing/demo-browser";
import { appRoutes } from "@/lib/app-routes";
import { DEMO_BOTS, type DemoBot, type DemoMessage } from "@/lib/marketing/landing";
import { cn } from "cn";
import { useEffect, useMemo, useRef, useState } from "react";

interface ProductDemoProps {
  onGetStarted: () => void;
}

export function ProductDemo({ onGetStarted }: ProductDemoProps) {
  const [selectedId, setSelectedId] = useState(DEMO_BOTS[0]?.id ?? "writer");
  const [query, setQuery] = useState("");
  const [computerOpen, setComputerOpen] = useState(false);
  const [botsOpen, setBotsOpen] = useState(false);
  const [activeRoutine, setActiveRoutine] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [extraMessages, setExtraMessages] = useState<Record<string, DemoMessage[]>>({});
  const threadRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const media = window.matchMedia("(min-width: 1024px)");
    function handleChange() {
      setComputerOpen(media.matches);
    }
    handleChange();
    media.addEventListener("change", handleChange);
    return () => media.removeEventListener("change", handleChange);
  }, []);

  const filteredBots = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return DEMO_BOTS;
    return DEMO_BOTS.filter((bot) => {
      return (
        bot.name.toLowerCase().includes(needle) ||
        bot.preview.toLowerCase().includes(needle)
      );
    });
  }, [query]);

  const selected =
    DEMO_BOTS.find((bot) => bot.id === selectedId) ?? filteredBots[0] ?? DEMO_BOTS[0];

  const thread = selected
    ? [...selected.messages, ...(extraMessages[selected.id] ?? [])]
    : [];

  useEffect(() => {
    const node = threadRef.current;
    if (!node) return;
    node.scrollTo({ top: node.scrollHeight, behavior: "smooth" });
  }, [thread.length, selectedId]);

  if (!selected) return null;

  function handleSelect(bot: DemoBot) {
    setSelectedId(bot.id);
    setDraft("");
    setActiveRoutine(null);
    setBotsOpen(false);
  }

  function handleSend() {
    const text = draft.trim();
    if (!text || !selected) return;
    const userMessage: DemoMessage = {
      id: `${selected.id}-u-${Date.now()}`,
      role: "user",
      text,
    };
    setExtraMessages((current) => ({
      ...current,
      [selected.id]: [...(current[selected.id] ?? []), userMessage],
    }));
    setDraft("");
    window.setTimeout(() => {
      const reply: DemoMessage = {
        id: `${selected.id}-a-${Date.now()}`,
        role: "assistant",
        text: "on it. i’ll use this computer and come back if i need an approval.",
      };
      setExtraMessages((current) => ({
        ...current,
        [selected.id]: [...(current[selected.id] ?? []), reply],
      }));
    }, 700);
  }

  return (
    <section id="demo" className="landing-shell mt-10 md:mt-14">
      <div className="dark relative flex h-[min(78dvh,680px)] min-h-[520px] flex-col overflow-hidden rounded-[24px] border border-white/8 bg-[#0c0c0c] shadow-[0_24px_80px_-24px_rgba(43,39,53,0.55)]">
        <div className="flex shrink-0 items-center gap-2 border-b border-white/8 px-4 py-3">
          <span className="size-2.5 rounded-full bg-[#ff5f57]" aria-hidden />
          <span className="size-2.5 rounded-full bg-[#febc2e]" aria-hidden />
          <span className="size-2.5 rounded-full bg-[#28c840]" aria-hidden />
          <span className="ml-2 text-[12px] text-white/35">Elsewhere</span>
        </div>

        <div className="relative flex min-h-0 flex-1">
          <aside className="hidden min-h-0 w-[220px] shrink-0 flex-col border-r border-white/8 sm:flex lg:w-[248px]">
            <BotList
              bots={filteredBots}
              selectedId={selected.id}
              query={query}
              onQueryChange={setQuery}
              onSelect={handleSelect}
              onNewBot={onGetStarted}
            />
          </aside>

          <div className="flex min-w-0 flex-1 flex-col bg-[#111111]">
            <div className="flex shrink-0 items-center justify-between gap-3 border-b border-white/8 px-3 py-2.5 sm:px-4 sm:py-3">
              <button
                type="button"
                className="flex min-w-0 items-center gap-2.5 rounded-lg px-1 py-0.5 text-left sm:pointer-events-none sm:px-0"
                onClick={() => setBotsOpen(true)}
                aria-label="Choose a bot"
              >
                <DemoBotAvatar name={selected.name} image={selected.image} />
                <span className="min-w-0">
                  <span className="flex items-center gap-1">
                    <span className="truncate text-[15px] font-medium text-white">
                      {selected.name}
                    </span>
                    <ChevronDown className="size-3.5 text-white/45 sm:hidden" aria-hidden />
                  </span>
                  <span className="block truncate text-[11px] text-white/40 sm:hidden">
                    {selected.preview}
                  </span>
                </span>
              </button>
              <button
                type="button"
                aria-pressed={computerOpen}
                aria-label="Toggle computer panel"
                onClick={() => setComputerOpen((open) => !open)}
                className={cn(
                  "inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-1.5 text-[12px] transition-colors",
                  computerOpen
                    ? "bg-white/10 text-white"
                    : "text-white/55 hover:bg-white/8 hover:text-white",
                )}
              >
                <Monitor className="size-3.5" aria-hidden />
                <span>Computer</span>
              </button>
            </div>

            <div
              className="flex shrink-0 gap-2 overflow-x-auto border-b border-white/8 px-3 py-2 sm:hidden"
              aria-label="Switch bots"
            >
              {DEMO_BOTS.map((bot) => {
                const active = bot.id === selected.id;
                return (
                  <button
                    key={bot.id}
                    type="button"
                    onClick={() => handleSelect(bot)}
                    className={cn(
                      "flex shrink-0 items-center gap-2 rounded-full border px-2 py-1 transition-colors",
                      active
                        ? "border-white/20 bg-white/10"
                        : "border-white/8 bg-white/[0.03] hover:bg-white/8",
                    )}
                  >
                    <DemoBotAvatar name={bot.name} image={bot.image} size="xs" />
                    <span className="text-[12px] font-medium text-white">
                      {bot.name.split(" ")[0]}
                    </span>
                  </button>
                );
              })}
            </div>

            <div ref={threadRef} className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-4">
              {thread.map((message) => (
                <DemoBubble key={message.id} message={message} bot={selected} />
              ))}
            </div>

            <form
              className="shrink-0 border-t border-white/8 p-3"
              onSubmit={(event) => {
                event.preventDefault();
                handleSend();
              }}
            >
              <div className="flex items-end gap-2 rounded-full border border-white/10 bg-[#1a1a1a] px-3 py-2">
                <button
                  type="button"
                  onClick={onGetStarted}
                  className="mb-0.5 inline-flex size-7 items-center justify-center rounded-full text-white/45 hover:text-white"
                  aria-label="Add"
                >
                  <Plus className="size-4" aria-hidden />
                </button>
                <label className="sr-only" htmlFor="demo-composer">
                  Message {selected.name}
                </label>
                <textarea
                  id="demo-composer"
                  rows={1}
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey) {
                      event.preventDefault();
                      handleSend();
                    }
                  }}
                  placeholder={`Message ${selected.name}`}
                  className="max-h-24 min-h-[28px] flex-1 resize-none bg-transparent py-1 text-[14px] text-white outline-none placeholder:text-white/35"
                />
                <button
                  type="submit"
                  className="mb-0.5 inline-flex size-8 items-center justify-center rounded-full bg-white text-black transition-opacity disabled:opacity-40"
                  aria-label="Send"
                  disabled={!draft.trim()}
                >
                  <ArrowUp className="size-4" aria-hidden />
                </button>
              </div>
            </form>
          </div>

          {computerOpen ? (
            <>
              <button
                type="button"
                className="absolute inset-0 z-20 bg-black/45 lg:hidden"
                aria-label="Close computer panel"
                onClick={() => setComputerOpen(false)}
              />
              <ComputerPanel
                bot={selected}
                activeRoutine={activeRoutine}
                onSelectRoutine={setActiveRoutine}
                onNewRoutine={onGetStarted}
                onClose={() => setComputerOpen(false)}
              />
            </>
          ) : null}

          {botsOpen ? (
            <div className="absolute inset-0 z-30 flex flex-col bg-[#0c0c0c] sm:hidden">
              <div className="flex items-center justify-between border-b border-white/8 px-3 py-3">
                <p className="text-[15px] font-medium text-white">Bots</p>
                <button
                  type="button"
                  onClick={() => setBotsOpen(false)}
                  className="inline-flex size-8 items-center justify-center rounded-md text-white/55 hover:bg-white/8 hover:text-white"
                  aria-label="Close bot list"
                >
                  <X className="size-4" aria-hidden />
                </button>
              </div>
              <BotList
                bots={filteredBots}
                selectedId={selected.id}
                query={query}
                onQueryChange={setQuery}
                onSelect={handleSelect}
                onNewBot={onGetStarted}
              />
            </div>
          ) : null}
        </div>
      </div>
      <p className="mt-4 text-center text-[13.5px] text-muted-foreground">
        Live demo. Pick a bot, open its computer, click around the browser, or start a new chat.
      </p>
    </section>
  );
}

function DemoBotAvatar({
  name,
  image,
  size = "sm",
}: {
  name: string;
  image: string;
  size?: "xs" | "sm";
}) {
  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-end justify-center",
        size === "xs" ? "h-6 w-6" : "h-9 w-9",
      )}
      role="img"
      aria-label={name}
    >
      <img
        src={image}
        alt=""
        width={280}
        height={320}
        decoding="async"
        draggable={false}
        className="max-h-full w-auto max-w-full object-contain object-bottom"
        aria-hidden
      />
    </span>
  );
}

function BotList({
  bots,
  selectedId,
  query,
  onQueryChange,
  onSelect,
  onNewBot,
}: {
  bots: DemoBot[];
  selectedId: string;
  query: string;
  onQueryChange: (value: string) => void;
  onSelect: (bot: DemoBot) => void;
  onNewBot: () => void;
}) {
  return (
    <>
      <div className="flex items-center gap-2 px-3 py-3">
        <p className="flex-1 text-[13px] font-medium text-white/70">Bots</p>
        <button
          type="button"
          onClick={onNewBot}
          className="inline-flex size-7 items-center justify-center rounded-md text-white/70 transition-colors hover:bg-white/8 hover:text-white"
          aria-label="New bot"
        >
          <Plus className="size-3.5" aria-hidden />
        </button>
      </div>
      <label className="mx-3 mb-2 flex items-center gap-2 rounded-lg bg-white/5 px-2.5 py-1.5 text-white/45">
        <Search className="size-3.5 shrink-0" aria-hidden />
        <input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Search"
          className="w-full bg-transparent text-[13px] text-white outline-none placeholder:text-white/35"
          aria-label="Search bots"
        />
      </label>
      <ul className="min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-2">
        {bots.map((bot) => {
          const active = bot.id === selectedId;
          return (
            <li key={bot.id}>
              <button
                type="button"
                onClick={() => onSelect(bot)}
                className={cn(
                  "flex w-full items-start gap-2.5 rounded-xl px-2 py-2 text-left transition-colors",
                  active ? "bg-white/8" : "hover:bg-white/[0.04]",
                )}
              >
                <DemoBotAvatar name={bot.name} image={bot.image} />
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline justify-between gap-2">
                    <span className="truncate text-[14.5px] font-medium text-white">
                      {bot.name}
                    </span>
                    <span className="shrink-0 text-[11.5px] text-white/40">
                      {bot.time}
                    </span>
                  </span>
                  <span className="mt-0.5 block truncate text-[13px] text-white/45">
                    {bot.preview}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      <div className="flex items-center gap-2 border-t border-white/8 px-3 py-3">
        <span className="flex size-8 items-center justify-center rounded-full bg-white/10 text-[11px] font-medium text-white">
          You
        </span>
        <span className="text-[13px] text-white/70">Your workspace</span>
      </div>
    </>
  );
}

function ComputerPanel({
  bot,
  activeRoutine,
  onSelectRoutine,
  onNewRoutine,
  onClose,
}: {
  bot: DemoBot;
  activeRoutine: string | null;
  onSelectRoutine: (name: string) => void;
  onNewRoutine: () => void;
  onClose: () => void;
}) {
  return (
    <aside className="absolute inset-x-0 bottom-0 z-30 flex max-h-[82%] flex-col rounded-t-2xl border-t border-white/10 bg-[#0c0c0c] sm:inset-y-0 sm:right-0 sm:left-auto sm:max-h-none sm:w-[320px] sm:rounded-none sm:border-t-0 sm:border-l lg:static lg:flex lg:w-[348px]">
      <div className="mx-auto mt-2 h-1 w-10 rounded-full bg-white/15 sm:hidden" aria-hidden />
      <div className="flex items-center justify-between px-3 py-2">
        <p className="text-[13px] font-medium text-white/80">{bot.computer.title}</p>
        <button
          type="button"
          className="inline-flex size-7 items-center justify-center rounded-md text-white/45 hover:bg-white/8 hover:text-white lg:hidden"
          aria-label="Close panel"
          onClick={onClose}
        >
          <X className="size-3.5" aria-hidden />
        </button>
      </div>
      <div className="flex min-h-0 flex-1 flex-col px-3 pb-3">
        <DemoBrowser
          key={bot.id}
          computer={bot.computer}
          controlHref={appRoutes.workspace}
        />
        <p className="mt-3 mb-1 text-[11px] tracking-wide text-white/40 uppercase">
          Routines
        </p>
        <ul className="shrink-0 space-y-0.5">
          {bot.routines.map((routine) => {
            const active = activeRoutine === routine.name;
            return (
              <li key={routine.name}>
                <button
                  type="button"
                  onClick={() => onSelectRoutine(routine.name)}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px] transition-colors",
                    active
                      ? "bg-white/10 text-white"
                      : "text-white/80 hover:bg-white/8",
                  )}
                >
                  <CalendarClock className="size-3.5 text-primary" aria-hidden />
                  <span className="flex-1">{routine.name}</span>
                  <span className="text-[11px] text-white/35">{routine.cadence}</span>
                </button>
              </li>
            );
          })}
          <li>
            <button
              type="button"
              onClick={onNewRoutine}
              className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px] text-white/50 transition-colors hover:bg-white/8 hover:text-white"
            >
              <Plus className="size-3.5" aria-hidden />
              New routine
            </button>
          </li>
        </ul>
      </div>
    </aside>
  );
}

function DemoBubble({ message, bot }: { message: DemoMessage; bot: DemoBot }) {
  if (message.role === "user") {
    return (
      <div className="ml-auto max-w-[80%] rounded-2xl bg-[#f2f2f2] px-3.5 py-2.5 text-[13.5px] leading-5 text-[#111]">
        {message.text}
      </div>
    );
  }

  const body = message.checklist ? (
    <ul className="space-y-1.5">
      {message.checklist.map((item) => (
        <li key={item.label} className="flex items-start gap-2">
          <Check className="mt-0.5 size-3.5 shrink-0 text-success" aria-hidden />
          <span>
            <span className="font-medium text-white">{item.label}</span>
            <span className="text-white/45"> → {item.detail}</span>
          </span>
        </li>
      ))}
    </ul>
  ) : (
    message.text
  );

  return (
    <div className="flex max-w-[92%] items-end gap-2">
      <DemoBotAvatar name={bot.name} image={bot.image} size="xs" />
      <div className="min-w-0 rounded-2xl bg-[#1c1c1c] px-3.5 py-2.5 text-[13.5px] leading-5 text-white/85">
        {body}
      </div>
    </div>
  );
}
