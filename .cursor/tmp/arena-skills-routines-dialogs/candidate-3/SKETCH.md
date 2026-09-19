# Type sketch — shell-owned overlay lane (candidate 3)

Derived from [USAGE.md](./USAGE.md). Bodies are `not implemented` / TODO unless noted.

## Module map

| Module | Owns |
| --- | --- |
| `bot-context-overlay.tsx` | Overlay discriminant, provider, host dialog, `useBotContextOverlay` |
| `bot-context-activity-dock.tsx` | Rail + mobile-safe trigger pair |
| `bot-context-header-actions.tsx` | Collapsed-rail header triggers (reuses dock internals) |
| `computer-rail-section.tsx` | Computer live view + optional files tree (single rail block) |
| `bot-context-rail.tsx` | Layout: Computer → Activity dock → Memory; no embedded lists |
| `bot-routines-sidebar.tsx` | Unchanged behavior; optional `mountId` for tests only |
| `bot-skills-sidebar.tsx` | Unchanged behavior |
| `workspace-shell.tsx` | Provider scope, host mount, settings mutual exclusion |
| `bot-conversation-view.tsx` | Header actions when `railCollapsed` |
| `bot-context-overlay.test.tsx` | Overlay state machine + polling gate |
| `bot-context-rail.test.tsx` | Rail section labels + no inline lists |

Call chain from trigger to network (≤3 hops): **Dock → `open()` → Host mounts sidebar → sidebar `load()`**.

## Core data structures

```typescript
// bot-context-overlay.tsx

/** Exactly one bot-context panel at a time; closed is the default. */
export type BotContextPanel = "closed" | "routines" | "skills";

/** Scope frozen when opening — prevents stale botId if user switches bots mid-dialog. */
export type BotContextOverlayScope = {
  botId: string;
  conversationId?: string;
};

export type BotContextOverlayState =
  | { panel: "closed" }
  | { panel: "routines"; scope: BotContextOverlayScope }
  | { panel: "skills"; scope: Pick<BotContextOverlayScope, "botId"> };

/** Provider input: current workspace bot route + optional settings closer. */
export type BotContextOverlayProviderProps = {
  /** When selectedBotId is null, provider still renders children but open() is a no-op. */
  scope: BotContextOverlayScope | null;
  onRequestCloseSettings: () => void;
  children: React.ReactNode;
};

export type BotContextOverlayController = {
  panel: BotContextPanel;
  isOpen: boolean;
  open: (
    panel: Exclude<BotContextPanel, "closed">,
    scope: BotContextOverlayScope,
  ) => void;
  close: () => void;
};

/** Invariants (encode-lessons-in-structure):
 * - panel !== 'closed' ⇒ scope.botId is set on state variant
 * - At most one of SettingsDialog and BotContextOverlayHost is open (shell policy)
 * - Switching panel: routines → skills replaces state in one setState (no double mount)
 * - selectedBotId change while open ⇒ provider resets to { panel: 'closed' }
 */
```

```typescript
// computer-rail-section.tsx

export type ComputerRailSectionProps = {
  bot: BotSummary;
  activeRun: RunSummary | null;
  /** Files nested under Computer when computerId present (defaultOpen). */
  className?: string;
};
```

## Public API signatures

```typescript
// bot-context-overlay.tsx

export function BotContextOverlayProvider(
  props: BotContextOverlayProviderProps,
): JSX.Element {
  // TODO: useReducer or useState<BotContextOverlayState>
  // TODO: when scope?.botId changes and state.panel !== 'closed', dispatch close
  // TODO: open() calls onRequestCloseSettings() before opening
  throw new Error("not implemented");
}

export function useBotContextOverlay(): BotContextOverlayController {
  // TODO: context; throw if missing provider in dev
  throw new Error("not implemented");
}

export type BotContextOverlayHostProps = {
  botId: string;
  conversationId?: string;
};

export function BotContextOverlayHost(
  props: BotContextOverlayHostProps,
): JSX.Element {
  // const { panel, close } = useBotContextOverlay();
  // return (
  //   <Dialog open={panel !== 'closed'} onOpenChange={(next) => !next && close()}>
  //     <DialogContent className="gap-3 p-4 sm:max-w-lg max-h-[min(72dvh,32rem)] overflow-y-auto">
  //       {panel === 'routines' ? (
  //         <>
  //           <DialogHeader>...</DialogHeader>
  //           <BotRoutinesSidebar botId={...} conversationId={...} variant="minimal" />
  //         </>
  //       ) : null}
  //       {panel === 'skills' ? (
  //         <>
  //           <DialogHeader>...</DialogHeader>
  //           <BotSkillsSidebar botId={...} variant="minimal" />
  //         </>
  //       ) : null}
  //     </DialogContent>
  //   </Dialog>
  // );
  throw new Error("not implemented");
}
```

```typescript
// bot-context-activity-dock.tsx

export type BotContextActivityDockProps = {
  botId: string;
  conversationId?: string;
  className?: string;
};

export function BotContextActivityDock(
  props: BotContextActivityDockProps,
): JSX.Element {
  // TODO: useBotContextOverlay(); toggle on repeated press (per boundary-discipline: no extra state)
  throw new Error("not implemented");
}
```

```typescript
// bot-context-header-actions.tsx

/** Thin wrapper — same buttons as dock, sized for conversation header. */
export function BotContextHeaderActions(
  props: BotContextActivityDockProps,
): JSX.Element {
  throw new Error("not implemented");
}
```

```typescript
// computer-rail-section.tsx

export function ComputerRailSection(props: ComputerRailSectionProps): JSX.Element {
  // TODO:
  // <RailSection title="Computer" defaultOpen>
  //   <ComputerStatePanel variant="minimal" />
  //   {bot.computerId ? <ComputerWorkspaceTree ... /> : null}
  // </RailSection>
  throw new Error("not implemented");
}
```

```typescript
// bot-context-rail.tsx — signature unchanged; body loses Routines/Skills RailSections

export function BotContextRail(props: BotContextRailProps): JSX.Element {
  throw new Error("not implemented");
}
```

## Sidebar polling (mount-gated, no new public props)

Keep existing components; rely on **conditional mount** in the host.

```typescript
// bot-routines-sidebar.tsx — internal only

useEffect(() => {
  void load();
  const timer = setInterval(() => void load(), 15_000);
  return () => clearInterval(timer);
}, [load]);
// Invariant: effect runs only while Host renders this component (dialog open).
// No polling when panel closed — per subtract-before-you-add, no liveRefresh prop required.
```

If tests need determinism, mock timers at overlay test layer rather than adding production flags.

## Shell integration (reducer policy)

```typescript
// workspace-shell.tsx — pseudocode

function handleOpenSettings(section: SettingsSection) {
  // TODO: overlayController.close() if hook exposed via ref or callback registration
  setSettingsSection(section);
  setSettingsOpen(true);
}

// Settings onOpenChange(true) should also close bot context overlay (symmetric mutual exclusion).
```

Optional narrow helper to avoid duplicating policy:

```typescript
export type WorkspaceModalPolicy = {
  closeBotContextOverlay: () => void;
  closeSettings: () => void;
};

// Registered once by provider + settings owner — not exported to feature code.
```

## Rail layout after change

```
BotContextRail
├── header (collapse, settings)
└── scroll
    ├── ComputerRailSection          ← was bare panel + separate Files section
    ├── BotContextActivityDock       ← replaces Routines + Skills collapsibles
    └── RailSection "Memory"
```

## Dialog copy (load-bearing UX)

| Panel | Title | Description (xs muted) |
| --- | --- | --- |
| routines | Routines | Recurring work for this bot. Describe schedules in chat or use the links below. |
| skills | Skills | Skills attached to this bot. Save from chat or attach versions in settings. |

Empty/list/link behaviors remain owned by sidebar components.

## Test sketch

```typescript
// bot-context-overlay.test.tsx

describe("BotContextOverlayHost", () => {
  it("mounts routines list only while open", () => { /* ... */ });
  it("toggle closes on second routines click", () => { /* ... */ });
  it("switching skills while routines open keeps single dialog", () => { /* ... */ });
  it("does not poll after close", async () => {
    // vi.useFakeTimers(); open dialog; advance 20s; close; advance 20s; expect fetch count bounded
  });
});

// bot-context-rail.test.tsx

it("shows Computer and Memory without inline routine rows", () => {
  expect(screen.getByText("Computer")).toBeTruthy();
  expect(screen.getByText("Memory")).toBeTruthy();
  expect(screen.queryByText("Advanced routines")).toBeNull();
  expect(screen.getByRole("button", { name: "Routines" })).toBeTruthy();
});
```

## Deliberately out of scope

- Rewriting `/app/skills` or `/app/routines`
- Attach/pin UI inside skills dialog (stays in `SettingsDialog` Skills tab)
- URL/query-param deep links to overlay panel
- Second dialog stack or sheet-based overlay on mobile (still `Dialog` for parity with create-bot)
