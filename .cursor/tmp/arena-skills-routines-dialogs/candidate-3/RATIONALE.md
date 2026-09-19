# Rationale — shell-owned overlay lane (candidate 3)

## Problem

The right rail embeds full Routines and Skills lists inside collapsibles, which competes with Computer and Memory for scarce vertical space and keeps 15s polling alive even when nobody is looking at those lists. The product ask is to keep Computer (live view + files) and Memory in the rail while moving Routines and Skills into minimal responsive dialogs, without rewriting the advanced `/app/*` managers or making Settings the primary skills/routines surface. Triggers must work when the rail is open, collapsed, or replaced by the mobile bot-details sheet, and only one modal layer may be visible at a time.

## Usage (caller's view)

See [USAGE.md](./USAGE.md). Callers wrap the shell in `BotContextOverlayProvider`, mount `BotContextOverlayHost` once, place `BotContextActivityDock` on the rail, and mirror triggers in `BotContextHeaderActions` when the rail is collapsed. Lists are not imported in the rail; `open("routines" | "skills", scope)` is the only action surface.

## Shape

**Single discriminated overlay state** (`BotContextOverlayState`) lives in a provider scoped to the active bot route. The public controller exposes `panel`, `open`, and `close` — not separate booleans — so “one overlay at a time” is a type invariant, not a runtime guard scattered across components (`encode-lessons-in-structure`).

**Shell ownership** places the `Dialog` host beside `SettingsDialog`. Opening routines/skills closes settings first; opening settings closes the overlay. That concentrates modal policy in one place instead of leaking “close the other thing” into every trigger (`boundary-discipline`, deep module).

**Mount-gated polling**: `BotRoutinesSidebar` and `BotSkillsSidebar` render only while the host dialog is open, so existing `useEffect` intervals need no new props — unmount stops polls (`subtract-before-you-add`).

**Rail regrouping**: `ComputerRailSection` folds `ComputerStatePanel` and `ComputerWorkspaceTree` under one **Computer** section so Files are not a third top-level peer of Memory. **Activity dock** (icon buttons) replaces Routines/Skills collapsibles; behavior stays in the unchanged sidebar components inside the dialog.

**Interface depth**: Callers know three symbols (provider, host, hook) and two presentational triggers. They do not choose dialog sizes, polling, or mutual exclusion with settings — the overlay module hides that policy behind `open()` / `close()`.

## Synthesis decision

orchestrator fills this

## Tradeoffs accepted

- We accept a React context dependency from conversation header and rail into shell-scoped overlay state in exchange for triggers that work when the rail is hidden or only present inside `MobileSheet`.
- We accept symmetric close rules between settings and bot-context overlays in exchange for never stacking two `Dialog` roots (`one overlay at a time`).
- We accept `Dialog` on mobile instead of a bottom sheet in exchange for one visual system and less custom sheet code (`Dialog` already matches create-bot tone).
- We accept resetting overlay to closed on bot route change in exchange for never showing another bot’s routines/skills list by mistake.
- We accept a new `ComputerRailSection` module in exchange for a clear product boundary (Computer vs Memory vs activity dialogs) without a large rewrite of memory or computer panels.

## Alternatives considered

- **Inline dialog components inside `BotContextRail` only** — rejected because collapsed desktop rail and mobile sheet duplication would either duplicate dialog roots or fail to show triggers when the rail is not visible; callers would need to know rail layout rules (`information leakage`).
- **URL search params (`?panel=routines`)** — rejected because it exposes overlay transport to routing, complicates back-button semantics with `SettingsDialog`, and widens the public API for a transient UI (`boundary-discipline`).
- **Expand `SettingsDialog` with Routines/Skills tabs** — rejected per product constraint; settings skills tab remains for attach/pin, not primary list UX.
- **Single `WorkspaceModalKind` enum merging settings + routines + skills** — rejected as over-merge for this slice; mutual exclusion callbacks achieve the same invariant with smaller churn to existing settings state.
- **Keep lists in rail but collapsed by default** — rejected; fails the product ask and keeps wasteful polling unless additional gating props are added anyway.

## Open questions and risks

- Should opening the mobile bot-details sheet auto-close an already-open routines/skills dialog, or is dialog-over-sheet acceptable visually on small viewports?
- When `SettingsDialog` opens from the rail gear while a routines dialog is open, is immediate close of the overlay the right UX, or should settings open be blocked with a toast?
- Are `CalendarClock` / `Sparkles` (or existing icon set equivalents) the right dock affordances, or should labels be visible text on narrow rails?
- Do we need an analytics/event hook on `open()` for product metrics, or is that out of scope for the first ship?

## Next implementation step

Add `bot-context-overlay.tsx` with provider, hook, and host rendering `BotRoutinesSidebar` / `BotSkillsSidebar`, wire `BotContextOverlayProvider` and host in `workspace-shell.tsx`, then refactor `bot-context-rail.tsx` to `ComputerRailSection` + activity dock and update tests.
