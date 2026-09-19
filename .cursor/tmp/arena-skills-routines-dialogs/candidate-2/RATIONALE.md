# Rationale — candidate 2 (shell overlay slot + unified list dialog)

## Problem

The right rail embeds full Routines and Skills lists inside collapsibles, which competes with Computer and Memory for scarce width and keeps 15s polling alive even when nobody is looking at those lists. Product wants the rail to foreground **Computer** (live view plus files) and **Memory**, while Routines and Skills open in minimal responsive dialogs that preserve today’s list behaviors (pause/resume, empty copy, deep links to `/app/routines` and `/app/skills`, skill name/version lines) without moving attach/pin into the popup or enlarging `SettingsDialog`. Triggers must work with the rail open, collapsed, or on mobile inside `MobileSheet`, and only one modal overlay may be active at a time.

## Usage (caller's view)

See [USAGE.md](./USAGE.md). Callers use `useWorkspaceBotOverlay()` and `BotOverlayTriggers`; the shell mounts `WorkspaceBotOverlayProvider` and a single `BotListDialogHost`. The rail stops rendering sidebar list components inline.

## Shape

**Single overlay slot as a discriminated union** (`WorkspaceBotOverlay`) lives in a tiny pure reducer module and is held in React context scoped to `WorkspaceShell`. Opening routines or skills replaces the slot; `close` clears it. This encodes “one overlay at a time” in the type system instead of coordinating two booleans or risking stacked dialogs (`encode-lessons-in-structure`).

**Interface depth:** Triggers and the conversation header call three methods (`openRoutines`, `openSkills`, `close`) and read `canOpen`. They do not import `Dialog`, poll intervals, or fetch paths. The dialog host hides dialog chrome, copy, and which sidebar mounts; sidebars keep all list policy and API wiring (`boundary-discipline` — validate scope at provider boundary, trust types inside).

**Unified `BotListDialogHost`:** One dialog component switches body by overlay tag. Avoids duplicated overlay/backdrop logic and guarantees mutual exclusion when switching lists (`single source of truth` for modal presentation).

**Rail restructure:** One **Computer** collapsible wraps `ComputerStatePanel` and the files tree (files are not a peer top-level section anymore). **Memory** stays a collapsible. **BotOverlayTriggers** sits between them as a fixed action row, not a third collapsible with embedded lists.

**Polling:** `pollWhileOpen` on existing sidebar components defaults false; the dialog host passes true so 15s refresh runs only while mounted in an open dialog. Unmount on close stops timers without a global poll manager.

**Settings conflict:** Opening bot settings closes the bot list overlay (and optionally settings close is unchanged). Prevents two full-screen modals (`one overlay at a time` extended across features).

**Complexity hidden from callers:** Scope binding when `selectedBotId` or `conversationId` changes, reducer transitions, dialog sizing (`sm:max-w-lg`, scroll cap like `CreateBotDialog`), and panel choice. **Exposed:** trigger placement variants (`rail` vs `header`) and shell provider props.

## Synthesis decision

orchestrator fills this

## Tradeoffs accepted

- We accept a React context dependency for triggers in both rail and conversation header in exchange for not threading `onOpenRoutines` props through five layers.
- We accept one slightly wider dialog (`sm:max-w-lg`) for routines rows in exchange for keeping existing minimal list markup without a new compact card layout.
- We accept default-off polling on sidebar components in exchange for explicit opt-in from the dialog host and no background fetch when lists are not shown.
- We accept duplicating trigger UI in two places (rail row + header icons) in exchange for guaranteed access when the rail is collapsed or mobile-only.

## Alternatives considered

- **URL search params (`?panel=routines`)** — Deep-linkable and shareable, but exposes overlay wire protocol to the router, complicates back-button behavior with `SettingsDialog`, and leaks transport into every trigger (`information leakage`). Loses on interface depth for a feature that is ephemeral UI state.

- **Two independent dialog components + two booleans in `WorkspaceShell`** — Familiar React pattern, but callers or shell must remember to close the other flag; tests must cover four combinations. Union slot is smaller public mental model with the same implementation cost.

- **Expand `SettingsDialog` with Routines tab** — Reuses existing modal, but violates product constraint, mixes attach/pin settings with read-only rail lists, and hides routines behind a gear icon when the rail is open (`shallow module` — large settings surface, little new policy).

- **Inline lists in a bottom `MobileSheet` only on small screens while desktop uses dialogs** — Two presentation paths for the same data, doubles test matrices and risks divergent empty copy (`temporal decomposition` by viewport instead of by domain).

## Open questions and risks

- Should opening **Bot details** `MobileSheet` auto-close an open routines/skills dialog, or can the sheet sit above the dialog on mobile?
- When the user switches bots via the left sidebar while a list dialog is open, is closing immediately acceptable, or should we rebind scope and keep the same list type open?
- Header trigger icons: reuse `CalendarClock` / a skills glyph vs text-only on mobile for touch targets?
- `BotSkillsSidebar` early-return empty state currently skips the “Manage skills” link; should the dialog host normalize empty layout so the link always shows (behavior parity audit)?

## Next implementation step

Add `lib/workspace-bot-overlay.ts` and `WorkspaceBotOverlayProvider`, mount `BotListDialogHost` in `workspace-shell.tsx`, then refactor `bot-context-rail.tsx` to the Computer/Memory layout with `BotOverlayTriggers` before touching sidebar polling props.
