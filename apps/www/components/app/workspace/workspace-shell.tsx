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
import { SettingsDialog } from "@/components/app/settings-dialog";
import { WorkspaceQuickStart } from "./workspace-quick-start";
import { cloudHostFetch } from "@/lib/cloud-api";
import { parseSettingsSection, type SettingsSection } from "@/lib/settings-sections";
import { ActiveRunProvider, useActiveRun } from "@/contexts/active-run-context";
import { BrowserPreviewProvider } from "@/contexts/browser-preview-context";
import { DesktopTitlebar } from "@/components/app/desktop-titlebar";

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
  const [quickStartBusy, setQuickStartBusy] = useState(false);
  const [contextSheetOpen, setContextSheetOpen] = useState(false);
  // Sidebar collapse is independent of preview dock/float. A docked preview
  // hides with the rail; a floating preview stays over the chat pane.
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("general");
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
    function handleKeyDown(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey) || event.key !== ",") {
        return;
      }
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT")
      ) {
        return;
      }
      event.preventDefault();
      setSettingsSection("general");
      setSettingsOpen(true);
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
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
    const section = parseSettingsSection(searchParams.get("settings"));
    if (!section) {
      return;
    }
    setSettingsSection(section);
    setSettingsOpen(true);
    const params = new URLSearchParams(searchParams.toString());
    params.delete("settings");
    const query = params.toString();
    router.replace(query ? `${pathname}?${query}` : pathname);
  }, [pathname, router, searchParams]);

  useEffect(() => {
    if (selectedBotId || quickStartBusy) {
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
      const params = searchParams.toString();
      router.replace(params ? `/app/bots/${bots[0].id}?${params}` : `/app/bots/${bots[0].id}`);
    }
  }, [bots, pathname, quickStartBusy, router, searchParams, selectedBotId]);

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

  const handleOpenSettings = useCallback((section: SettingsSection = "general") => {
    setSettingsSection(section);
    setSettingsOpen(true);
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

  const workspaceLoading =
    !workspace && (workspacePhase === "initial" || workspacePhase === "loading");
  const showQuickStart =
    !selectedBotId && !selectedGroupId && !workspaceLoading && bots.length === 0;
  const showConversation = Boolean(selectedBotId || selectedGroupId);
  const showMainPane = showConversation || showQuickStart;

  const previewEnabled = Boolean(bot?.computerId);
  const previewComputerId = bot?.computerId ?? null;
  const previewSessionKey = activeRun?.runId ?? streamRunId;

  const conversationMain = (
    <main
      className={cn(
        "flex min-w-0 flex-1 flex-col bg-background",
        !showMainPane && "hidden lg:flex",
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
      ) : workspaceLoading ? (
        <div className="flex flex-1 items-center justify-center px-6 py-8">
          <p className="text-sm text-muted-foreground">Loading workspace…</p>
        </div>
      ) : showQuickStart ? (
        <WorkspaceQuickStart
          onStarted={() => {
            void refresh();
          }}
          onBusyChange={setQuickStartBusy}
        />
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-8 text-center">
          <p className="text-sm text-muted-foreground">Select a bot to start chatting.</p>
        </div>
      )}
    </main>
  );

  const contextRail = selectedBotId ? (
    <aside
      className={cn(
        "relative hidden w-[min(100%,18.5rem)] min-w-0 shrink-0 overflow-hidden border-l border-border bg-surface lg:flex lg:flex-col",
        railCollapsed && "lg:hidden",
      )}
      aria-label="Bot context"
    >
      <BotContextRail
        bot={bot}
        activeRun={activeRun}
        onBotSaved={handleBotSaved}
        onOpenSettings={() => handleOpenSettings("general")}
        onCollapseRail={() => setRailCollapsed(true)}
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
            showMainPane ? "hidden lg:flex" : "flex",
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
                onOpenSettings={(section) => handleOpenSettings(section ?? "chatgpt")}
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
            onBotSaved={handleBotSaved}
            onOpenSettings={() => handleOpenSettings("general")}
          />
        ) : null}
      </MobileSheet>

      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        section={settingsSection}
        onSectionChange={setSettingsSection}
        bot={bot}
        onBotSaved={handleBotSaved}
        onBotDeleted={() => setSettingsOpen(false)}
      />
    </>
  );

  return (
    <div className="workspace-window flex h-[100dvh] flex-col overflow-hidden text-foreground">
      <DesktopTitlebar />
      {/* Keep router outlet mounted for desktop shell + any future page slots. */}
      <div className="hidden" aria-hidden inert>
        {children}
      </div>
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
