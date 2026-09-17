"use client";

import type { BotSummary, GroupListItem, RunSummary } from "@/lib/api-types";
import { useWorkspaceOverview } from "@/hooks/use-workspace-overview";
import { cn } from "cn";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { BotContextRail } from "./bot-context-rail";
import { BotConversationView } from "./bot-conversation-view";
import { GroupConversationView } from "./group-conversation-view";
import { CreateGroupDialog } from "./create-group-dialog";
import { BotListSidebar } from "./bot-list-sidebar";
import { CreateBotDialog } from "./create-bot-dialog";
import { MobileSheet } from "./mobile-sheet";
import { ProfileFooter } from "./profile-footer";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { Button } from "@/components/ui/button";
import { cloudHostFetch } from "@/lib/cloud-api";
import { ActiveRunProvider, useActiveRun } from "@/contexts/active-run-context";
import { BrowserPreviewProvider } from "@/contexts/browser-preview-context";

/** Must be module-scoped — an inline component remounts the whole workspace on every parent render. */
function WorkspaceBrowserPreviewLayer({
  computerId,
  enabled,
  sessionKey,
  children,
}: {
  computerId: string | null;
  enabled: boolean;
  sessionKey: string | null | undefined;
  children: ReactNode;
}) {
  const { browserPreviewGeneration } = useActiveRun();
  return (
    <BrowserPreviewProvider
      computerId={computerId}
      enabled={enabled}
      sessionKey={sessionKey}
      refreshGeneration={browserPreviewGeneration}
    >
      {children}
    </BrowserPreviewProvider>
  );
}

interface WorkspaceShellProps {
  userEmail: string;
  children: React.ReactNode;
}

function parseBotId(pathname: string): string | null {
  const match = pathname.match(/^\/app\/bots\/([^/]+)$/);
  return match?.[1] ?? null;
}

function parseGroupId(pathname: string): string | null {
  const match = pathname.match(/^\/app\/groups\/([^/]+)$/);
  return match?.[1] ?? null;
}

export function WorkspaceShell({ userEmail, children }: WorkspaceShellProps) {
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const { data: workspace, error: workspaceError, phase: workspacePhase, refresh } =
    useWorkspaceOverview();

  const selectedBotId = parseBotId(pathname);
  const selectedGroupId = parseGroupId(pathname);
  const [createOpen, setCreateOpen] = useState(false);
  const [createGroupOpen, setCreateGroupOpen] = useState(false);
  const [contextSheetOpen, setContextSheetOpen] = useState(false);
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [connectionOpen, setConnectionOpen] = useState(false);
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [runActivityAt, setRunActivityAt] = useState<Record<string, string>>({});
  const [streamRunId, setStreamRunId] = useState<string | null>(null);
  const [groups, setGroups] = useState<GroupListItem[]>([]);

  const bots = workspace?.bots ?? [];

  const refreshGroups = useCallback(async () => {
    try {
      const response = await cloudHostFetch("/v1/conversations/groups");
      if (!response.ok) {
        return;
      }
      const next: GroupListItem[] = await response.json();
      setGroups(next);
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    let stopped = false;
    async function loadGroups() {
      try {
        const response = await cloudHostFetch("/v1/conversations/groups");
        if (!response.ok) {
          return;
        }
        const next: GroupListItem[] = await response.json();
        if (!stopped) {
          setGroups(next);
        }
      } catch {
        /* ignore */
      }
    }
    void loadGroups();
    const timer = setInterval(() => void loadGroups(), 12000);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (searchParams.get("create") !== "1") {
      return;
    }
    setCreateOpen(true);
    const params = new URLSearchParams(searchParams.toString());
    params.delete("create");
    const query = params.toString();
    router.replace(query ? `/app?${query}` : "/app");
  }, [router, searchParams]);

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

  const handleStreamRunIdChange = useCallback((runId: string | null) => {
    setStreamRunId((current) => (current === runId ? current : runId));
  }, []);

  const handleBotLoaded = useCallback((loaded: BotSummary) => {
    setBot(loaded);
  }, []);

  const handleBotSaved = useCallback(
    (updated: BotSummary) => {
      setBot(updated);
      void refresh();
    },
    [refresh],
  );

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

  const handleRenameGroup = useCallback(
    async (groupId: string, name: string) => {
      const response = await cloudHostFetch(`/v1/conversations/${groupId}`, {
        method: "PATCH",
        body: JSON.stringify({ name }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not rename group");
      }
      await refreshGroups();
    },
    [refreshGroups],
  );

  const handleDeleteGroup = useCallback(
    async (groupId: string) => {
      const response = await cloudHostFetch(`/v1/conversations/${groupId}`, {
        method: "DELETE",
      });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(
          typeof body.error === "string" ? body.error : "Could not delete group",
        );
      }
      await refreshGroups();
      if (selectedGroupId === groupId) {
        router.push("/app");
      }
      router.refresh();
    },
    [refreshGroups, router, selectedGroupId],
  );

  const showConversation = Boolean(selectedBotId || selectedGroupId);

  const previewEnabled = Boolean(bot?.computerId);
  const previewComputerId = bot?.computerId ?? null;
  const previewSessionKey = activeRun?.runId ?? streamRunId;

  const conversationMain = (
    <main
      className={cn(
        "flex min-w-0 flex-1 flex-col bg-background",
        !showConversation && "hidden lg:flex",
      )}
    >
      {workspaceError ? (
        <p
          className="shrink-0 border-b border-border px-4 py-1.5 text-center text-[11px] text-warning"
          role="status"
        >
          {workspaceError}. Activity may be out of date.
        </p>
      ) : null}
      {workspace && !workspace.runnerReady ? (
        <p className="shrink-0 border-b border-border bg-warning/8 px-4 py-1.5 text-center text-[11px] text-warning">
          Background work is temporarily unavailable. Queued assignments stay saved.
        </p>
      ) : null}
      {selectedGroupId ? (
        <GroupConversationView groupId={selectedGroupId} bots={bots} />
      ) : selectedBotId ? (
        <BotConversationView
          botId={selectedBotId}
          onOpenContext={() => setContextSheetOpen(true)}
          onBotLoaded={handleBotLoaded}
          syncedBot={bot}
          onRenameBot={handleRenameBot}
          onStreamRunIdChange={handleStreamRunIdChange}
          railCollapsed={railCollapsed}
          onExpandRail={() => setRailCollapsed(false)}
        />
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-6 px-6 py-8 text-center">
          {children}
          <ProviderStatusCard variant="featured" />
          <div className="max-w-md space-y-3">
            <p className="text-[13px] text-muted-foreground">
              After ChatGPT is connected, create a bot to start chatting and running work.
            </p>
            <Button
              type="button"
              variant="outline"
              className="rounded-full px-4"
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
        "hidden w-[min(100%,18.5rem)] min-w-0 shrink-0 overflow-hidden border-l border-border bg-surface lg:flex lg:flex-col",
        railCollapsed && "lg:hidden",
      )}
      aria-label="Bot context"
    >
      <BotContextRail
        bot={bot}
        activeRun={activeRun}
        showConnectionSettings={false}
        onBotSaved={handleBotSaved}
        onCollapse={() => setRailCollapsed(true)}
      />
    </aside>
  ) : null;

  const workspaceBody = (
    <>
      <div className="flex min-h-0 flex-1">
        {/* Left: bot list — desktop always; mobile when no bot selected */}
        <aside
          className={cn(
            "flex w-full flex-col border-r border-border bg-surface",
            "md:max-w-[min(100%,16rem)] lg:w-60 lg:max-w-none lg:shrink-0",
            showConversation ? "hidden lg:flex" : "flex",
          )}
          aria-label="Bots"
        >
          <BotListSidebar
            bots={bots}
            groups={groups}
            selectedBotId={selectedBotId}
            selectedGroupId={selectedGroupId}
            runActivityAt={runActivityAt}
            workspacePhase={workspacePhase}
            workspaceError={workspaceError}
            onCreateBot={() => setCreateOpen(true)}
            onCreateGroup={() => setCreateGroupOpen(true)}
            onRenameBot={handleRenameBot}
            onDeleteBot={handleDeleteBot}
            onRenameGroup={handleRenameGroup}
            onDeleteGroup={handleDeleteGroup}
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
      <CreateGroupDialog
        open={createGroupOpen}
        bots={bots}
        onClose={() => setCreateGroupOpen(false)}
        onCreated={(groupId) => {
          void refreshGroups();
          router.push(`/app/groups/${groupId}`);
        }}
      />

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
            onBotSaved={handleBotSaved}
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
    <div className="workspace-window flex h-[100dvh] flex-col overflow-hidden text-foreground">
      <div className="workspace-window-frame flex min-h-0 flex-1 flex-col overflow-hidden">
        {selectedBotId ? (
          <ActiveRunProvider runId={streamRunId}>
            <WorkspaceBrowserPreviewLayer
              computerId={previewComputerId}
              enabled={previewEnabled}
              sessionKey={previewSessionKey}
            >
              {workspaceBody}
            </WorkspaceBrowserPreviewLayer>
          </ActiveRunProvider>
        ) : (
          workspaceBody
        )}
      </div>
    </div>
  );
}
