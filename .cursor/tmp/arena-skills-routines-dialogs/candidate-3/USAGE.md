# Bot context overlays (candidate 3)

Skills and Routines leave the right rail. The rail shows **Computer** (live view + files) and **Memory** only. Routines and Skills open in the same compact `Dialog` pattern as `CreateBotDialog`, driven from a **single overlay slot** owned by `WorkspaceShell` so triggers work with the rail open, collapsed, or inside the mobile bot-details sheet.

## Quickstart

1. Wrap the signed-in bot workspace in `BotContextOverlayProvider` (shell already knows `selectedBotId`, `bot`, and `activeRun`).
2. Mount `BotContextOverlayHost` once next to `SettingsDialog` (one dialog root, never stacked with another modal).
3. Put `BotContextActivityDock` on the rail between Computer and Memory; pass the same `botId` / `conversationId` the sidebars need today.
4. When the desktop rail is collapsed, render the same dock in the conversation header via `BotContextHeaderActions`.
5. Do **not** mount `BotRoutinesSidebar` / `BotSkillsSidebar` inline in the rail; they only render as children of `BotContextOverlayHost` while open.

Import surface:

```tsx
import {
  BotContextOverlayProvider,
  BotContextOverlayHost,
  useBotContextOverlay,
} from "@/components/app/workspace/bot-context-overlay";
import { BotContextActivityDock } from "@/components/app/workspace/bot-context-activity-dock";
import { BotContextHeaderActions } from "@/components/app/workspace/bot-context-header-actions";
```

## Call site 1 — `WorkspaceShell`

Provider + host sit with the other global modals. Opening routines/skills closes settings first so two dialogs never stack.

```tsx
// workspace-shell.tsx (inside selectedBotId branch, around workspaceBody)

const overlayCtx = useMemo(
  () => ({
    botId: selectedBotId!,
    conversationId: activeRun?.conversationId,
  }),
  [selectedBotId, activeRun?.conversationId],
);

return (
  <BotContextOverlayProvider
    scope={overlayCtx}
    onRequestCloseSettings={() => setSettingsOpen(false)}
  >
    {workspaceBody}
    <BotContextOverlayHost
      botId={overlayCtx.botId}
      conversationId={overlayCtx.conversationId}
    />
  </BotContextOverlayProvider>
);
```

## Call site 2 — `BotContextRail`

Computer and Memory stay in the scroll area; routines/skills are dock buttons only.

```tsx
// bot-context-rail.tsx (bot present)

<ComputerRailSection bot={bot} activeRun={activeRun} />

<BotContextActivityDock
  className="mt-2 border-t border-border pt-2"
  botId={bot.id}
  conversationId={activeRun?.conversationId}
/>

<RailSection title="Memory">
  <BotMemoryPanel /* unchanged */ />
</RailSection>
```

## Call site 3 — `BotConversationView` (collapsed rail)

When `railCollapsed`, the header exposes the same two actions so users never need to expand the rail just to pause a routine.

```tsx
// bot-conversation-view.tsx (header actions, lg:flex when railCollapsed)

{railCollapsed ? (
  <BotContextHeaderActions
    botId={botId}
    conversationId={activeRun?.conversationId}
    className="hidden lg:flex"
  />
) : null}
```

Opening from the dock or header:

```tsx
function BotContextActivityDock({ botId, conversationId, className }: Props) {
  const { open, panel } = useBotContextOverlay();

  return (
    <div className={cn("flex gap-1", className)} role="group" aria-label="Bot activity">
      <ComposerIconButton
        label="Routines"
        aria-pressed={panel === "routines"}
        onClick={() => open("routines", { botId, conversationId })}
      >
        <CalendarClock className="size-4" />
      </ComposerIconButton>
      <ComposerIconButton
        label="Skills"
        aria-pressed={panel === "skills"}
        onClick={() => open("skills", { botId })}
      >
        <Sparkles className="size-4" />
      </ComposerIconButton>
    </div>
  );
}
```

Toggle behavior: clicking the active panel’s trigger again calls `close()`. Switching panels replaces the open panel without a second dialog instance.

## What callers get back

`useBotContextOverlay()` returns:

| Field | Meaning |
| --- | --- |
| `panel` | `"closed"` \| `"routines"` \| `"skills"` |
| `open(panel, scope)` | Opens overlay; closes `SettingsDialog` via provider callback |
| `close()` | Idempotent; returns to `"closed"` |
| `isOpen` | `panel !== "closed"` |

No fetch types leak to callers — lists stay inside existing sidebar components.

## Dialog UX (minimal + responsive)

- `Dialog` + `DialogContent` with `p-4 sm:max-w-lg` and `max-h-[min(72dvh,32rem)] overflow-y-auto` (same tone as create-bot).
- Title: **Routines** or **Skills**; one-line `DialogDescription` matching today’s empty-state intent (schedule in chat / save from chat or settings).
- Body: `BotRoutinesSidebar` (`variant="minimal"`) or `BotSkillsSidebar` (`variant="minimal"`) — **only mounted while open** so 15s polling runs only during the dialog lifetime.
- Footer link row unchanged (New routine, Advanced, Manage skills).

## Tests callers should update

- `bot-context-rail.test.tsx`: expect **Computer**, **Memory**, and dock labels; deny inline routine/skill list text unless a dialog is open.
- New `bot-context-overlay.test.tsx`: open/close/toggle/switch panel; assert unmount stops polling (mock `cloudHostFetch` call count over time).
