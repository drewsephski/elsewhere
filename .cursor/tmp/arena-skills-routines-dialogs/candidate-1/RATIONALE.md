# Rationale

## Problem

The right rail currently stacks Computer, Files, Routines, Skills, and Memory as peer collapsibles. Routines and Skills are catalogs you open, scan, and leave. They poll every 15s while collapsed, which is wasted work for content the user is not looking at. Desktop still mounts a hidden rail on mobile, so a second `BotContextRail` in the bot-details sheet can poll the same lists twice. The product change makes the rail Computer plus Memory, and opens Routines and Skills as compact dialogs. `SettingsDialog` stays out of that path. `/app/skills` and `/app/routines` stay the full editors. Overlay ownership is the hard part. Two rails exist, collapse hides the desktop rail, settings and the mobile sheet already overlay the workspace, and two booleans for two dialogs can both be true.

## Usage (caller's view)

The caller is `WorkspaceShell`. It holds one `TransientChromeOverlay`, dispatches `ChromeEvent`s, and mounts a single `BotLibraryDialog`. `BotConversationView` adds Routines and Skills icon buttons in the conversation header on every viewport, including when the rail is open. `BotContextRail` stops importing the list components. Files render under a static Computer heading. Memory stays the remaining collapsible. Full copy and the three call sites live in `USAGE.md`. The types in `SKETCH.md` are derived from those call sites.

## Shape

The load-bearing structure is `TransientChromeOverlay`, a discriminated union of `none`, `settings`, `library`, and `bot-details`. `reduceChrome` is the only writer. `LibraryOverlay` is a projection the dialog is allowed to see, so the dialog never matches on settings. That follows model-the-domain and type-system-discipline.

`library` always carries `botId`. Switching bots while a catalog is open closes it. Toggle of the same library closes. Toggle of the other library replaces. That is one overlay, not a stack. encode-lessons-in-structure puts the rule in the union instead of a comment on two booleans.

Lists mount only while `overlay.status === "open"`. Unmount is the poll gate. No `active` flag, no second timer. Hidden rails stop fetching because they no longer contain the lists. That follows subtract-before-you-add and laziness-protocol.

Triggers live only in the conversation header. That header is on screen when the rail is open, collapsed, or replaced by the mobile sheet. Putting the dialog inside `BotContextRail` would drop it on collapse and duplicate it when the sheet is open. experience-first and redesign-from-first-principles pick one always-visible trigger row over bolting dialogs onto the old collapsibles.

Interface depth. Callers dispatch events and pass `libraryOverlayFromChrome(chrome)`. They do not coordinate three booleans, pick a list, or know about polling. Complexity that stays with the caller is the two header buttons and the Computer grouping in the rail. The public API is `reduceChrome`, `BotLibraryDialog`, and `LibraryKind`. No `BotLibraryTriggers` wrapper. A wrapper with one caller is a pass-through. That follows minimize-reader-load and boundary-discipline.

The shell remains the single writer. Header and settings URL parsing publish events. They do not share a mutable overlay object. That follows separate-before-serializing-shared-state and foundational-thinking.

Attach and pin stay on the Settings Skills tab. `/app/routines` stays the editor. Create-bot stays outside the union. `SaveAsSkillDialog` stays local to the conversation view and closes when `openLibrary` is set.

## Synthesis decision

orchestrator fills this

## Tradeoffs accepted

- We accept a `WorkspaceShell` reducer refactor for settings and the mobile sheet in exchange for making stacked overlays unrepresentable, instead of only documenting "close the other one".
- We accept two extra header icon buttons, including while the rail is open, in exchange for one trigger location that survives collapse and mobile.
- We accept remounting a list (and refetching) when switching Routines to Skills in exchange for never polling a catalog that is not on screen.
- We accept leaving `BotRoutinesSidebar`'s unused `variant="default"` in place in exchange for not mixing a dead-API deletion into the overlay change.
- We accept `SaveAsSkillDialog` remaining outside the chrome union in exchange for not lifting run-flow state into the shell.

## Alternatives considered

- **Host both catalogs as new `SettingsDialog` tabs.** Callers would open settings to a section. That reuses one overlay, but it exposes the full settings chrome (section list, bot form density) for a scan-and-leave list. Routines have no settings tab today. The dialog would grow. Rejected on interface depth and on the product constraint.
- **Two `useState` booleans and two `Dialog`s, with triggers on the rail and again on the collapsed header.** Callers must keep the flags exclusive, must pass conversation id into both rails, and can still stack on settings. The public API is larger and hides less. Rejected.
- **Keep overlay state inside `BotContextRail` and portal a dialog from whichever rail is mounted.** The desktop aside stays in the DOM at `lg:hidden`, so two rails still exist. Collapse unmounts visibility but not ownership. Callers of the rail would learn the rail's internal open rules. Rejected.

## Open questions and risks

- Should create-bot and create-group join `TransientChromeOverlay` in a follow-up, so a create dialog cannot sit on settings the way it can today?
- Is `BookMarked` the right lucide export for Skills, or does this tree already have a closer icon?
- When a library dialog is open and the user hits Cmd+,, replacing it with settings is the typed behavior. Do we want that, or should Cmd+, be a no-op while a catalog is open?

## Next implementation step

Add `apps/www/lib/workspace-chrome.ts` with `reduceChrome` and reducer unit tests for replace, toggle-close, settings takeover, and bot change.
