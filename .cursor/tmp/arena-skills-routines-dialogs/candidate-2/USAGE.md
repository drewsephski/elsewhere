# Workspace bot overlays (candidate 2)

Routines and Skills leave the right rail. The rail shows **Computer** (live preview + files) and **Memory** only. Opening Routines or Skills uses one shared dialog host at the workspace shell so only one overlay is active and polling runs only while that dialog is open.

## Quickstart

1. Wrap the signed-in bot workspace in `WorkspaceBotOverlayProvider` (in `WorkspaceShell`, alongside existing dialog state).
2. Mount `BotListDialogHost` once at shell level (sibling to `SettingsDialog`, not inside the rail).
3. Replace inline rail lists with `BotOverlayTriggers` buttons that call `openRoutines()` / `openSkills()`.
4. Duplicate those triggers in the conversation header when the desktop rail is collapsed or on mobile (`lg:hidden`), so access does not depend on the rail being visible.

Consumers import the hook, not overlay wire types:

```tsx
import { useWorkspaceBotOverlay } from "@/contexts/workspace-bot-overlay-context";
```

## Call site 1 — `BotContextRail` (desktop rail + mobile sheet)

Between the **Computer** collapsible and **Memory** collapsible, render compact triggers. Do not mount `BotRoutinesSidebar` or `BotSkillsSidebar` in the rail.

```tsx
// bot-context-rail.tsx (conceptual)
import { BotOverlayTriggers } from "./bot-overlay-triggers";
import { BotMemoryPanel } from "@/components/app/bot-memory";
import { ComputerStatePanel } from "./computer-state-panel";
import { ComputerWorkspaceTree } from "./computer-workspace-tree";

export function BotContextRail({ bot, activeRun, onBotSaved, ... }: BotContextRailProps) {
  const conversationId = activeRun?.conversationId;

  return (
    <div className="...">
      {/* header: collapse + settings unchanged */}
      <div className="flex-1 overflow-y-auto px-3 pb-3">
        <RailSection title="Computer" defaultOpen>
          <ComputerStatePanel bot={bot} activeRun={activeRun} variant="minimal" />
          {bot?.computerId ? (
            <div className="mt-2 border-t border-border pt-2">
              <ComputerWorkspaceTree computerId={bot.computerId} />
            </div>
          ) : null}
        </RailSection>

        {bot ? (
          <>
            <BotOverlayTriggers
              className="border-t border-border py-2"
              disabled={false}
            />
            <RailSection title="Memory">
              <BotMemoryPanel
                botId={bot.id}
                learnFromConversations={Boolean(bot.learnFromConversations)}
                onLearnChanged={(enabled) =>
                  onBotSaved({ ...bot, learnFromConversations: enabled })
                }
                embedded
              />
            </RailSection>
          </>
        ) : null}
      </div>
    </div>
  );
}
```

`BotOverlayTriggers` reads `botId` / `conversationId` from overlay context scope set by the provider when the user enters a bot conversation (see call site 3).

## Call site 2 — `BotConversationView` header (collapsed rail + mobile)

When the user cannot see the rail triggers, show the same pair in the conversation header.

```tsx
// bot-conversation-view.tsx (conceptual, inside header actions)
import { BotOverlayTriggers } from "./bot-overlay-triggers";

const showOverlayTriggers = railCollapsed; // desktop collapsed
// OR always on mobile — implement as:
// className={cn("lg:hidden", !railCollapsed && "hidden")} on a wrapper

{bot ? (
  <BotOverlayTriggers
    variant="header"
    className={cn(
      "flex items-center gap-0.5",
      railCollapsed ? "hidden lg:flex" : "lg:hidden",
    )}
  />
) : null}
```

`variant="header"` renders icon-sized `ComposerIconButton`s with labels "Routines" and "Skills" instead of the rail’s text row.

## Call site 3 — `WorkspaceShell` (provider, scope, single dialog host)

The shell owns overlay state, binds it to the active bot, and ensures settings and bot overlays do not stack.

```tsx
// workspace-shell.tsx (conceptual)
import {
  WorkspaceBotOverlayProvider,
  useWorkspaceBotOverlay,
} from "@/contexts/workspace-bot-overlay-context";
import { BotListDialogHost } from "./bot-list-dialog-host";

function WorkspaceShellInner(...) {
  const selectedBotId = parseBotId(pathname);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const handleOpenSettings = (section: SettingsSection) => {
    overlay.close(); // from context helper wired in provider
    setSettingsSection(section);
    setSettingsOpen(true);
  };

  return (
    <WorkspaceBotOverlayProvider
      botId={selectedBotId}
      conversationId={activeRun?.conversationId ?? null}
      onConflict={() => setSettingsOpen(false)}
    >
      {/* ... sidebar, main, BotContextRail, MobileSheet ... */}
      <BotListDialogHost />
      <SettingsDialog open={settingsOpen} ... />
    </WorkspaceBotOverlayProvider>
  );
}
```

Opening routines from any trigger:

```tsx
function BotOverlayTriggers({ variant, className }: BotOverlayTriggersProps) {
  const { openRoutines, openSkills, canOpen } = useWorkspaceBotOverlay();

  return (
    <div className={className} role="group" aria-label="Bot lists">
      <button type="button" disabled={!canOpen} onClick={() => openRoutines()}>
        Routines
      </button>
      <button type="button" disabled={!canOpen} onClick={() => openSkills()}>
        Skills
      </button>
    </div>
  );
}
```

`BotListDialogHost` renders nothing when overlay is `none`. When `routines` or `skills`, it shows one `Dialog` with title/description and mounts the existing sidebar component with `pollWhileOpen`:

```tsx
// bot-list-dialog-host.tsx (conceptual)
export function BotListDialogHost() {
  const { overlay, close } = useWorkspaceBotOverlay();

  if (overlay.tag === "none") {
    return null;
  }

  const { botId, conversationId } = overlay.scope;

  return (
    <Dialog open onOpenChange={(next) => !next && close()}>
      <DialogContent className="gap-3 p-4 sm:max-w-lg">
        <DialogHeader>...</DialogHeader>
        {overlay.tag === "routines" ? (
          <BotRoutinesSidebar
            botId={botId}
            conversationId={conversationId}
            variant="minimal"
            pollWhileOpen
          />
        ) : (
          <BotSkillsSidebar botId={botId} variant="minimal" pollWhileOpen />
        )}
      </DialogContent>
    </Dialog>
  );
}
```

## What callers should not do

- Do not add a second `Dialog` for skills while routines uses another — use the single host.
- Do not embed list UIs in `SettingsDialog` as the primary path (settings Skills tab stays attach/pin only).
- Do not pass `cloudHostFetch` responses into triggers; lists keep fetching inside sidebar components.

## Tests (caller expectations)

- `bot-context-rail.test.tsx`: expect **Computer** and **Memory**, not inline "Routines"/"Skills" section titles; `BotOverlayTriggers` labels present; list empty copy not in rail until dialog opens.
- New tests on `BotListDialogHost`: open routines → pause/resume control exists; close → unmount stops polling (mock interval).
- Header triggers visible when `railCollapsed` (desktop) and on mobile viewport.
