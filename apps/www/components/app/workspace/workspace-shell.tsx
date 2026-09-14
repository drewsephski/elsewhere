"use client";

import type { BotSummary, RunSummary } from "@/lib/api-types";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { siteConfig } from "@elsewhere/brand";
import { cn } from "cn";
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";
import { BotContextRail } from "./bot-context-rail";
import { BotConversationView } from "./bot-conversation-view";
import { BotListSidebar } from "./bot-list-sidebar";
import { CreateBotDialog } from "./create-bot-dialog";
import { MobileSheet } from "./mobile-sheet";
import { ProfileFooter } from "./profile-footer";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { ProductLogo } from "@/components/product-logo";
import { Button } from "@/components/ui/button";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BrowserPreviewProvider } from "@/contexts/browser-preview-context";

interface WorkspaceShellProps {
  userEmail: string;
  children: React.ReactNode;
}

function parseBotId(pathname: string): string | null {
  const match = pathname.match(/^\/app\/bots\/([^/]+)$/);
  return match?.[1] ?? null;
}

export function WorkspaceShell({ userEmail, children }: WorkspaceShellProps) {
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const { data: workspace, error: workspaceError, refresh } = useWorkspaceOverview();

  const selectedBotId = parseBotId(pathname);
  const [createOpen, setCreateOpen] = useState(false);
  const [contextSheetOpen, setContextSheetOpen] = useState(false);
  const [connectionOpen, setConnectionOpen] = useState(false);
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [runActivityAt, setRunActivityAt] = useState<Record<string, string>>({});

  const bots = workspace?.bots ?? [];

  useEffect(() => {
    if (searchParams.get("create") === "1") {
      setCreateOpen(true);
    }
  }, [searchParams]);

  useEffect(() => {
    if (selectedBotId) {
      return;
    }
    if (pathname !== "/app") {
      return;
    }
    if (!bots.length) {
      return;
    }
    const isDesktop = window.matchMedia("(min-width: 1024px)").matches;
    if (isDesktop) {
      router.replace(`/app/bots/${bots[0].id}`);
    }
  }, [bots, pathname, router, selectedBotId]);

  useEffect(() => {
    let stopped = false;
    async function loadActivity() {
      try {
        const response = await cloudHostFetch("/v1/runs?limit=50");
        if (!response.ok) {
          return;
        }
        const runs: RunSummary[] = await response.json();
        if (stopped) {
          return;
        }
        const map: Record<string, string> = {};
        for (const run of runs) {
          const existing = map[run.botId];
          if (!existing || run.createdAt > existing) {
            map[run.botId] = run.createdAt;
          }
        }
        setRunActivityAt(map);
      } catch {
        /* ignore */
      }
    }
    void loadActivity();
    const timer = setInterval(() => void loadActivity(), 15000);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, []);

  const activeRun = useMemo(() => {
    if (!selectedBotId || !workspace) {
      return null;
    }
    const presence = workspace.bots.find((item) => item.id === selectedBotId);
    if (!presence?.workId) {
      return null;
    }
    return {
      runId: presence.workId,
      task: presence.task ?? "",
      botId: selectedBotId,
      status: presence.presence === "working" ? "running" : "queued",
    } as RunSummary;
  }, [selectedBotId, workspace]);

  const handleBotLoaded = useCallback((loaded: BotSummary) => {
    setBot(loaded);
  }, []);

  const handleDeleteBot = useCallback(
    async (botId: string) => {
      const response = await cloudHostFetch(`/v1/bots/${botId}`, { method: "DELETE" });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(
          typeof body.error === "string" ? body.error : "Could not delete bot",
        );
      }
      await refresh();
      if (selectedBotId === botId) {
        router.push("/app");
      }
      router.refresh();
    },
    [refresh, router, selectedBotId],
  );

  const handleRenameBot = useCallback(
    async (botId: string, name: string) => {
      const response = await cloudHostFetch(`/v1/bots/${botId}`, {
        method: "PATCH",
        body: JSON.stringify({ name }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not rename bot");
      }
      await refresh();
      if (bot?.id === botId) {
        setBot(body as BotSummary);
      }
    },
    [bot?.id, refresh],
  );

  const showConversation = Boolean(selectedBotId);

  const previewEnabled = Boolean(
    bot?.computerId &&
      activeRun &&
      (activeRun.status === "queued" || activeRun.status === "running"),
  );

  const previewProviderProps = {
    computerId: bot?.computerId ?? null,
    enabled: previewEnabled,
    sessionKey: activeRun?.runId ?? null,
  };

  const conversationMain = (
    <main
      className={cn(
        "flex min-w-0 flex-1 flex-col bg-[#f8f6fc]",
        !showConversation && "hidden lg:flex",
      )}
    >
      {workspaceError ? (
        <p className="shrink-0 px-4 py-2 text-xs text-amber-800" role="status">
          {workspaceError}. Activity may be out of date.
        </p>
      ) : null}
      {workspace && !workspace.runnerReady ? (
        <p className="shrink-0 border-b border-amber-200 bg-amber-50 px-4 py-2 text-xs text-amber-900">
          Background work is temporarily unavailable. Queued assignments stay saved.
        </p>
      ) : null}
      {selectedBotId ? (
        <BotConversationView
          botId={selectedBotId}
          onOpenContext={() => setContextSheetOpen(true)}
          onBotLoaded={handleBotLoaded}
          onRenameBot={handleRenameBot}
        />
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-6 px-6 py-8 text-center">
          {children}
          <ProviderStatusCard variant="featured" />
          <div className="max-w-md space-y-3">
            <p className="text-sm text-muted-foreground">
              After ChatGPT is connected, create a bot to start chatting and running work.
            </p>
            <Button
              type="button"
              variant="outline"
              className="rounded-full border-primary/30 bg-primary/5 text-primary hover:bg-primary/10"
              onClick={() => setCreateOpen(true)}
            >
              Create your first bot
            </Button>
          </div>
        </div>
      )}
    </main>
  );

  const contextRail = selectedBotId ? (
    <aside
      className={cn(
        "hidden w-[min(100%,20rem)] shrink-0 border-l border-border/70 bg-white/55 backdrop-blur-md lg:flex lg:flex-col",
      )}
      aria-label="Bot context"
    >
      <BotContextRail
        bot={bot}
        activeRun={activeRun}
        showConnectionSettings={false}
        onBotSaved={setBot}
      />
    </aside>
  ) : null;

  const workspaceBody = (
    <>
      <div className="flex min-h-0 flex-1">
        {/* Left: bot list — desktop always; mobile when no bot selected */}
        <aside
          className={cn(
            "flex w-full max-w-md flex-col border-r border-border/70 bg-white/55 backdrop-blur-md lg:w-[min(100%,20rem)] lg:max-w-none lg:shrink-0",
            showConversation ? "hidden lg:flex" : "flex",
          )}
          aria-label="Bots"
        >
          <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border/60 px-4">
            <Link href="/app" className="flex items-center gap-2">
              <ProductLogo size="md" />
              <span className="text-sm font-semibold tracking-tight">
                {siteConfig.productName}
              </span>
            </Link>
          </div>
          <BotListSidebar
            bots={bots}
            selectedBotId={selectedBotId}
            runActivityAt={runActivityAt}
            onCreateBot={() => setCreateOpen(true)}
            onRenameBot={handleRenameBot}
            onDeleteBot={handleDeleteBot}
            footer={
              <ProfileFooter
                email={userEmail}
                onOpenSettings={() => setConnectionOpen(true)}
              />
            }
            className="min-h-0 flex-1"
          />
        </aside>

        {conversationMain}
        {contextRail}
      </div>

      <CreateBotDialog open={createOpen} onClose={() => setCreateOpen(false)} />

      <MobileSheet
        open={contextSheetOpen}
        title={bot?.name ? `${bot.name} details` : "Bot details"}
        onClose={() => setContextSheetOpen(false)}
      >
        {selectedBotId ? (
          <BotContextRail
            bot={bot}
            activeRun={activeRun}
            showConnectionSettings={false}
            onBotSaved={setBot}
          />
        ) : null}
      </MobileSheet>

      <MobileSheet
        open={connectionOpen}
        title="ChatGPT connection"
        onClose={() => setConnectionOpen(false)}
      >
        <ProviderStatusCard />
      </MobileSheet>
    </>
  );

  return (
    <div className="app-shell-bg flex h-[100dvh] flex-col overflow-hidden text-foreground">
      {selectedBotId ? (
        <BrowserPreviewProvider {...previewProviderProps}>
          {workspaceBody}
        </BrowserPreviewProvider>
      ) : (
        workspaceBody
      )}
    </div>
  );
}
