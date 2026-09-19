# Type sketch — workspace bot overlays (candidate 2)

## Module map

| Module | Owns |
|--------|------|
| `apps/www/lib/workspace-bot-overlay.ts` | Discriminated overlay state, scope, pure transitions |
| `apps/www/contexts/workspace-bot-overlay-context.tsx` | Provider, `useWorkspaceBotOverlay`, scope from active bot |
| `apps/www/components/app/workspace/bot-list-dialog-host.tsx` | Single `Dialog`; mounts one list panel |
| `apps/www/components/app/workspace/bot-overlay-triggers.tsx` | Rail + header trigger UI |
| `apps/www/components/app/workspace/bot-context-rail.tsx` | Computer + Memory layout; triggers only |
| `apps/www/components/app/workspace/workspace-shell.tsx` | Provider wrap, conflict with settings |
| `apps/www/components/app/workspace/bot-conversation-view.tsx` | Header triggers when rail hidden |
| `apps/www/components/app/workspace/bot-routines-sidebar.tsx` | List behavior + optional polling |
| `apps/www/components/app/workspace/bot-skills-sidebar.tsx` | List behavior + optional polling |
| `apps/www/components/app/workspace/bot-context-rail.test.tsx` | Rail structure assertions |
| `apps/www/components/app/workspace/bot-list-dialog-host.test.tsx` | Dialog open/close + list behaviors |

No changes to `/app/skills`, `/app/routines`, or `SettingsDialog` body beyond closing overlay when settings opens.

## Data structures (first)

```ts
// lib/workspace-bot-overlay.ts

/** Active bot conversation scope; required to open either list. */
export type BotOverlayScope = Readonly<{
  botId: string;
  conversationId: string | null;
}>;

/** Exactly one overlay slot; replaces paired booleans. */
export type WorkspaceBotOverlay =
  | Readonly<{ tag: "none" }>
  | Readonly<{ tag: "routines"; scope: BotOverlayScope }>
  | Readonly<{ tag: "skills"; scope: BotOverlayScope }>;

export type WorkspaceBotOverlayAction =
  | { type: "open"; list: "routines" | "skills"; scope: BotOverlayScope }
  | { type: "close" }
  | { type: "scope_changed"; scope: BotOverlayScope | null };

/** Invariant: open(*) requires non-null scope.botId. */
export function reduceWorkspaceBotOverlay(
  state: WorkspaceBotOverlay,
  action: WorkspaceBotOverlayAction,
): WorkspaceBotOverlay {
  switch (action.type) {
    case "close":
      return { tag: "none" };
    case "scope_changed":
      if (action.scope === null) {
        return { tag: "none" };
      }
      if (state.tag === "none") {
        return state;
      }
      // Keep open list but rebind scope (same bot, new conversation id).
      return { ...state, scope: action.scope };
    case "open":
      return { tag: action.list, scope: action.scope };
    default:
      return state;
  }
}

export function overlayIsOpen(
  overlay: WorkspaceBotOverlay,
): overlay is Extract<WorkspaceBotOverlay, { tag: "routines" | "skills" }> {
  return overlay.tag !== "none";
}
```

```ts
// contexts/workspace-bot-overlay-context.tsx

export type WorkspaceBotOverlayContextValue = Readonly<{
  overlay: WorkspaceBotOverlay;
  scope: BotOverlayScope | null;
  canOpen: boolean;
  openRoutines: () => void;
  openSkills: () => void;
  close: () => void;
}>;

export function WorkspaceBotOverlayProvider(props: {
  botId: string | null;
  conversationId: string | null;
  onConflict?: () => void;
  children: React.ReactNode;
}): JSX.Element {
  // TODO: derive scope from props; dispatch scope_changed on bot/conversation change
  // TODO: openRoutines/openSkills no-op when scope null; else dispatch open
  // TODO: when parent calls onConflict (settings opened), dispatch close
  throw new Error("not implemented");
}

export function useWorkspaceBotOverlay(): WorkspaceBotOverlayContextValue {
  throw new Error("not implemented");
}
```

## List panels — polling invariant

```ts
// bot-routines-sidebar.tsx / bot-skills-sidebar.tsx (added props)

export type BotListPollPolicy =
  | Readonly<{ mode: "once" }>
  | Readonly<{ mode: "interval"; ms: 15_000 }>;

export interface BotRoutinesSidebarProps {
  botId: string;
  conversationId?: string;
  className?: string;
  variant?: "default" | "minimal";
  /** When true, poll every 15s while mounted. Dialog host passes true; default false after rail removal. */
  pollWhileOpen?: boolean;
}

export function BotRoutinesSidebar(props: BotRoutinesSidebarProps): JSX.Element {
  const poll: BotListPollPolicy = props.pollWhileOpen
    ? { mode: "interval", ms: 15_000 }
    : { mode: "once" };
  // TODO: useEffect loads once; interval only if poll.mode === "interval"
  // Existing toggle, empty copy, links unchanged.
  throw new Error("not implemented");
}

export function BotSkillsSidebar(props: {
  botId: string;
  variant?: "default" | "minimal";
  pollWhileOpen?: boolean;
}): JSX.Element {
  throw new Error("not implemented");
}
```

**Invariant:** With default `pollWhileOpen: false`, mounting in tests does not register 15s timers unless explicitly opted in.

## Dialog host (one overlay)

```tsx
// bot-list-dialog-host.tsx

const DIALOG_COPY = {
  routines: {
    title: "Routines",
    description: "Recurring work for this bot. Pause or resume without leaving chat.",
  },
  skills: {
    title: "Skills",
    description: "Skills attached to this bot. Versions shown as pinned or current.",
  },
} as const;

export function BotListDialogHost(): JSX.Element | null {
  const { overlay, close } = useWorkspaceBotOverlay();
  if (overlay.tag === "none") {
    return null;
  }

  const copy = DIALOG_COPY[overlay.tag];
  const { botId, conversationId } = overlay.scope;

  return (
    <Dialog open onOpenChange={(next) => !next && close()}>
      <DialogContent className="max-h-[min(85dvh,calc(100dvh-2rem))] gap-3 overflow-y-auto p-4 sm:max-w-lg">
        <DialogHeader className="gap-1">
          <DialogTitle className="text-base">{copy.title}</DialogTitle>
          <DialogDescription className="text-xs">{copy.description}</DialogDescription>
        </DialogHeader>
        {overlay.tag === "routines" ? (
          <BotRoutinesSidebar
            botId={botId}
            conversationId={conversationId ?? undefined}
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

**Invariant:** At most one `Dialog` from this host; switching `openSkills()` while routines open replaces reducer state (no stack).

## Triggers

```tsx
// bot-overlay-triggers.tsx

export type BotOverlayTriggersProps = Readonly<{
  variant?: "rail" | "header";
  className?: string;
  disabled?: boolean;
}>;

export function BotOverlayTriggers({
  variant = "rail",
  className,
  disabled = false,
}: BotOverlayTriggersProps): JSX.Element {
  const { openRoutines, openSkills, canOpen } = useWorkspaceBotOverlay();
  const inactive = disabled || !canOpen;
  // rail: text links in a row (11px, matches former footer links)
  // header: ComposerIconButton + CalendarClock / Sparkles icons (labels exposed)
  throw new Error("not implemented");
}
```

## Rail layout change

```tsx
// bot-context-rail.tsx — structural diff only

// REMOVE: RailSection "Routines", RailSection "Skills", embedded sidebars
// REMOVE: standalone Files RailSection title at top level
// ADD: RailSection title="Computer" defaultOpen wrapping panel + files tree
// ADD: BotOverlayTriggers between Computer and Memory
```

`RailSection` stays local; no new collapsible for routines/skills.

## Shell wiring

```tsx
// workspace-shell.tsx

<WorkspaceBotOverlayProvider
  botId={selectedBotId}
  conversationId={activeRun?.conversationId ?? null}
  onConflict={() => setSettingsOpen(false)}
>
  {workspaceBody}
  <BotListDialogHost />
  <SettingsDialog ... />
</WorkspaceBotOverlayProvider>
```

```ts
function handleOpenSettings(section: SettingsSection) {
  // close overlay before settings — via ref or context callback registered in provider
  setSettingsSection(section);
  setSettingsOpen(true);
}
```

Provider listens: when `settingsOpen` becomes true, dispatch `close` (or shell calls `overlay.close()` in `handleOpenSettings`).

## Call chain (max depth 3)

1. User clicks **Routines** → `openRoutines()` in context  
2. Reducer → `{ tag: "routines", scope }`  
3. `BotListDialogHost` renders `BotRoutinesSidebar` with polling  

No intermediate controller component beyond provider + host.

## Deliberately not in public API

- Raw `WorkspaceBotOverlay` setter from rail (only `open*` / `close`)
- URL query sync for overlay kind
- Shared fetch cache between routines and skills lists

## Test sketch

```ts
// bot-list-dialog-host.test.tsx
// - render with provider, openRoutines(), expect dialog role + "New routine" link
// - fireEvent click pause, expect cloudHostFetch POST enabled
// - close dialog, expect sidebar unmount (spy clearInterval)

// bot-context-rail.test.tsx
// - expect queryByText("Routines") on trigger button, not collapsible section header for list body
// - expect getByText("Computer"), getByText("Memory")
// - queryByText empty routines copy absent until dialog opened via test helper
```
