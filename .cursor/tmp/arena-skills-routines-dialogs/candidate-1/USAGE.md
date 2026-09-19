# Bot library dialogs

Routines and Skills leave the right rail. You open each as a compact `Dialog`. The rail keeps two areas only: Computer (live view plus files) and Memory.

The conversation header opens the catalogs. The rail shows the live computer and memory.

## What you import

```ts
import {
	reduceChrome,
	libraryOverlayFromChrome,
	type ChromeEvent,
	type TransientChromeOverlay,
	type LibraryKind,
	type LibraryOverlay,
} from "@/lib/workspace-chrome";
import { BotLibraryDialog } from "@/components/app/workspace/bot-library-dialog";
```

You do not import a second dialog. You do not add `routinesOpen` and `skillsOpen` booleans. You do not mount `BotRoutinesSidebar` or `BotSkillsSidebar` in `BotContextRail`.

`WorkspaceShell` is the only writer of chrome state. Header buttons dispatch events. The dialog reads a projection of that state.

## How a caller opens a catalog

1. Dispatch `toggle-library` with the kind and the current bot id.
2. The reducer closes settings and the mobile bot-details sheet in the same transition.
3. `BotLibraryDialog` mounts the matching list. The list starts its 15s poll because it is mounted.
4. Close sets chrome to `none`. The list unmounts and the poll stops.

Clicking Routines while Routines is already open closes it. Clicking Skills while Routines is open replaces the body. There is still one dialog.

## Call site 1. Shell owns chrome and mounts one dialog

`WorkspaceShell` replaces `settingsOpen`, `settingsSection`, and `contextSheetOpen` with one overlay value. Create-bot and create-group stay as they are. Rail collapse is not an overlay.

```tsx
const [chrome, setChrome] = useState<TransientChromeOverlay>({ kind: "none" });

function dispatch(event: ChromeEvent) {
	setChrome((current) => reduceChrome(current, event));
}

useEffect(() => {
	dispatch({ type: "bot-changed", botId: selectedBotId });
}, [selectedBotId]);

// Cmd+, and ?settings= both dispatch { type: "open-settings", section }

<BotConversationView
	onOpenContext={() => dispatch({ type: "open-bot-details" })}
	onOpenSettings={(section) =>
		dispatch({ type: "open-settings", section: section ?? "general" })
	}
	onToggleLibrary={(library) => {
		if (!bot) return;
		dispatch({ type: "toggle-library", library, botId: bot.id });
	}}
	openLibrary={
		chrome.kind === "library" ? chrome.library : null
	}
/>

<BotLibraryDialog
	overlay={libraryOverlayFromChrome(chrome)}
	conversationId={activeRun?.conversationId}
	onClose={() => dispatch({ type: "close" })}
/>

<MobileSheet
	open={chrome.kind === "bot-details"}
	onClose={() => dispatch({ type: "close" })}
>
	<BotContextRail ... />
</MobileSheet>

<SettingsDialog
	open={chrome.kind === "settings"}
	section={chrome.kind === "settings" ? chrome.section : "general"}
	onOpenChange={(open) => {
		if (!open) dispatch({ type: "close" });
	}}
	onSectionChange={(section) =>
		dispatch({ type: "set-settings-section", section })
	}
/>
```

Mount `BotLibraryDialog` once, next to `SettingsDialog`. Do not put it inside `BotContextRail`. The desktop rail and the mobile sheet each mount a rail. A dialog inside the rail would exist twice, and would vanish when the rail collapses.

When you open create-bot or create-group, dispatch `close` first so the library dialog cannot sit under that overlay.

## Call site 2. Conversation header is the only trigger

The header is visible when the rail is open, when it is collapsed, and on mobile. Put the two buttons there, always, next to New chat. Do not hide them behind `railCollapsed`. Do not duplicate them in the rail header.

```tsx
<ComposerIconButton
	label="Routines"
	className="size-7"
	aria-pressed={openLibrary === "routines"}
	onClick={() => {
		setSaveSkillRun(null);
		onToggleLibrary?.("routines");
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
		onToggleLibrary?.("skills");
	}}
>
	<BookMarked className="size-4" aria-hidden />
</ComposerIconButton>
```

Render them only after a bot id exists. `onToggleLibrary` is a no-op in the shell until `bot` is loaded.

If `SaveAsSkillDialog` is open, set `saveSkillRun` to `null` in the same click handler before `onToggleLibrary`. Also clear `saveSkillRun` when `openLibrary` is set. That run-flow dialog stays local to the conversation view. It is not a chrome variant.

## Call site 3. Rail is Computer plus Memory

`BotContextRail` no longer imports the two sidebars. Files sit under Computer. Memory stays a collapsible, default closed.

```tsx
<div className="min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto px-3 pb-3">
	<section aria-labelledby="computer-heading">
		<h2
			id="computer-heading"
			className="py-2 text-[12px] font-medium text-foreground"
		>
			Computer
		</h2>
		<ComputerStatePanel bot={bot} activeRun={activeRun} variant="minimal" />
		{bot?.computerId ? (
			<RailSection title="Files" defaultOpen>
				<ComputerWorkspaceTree computerId={bot.computerId} />
			</RailSection>
		) : null}
	</section>
	{bot ? (
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
	) : null}
</div>
```

Leave `ComputerStatePanel`'s sr-only "Live view" heading alone. Do not add Routines or Skills rows, chips, or empty-state links in this file.

The lists keep their current copy and actions once the dialog mounts them.

- Routines: pause and resume, weekday-brief empty copy, New routine, Advanced routines when any exist.
- Skills: name plus version, empty copy that points at chat save and settings, Manage skills to `/app/skills`.

Pass `variant="minimal"` in the dialog. Do not restyle the lists for a third look. Do not move attach or pin into the skills dialog. That stays on the Settings Skills tab.

## What you do not do

- Do not add Routines to `SettingsDialog`.
- Do not rewrite `/app/skills` or `/app/routines`.
- Do not keep a 15s poll in a hidden rail instance. Unmount is the poll gate.
- Do not stack this dialog on settings, bot-details, or `SaveAsSkillDialog`.

## Tests to update

`bot-context-rail.test.tsx` must see Computer, Files, and Memory. It must not see Routines or Skills as rail headings.

Add dialog tests for open, replace, close, and collapsed-header access. Assert the list is absent from the document when chrome is `none`.
