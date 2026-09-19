# Sketch. Bot library overlay

## Module map

```
apps/www/lib/workspace-chrome.ts
	TransientChromeOverlay, LibraryOverlay, ChromeEvent
	reduceChrome, libraryOverlayFromChrome
	single writer for settings, library, bot-details

apps/www/components/app/workspace/bot-library-dialog.tsx
	BotLibraryDialog
	one Dialog, body chosen by overlay.library
	mounts BotRoutinesSidebar or BotSkillsSidebar only while open

apps/www/components/app/workspace/workspace-shell.tsx
	useState overlay, dispatch(reduceChrome)
	mounts BotLibraryDialog once
	projects chrome into SettingsDialog, MobileSheet, conversation callbacks

apps/www/components/app/workspace/bot-conversation-view.tsx
	Routines and Skills ComposerIconButtons in the header, always
	closes SaveAsSkillDialog when a library overlay is active

apps/www/components/app/workspace/bot-context-rail.tsx
	Computer group (preview plus Files) and Memory
	deletes Routines and Skills RailSections

apps/www/components/app/workspace/bot-routines-sidebar.tsx
apps/www/components/app/workspace/bot-skills-sidebar.tsx
	unchanged list behavior
	poll 15s for as long as the component is mounted
```

The call chain is three hops.

1. Header click calls `onToggleLibrary`.
2. Shell `dispatch` runs `reduceChrome`.
3. `BotLibraryDialog` reads `libraryOverlayFromChrome(chrome)` and mounts one list.

A fourth file in that path is a smell. Do not add a `BotLibraryTriggers` wrapper.

## Domain types

```ts
export type LibraryKind = "routines" | "skills";

/**
 * Projection the dialog understands.
 * Closed has no botId and no library. Open always has both.
 */
export type LibraryOverlay =
	| { status: "closed" }
	| { status: "open"; library: LibraryKind; botId: string };

/**
 * Exclusive bot-context overlay. Illegal combinations cannot be stored.
 * Create-bot and create-group stay outside this union.
 * railCollapsed is layout, not chrome.
 */
export type TransientChromeOverlay =
	| { kind: "none" }
	| { kind: "settings"; section: SettingsSection }
	| { kind: "library"; library: LibraryKind; botId: string }
	| { kind: "bot-details" };

export type ChromeEvent =
	| { type: "close" }
	| { type: "toggle-library"; library: LibraryKind; botId: string }
	| { type: "open-settings"; section: SettingsSection }
	| { type: "set-settings-section"; section: SettingsSection }
	| { type: "open-bot-details" }
	| { type: "bot-changed"; botId: string | null };
```

Invariants encoded in the types.

- Two catalogs cannot be open together. There is one `library` field.
- Settings, library, and bot-details cannot be open together. They are variants of one union.
- A library overlay without a `botId` does not compile.
- `set-settings-section` only makes sense while settings is open. The reducer ignores it otherwise, so a closed overlay cannot hold a dangling section.

## Constructors and reducer

```ts
export function closedLibrary(): LibraryOverlay {
	return { status: "closed" };
}

export function libraryOverlayFromChrome(
	chrome: TransientChromeOverlay,
): LibraryOverlay {
	if (chrome.kind === "library") {
		return {
			status: "open",
			library: chrome.library,
			botId: chrome.botId,
		};
	}
	return closedLibrary();
}

/**
 * Total function. The shell is the only caller.
 * Toggle of the active library closes. Toggle of the other library replaces.
 * Every open event leaves at most one overlay kind.
 */
export function reduceChrome(
	state: TransientChromeOverlay,
	event: ChromeEvent,
): TransientChromeOverlay {
	switch (event.type) {
		case "close":
			return { kind: "none" };
		case "toggle-library": {
			if (
				state.kind === "library" &&
				state.library === event.library &&
				state.botId === event.botId
			) {
				return { kind: "none" };
			}
			return {
				kind: "library",
				library: event.library,
				botId: event.botId,
			};
		}
		case "open-settings":
			return { kind: "settings", section: event.section };
		case "set-settings-section":
			if (state.kind !== "settings") {
				return state;
			}
			return { kind: "settings", section: event.section };
		case "open-bot-details":
			return { kind: "bot-details" };
		case "bot-changed": {
			if (state.kind === "library") {
				if (!event.botId || state.botId !== event.botId) {
					return { kind: "none" };
				}
			}
			if (state.kind === "bot-details" && !event.botId) {
				return { kind: "none" };
			}
			return state;
		}
		default: {
			const _exhaustive: never = event;
			return state;
		}
	}
}
```

Bodies above are the spec. Fill them in `workspace-chrome.ts` as real code, not comments that restate the cases.

Reducer tests live next to the module and cover, without DOM:

- open routines, then skills, still one library overlay
- toggle routines twice, ends at `none`
- open library, then settings, library is gone
- bot change while library is open for another id, closes
- `set-settings-section` while `none`, unchanged

## Dialog

```tsx
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import type { LibraryKind, LibraryOverlay } from "@/lib/workspace-chrome";
import { BotRoutinesSidebar } from "./bot-routines-sidebar";
import { BotSkillsSidebar } from "./bot-skills-sidebar";

const COPY: Record<
	LibraryKind,
	{ title: string; description: string }
> = {
	routines: {
		title: "Routines",
		description: "Scheduled work for this Bot.",
	},
	skills: {
		title: "Skills",
		description: "Playbooks attached to this Bot.",
	},
};

export function BotLibraryDialog({
	overlay,
	conversationId,
	onClose,
}: {
	overlay: LibraryOverlay;
	conversationId?: string;
	onClose: () => void;
}) {
	const open = overlay.status === "open";
	const copy = open ? COPY[overlay.library] : COPY.routines;

	return (
		<Dialog open={open} onOpenChange={(next) => {
			if (!next) onClose();
		}}>
			<DialogContent className="gap-3 p-4 sm:max-w-lg">
				<DialogHeader className="gap-1 pr-8">
					<DialogTitle className="text-base">{copy.title}</DialogTitle>
					<DialogDescription className="text-xs">
						{copy.description}
					</DialogDescription>
				</DialogHeader>
				{overlay.status === "open" ? (
					<div className="max-h-[min(28rem,70dvh)] overflow-y-auto">
						{overlay.library === "routines" ? (
							<BotRoutinesSidebar
								botId={overlay.botId}
								conversationId={conversationId}
								variant="minimal"
							/>
						) : (
							<BotSkillsSidebar
								botId={overlay.botId}
								variant="minimal"
							/>
						)}
					</div>
				) : null}
			</DialogContent>
		</Dialog>
	);
}
```

Match `CreateBotDialog` density (`p-4`, title `text-base`, description `text-xs`). Keep the shared Dialog overlay (`bg-black/30 backdrop-blur-[2px]`, `rounded-2xl`, close `aria-label="Close"`). Do not use `MobileSheet` for these catalogs.

`{overlay.status === "open" ? list : null}` is the poll gate. Do not add an `active` prop to the sidebars.

## Conversation header

```tsx
// BotConversationViewProps gains:
onToggleLibrary?: (library: LibraryKind) => void;
openLibrary?: LibraryKind | null;

// In the header cluster, after New chat, before settings:
{onToggleLibrary ? (
	<>
		<ComposerIconButton
			label="Routines"
			className="size-7"
			aria-pressed={openLibrary === "routines"}
			onClick={() => {
				setSaveSkillRun(null);
				onToggleLibrary("routines");
			}}
		>
			<CalendarClock className="size-4" aria-hidden />
		</ComposerIconButton>
		<ComposerIconButton
			label="Skills"
			className="size-7"
			aria-pressed={openLibrary === "skills"}
			onClick={() => {
				setSaveSkillRun(null);
				onToggleLibrary("skills");
			}}
		>
			<BookMarked className="size-4" aria-hidden />
		</ComposerIconButton>
	</>
) : null}
```

`SaveAsSkillDialog` is already gated on `saveSkillRun`. Clear that run when a library overlay is active.

```tsx
useEffect(() => {
	if (openLibrary) {
		setSaveSkillRun(null);
	}
}, [openLibrary]);
```

TODO: pick `BookMarked` or the nearest existing lucide export. Do not add a new icon package.

Settings stays `hidden lg:flex` and gated on `railCollapsed`. Routines and Skills do not copy that gate.

## Rail

Delete the Routines and Skills `RailSection` blocks and their imports.

Computer is a static heading, not a `RailSection`. The live preview stays visible. Files remain a `RailSection` with `defaultOpen`, nested under that heading, only when `bot.computerId` is set. Memory stays a `RailSection` with `defaultOpen` false.

`RailSection` stays local to this file. Do not extract a Computer component for one heading.

## Shell exclusivity with create flows

```tsx
onCreateBot={() => {
	dispatch({ type: "close" });
	setCreateOpen(true);
}}
```

Same for create-group. `reduceChrome` does not grow a create-bot variant in this change.

## Lists

No signature change required on `BotRoutinesSidebar` or `BotSkillsSidebar`. They already poll in `useEffect` on mount and clear the interval on unmount.

Leave `variant="default"` on routines in place. It has no caller. Deleting it is a later cleanup, not this overlay.

Do not fetch in the dialog. The dialog chooses which list exists.

## Tests

`bot-context-rail.test.tsx`

- expect `Computer`, `Files`, `Memory`
- expect no rail heading `Routines` or `Skills`
- gear still calls `onOpenSettings`

New `workspace-chrome.test.ts`

- reducer cases listed above

New `bot-library-dialog.test.tsx`

- closed overlay. query by dialog title is empty, sidebar empty copy is absent
- open routines. weekday-brief empty copy is present (mock `/v1/routines` as today)
- open routines then dispatch skills. one dialog, skills empty copy, routines copy gone
- header in `BotConversationView` still exposes Routines when `railCollapsed` is true

## Deliberately not in this sketch

- Attach, detach, and pin version. Settings Skills tab.
- Full routine editor. `/app/routines`.
- Unifying create-bot into `TransientChromeOverlay`.
- A poll `enabled` flag on the sidebars.
- A rail footer that restates Routines and Skills.
