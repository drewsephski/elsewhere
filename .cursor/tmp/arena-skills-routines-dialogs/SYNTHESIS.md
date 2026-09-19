# Synthesis. Skills and routines dialogs

## Pick

Base: **candidate-1**. Cross-judge ([CROSS-JUDGE.md](./CROSS-JUDGE.md)) totals 29 / 22 / 21. Parent scored the same base before the judge returned: header-only triggers, `TransientChromeOverlay` union, unmount as poll gate, no overlay context.

Agreement on the base.

## Grafts

- Skills icon: existing `Sparkles` from `apps/www/components/icons/lucide.tsx`. This tree has no `BookMarked` export.
- Dialog descriptions from candidate-3 (closer to current empty-state intent):
  - Routines: `Recurring work for this Bot. Describe schedules in chat or use the links below.`
  - Skills: `Skills attached to this Bot. Save from chat or attach versions in settings.`
- Optional `conversationId` on the library chrome variant, supplied from `BotConversationView` when known, so New routine links still prefill. Candidate-1 left this on the shell's synthetic `activeRun`, which often has no `conversationId`.

## Rejected

- Candidate-2 context, `pollWhileOpen`, rail+header trigger pair, list-only overlay union.
- Candidate-3 rail dock, `BotContextHeaderActions` pass-through, `ComputerRailSection` extract, mobile header hidden.

## Contract for implementation

Follow `candidate-1/SKETCH.md` and `candidate-1/USAGE.md` with the grafts above. `reduceChrome` is real code in `apps/www/lib/workspace-chrome.ts`, not comments that restate cases.

## Verification of this synthesis

The synthesized public surface is still: shell holds chrome, conversation header toggles library, one `BotLibraryDialog`, rail is Computer+Files+Memory. That matches the product ask and both independent scores.
