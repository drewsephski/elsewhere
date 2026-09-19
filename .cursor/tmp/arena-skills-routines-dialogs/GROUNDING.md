# Grounding: skills/routines leave the right rail

Product ask (verbatim intent): replace routines and skills content in the right sidebar with popup dialogs. The rail's two main areas become Computer and Memory. Skills and Routines become minimal, responsive dialogs.

## How the current shell works

The signed-in chat shell is `WorkspaceShell` (`apps/www/components/app/workspace/workspace-shell.tsx`).

Desktop layout:

- Left: `BotListSidebar`
- Center: `BotConversationView`
- Right: `<aside aria-label="Bot context">` hosting `BotContextRail`, hidden below `lg` or when `railCollapsed`

Mobile:

- Conversation header button `Bot details` (`lg:hidden`) opens `MobileSheet`
- The sheet mounts a second `BotContextRail` (no collapse control)

Conversation header (`bot-conversation-view.tsx` ~703-736):

- New chat
- Bot settings (only when `railCollapsed`, `hidden lg:flex`)
- Bot details (mobile sheet)
- Expand sidebar (when collapsed)

Rail header (`bot-context-rail.tsx`):

- Collapse sidebar
- Bot settings

`SettingsDialog` already exists and has a Skills tab (`BotSkillsSettings`: attach/detach, pin version). Routines have no settings tab. Full editors live at `/app/routines` and `/app/skills` (legacy chrome).

## Current rail body

`BotContextRail` stacks:

1. `ComputerStatePanel` variant `minimal` (browser preview + ready/manage) always
2. If bot: `Files` collapsible (`ComputerWorkspaceTree`) when `bot.computerId`
3. If bot: `Routines` collapsible wrapping `BotRoutinesSidebar` variant `minimal`
4. If bot: `Skills` collapsible wrapping `BotSkillsSidebar` variant `minimal`
5. If bot: `Memory` collapsible wrapping `BotMemoryPanel` `embedded`

`RailSection` is a local `Collapsible` with 12px title + chevron. Routines/Skills default closed. Files and... wait, Files defaultOpen, Routines/Skills/Memory default closed (`defaultOpen = false` except Files).

## Skills list today

`BotSkillsSidebar` is only used from the rail.

- GET `/v1/bots/${botId}/skills` every 15s
- Empty: "No skills attached yet. Save successful work from chat or manage skills in settings."
- List: name + version
- Link: Manage skills -> `/app/skills`
- No attach UI (that's settings)

`SaveAsSkillDialog` is a separate chat-run flow, not this list.

## Routines list today

`BotRoutinesSidebar` is only used from the rail. `variant="default"` exists but has no caller.

- GET `/v1/routines`, filter `routine.botId === botId`, poll 15s
- Toggle enable via POST `/v1/routines/${id}/enabled`
- Empty (minimal): tell the bot in chat, example weekday 8 AM brief
- Links: New routine (`/app/routines?bot=...&conversation=...`), Advanced routines when any exist

## Memory / Computer stay

`ComputerStatePanel` + files tree + `BotMemoryPanel` (pinned context, learn toggle, search, edit/forget) remain the rail's content. Files belong with Computer, not as a third peer of Memory.

## Established dialog pattern

`apps/www/components/ui/dialog.tsx` is Base UI Dialog.

- Overlay: `bg-black/30 backdrop-blur-[2px]`
- Content: `rounded-2xl bg-card p-5 ... max-w-[calc(100%-2rem)] sm:max-w-md`
- Close button top-right, `aria-label="Close"`

`CreateBotDialog` is the closest compact dialog: `p-4 sm:max-w-lg`, title `text-base`, description `text-xs`.

`SettingsDialog` is a large tabbed settings surface. Do not dump skills/routines into it as the primary replacement. Settings Skills tab can remain for attach/pin.

`MobileSheet` is a custom bottom sheet (`lg:hidden`, `max-h-[92dvh]`, rounded top). There is no shadcn `Sheet` in this tree.

## Tests that will break if ignored

`bot-context-rail.test.tsx` asserts visible text `Routines`, `Files`, `Memory`. After this change it must assert Computer + Memory (and Files under computer), and that skills/routines lists are not inline. Dialog open/close needs tests.

## Constraints

- Do not rewrite `/app/skills` or `/app/routines` managers.
- Do not put attach/pin complexity into the skills popup unless a candidate argues it belongs (settings already has it).
- Keep 15s polling only while a dialog is open if that is cheaper; polling while closed is waste.
- One overlay at a time. Two stacked dialogs is wrong.
- Triggers must work when the rail is open, collapsed, and on mobile.
- Reuse `Dialog` + existing list behavior. Smallest public surface.
- No comments that narrate phases.
- Types first. Discriminated overlay vs two booleans is a real fork. Pick one and encode it.

## Files a candidate should treat as the contract

- `apps/www/components/app/workspace/bot-context-rail.tsx`
- `apps/www/components/app/workspace/bot-context-rail.test.tsx`
- `apps/www/components/app/workspace/bot-routines-sidebar.tsx`
- `apps/www/components/app/workspace/bot-skills-sidebar.tsx`
- `apps/www/components/app/workspace/workspace-shell.tsx`
- `apps/www/components/app/workspace/bot-conversation-view.tsx`
- `apps/www/components/ui/dialog.tsx`
- `apps/www/components/app/workspace/create-bot-dialog.tsx` (size/tone)
- `apps/www/components/app/settings-dialog.tsx` (do not enlarge this for the primary UX)
- `apps/www/components/app/bot-memory.tsx`
- `apps/www/components/app/workspace/computer-state-panel.tsx`
