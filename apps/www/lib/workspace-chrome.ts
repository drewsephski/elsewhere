import type { SettingsSection } from "@/lib/settings-sections";

export type LibraryKind = "routines" | "skills";

export type LibraryOverlay =
  | { status: "closed" }
  | { status: "open"; library: LibraryKind; botId: string; conversationId?: string };

export type TransientChromeOverlay =
  | { kind: "none" }
  | { kind: "settings"; section: SettingsSection }
  | { kind: "library"; library: LibraryKind; botId: string; conversationId?: string }
  | { kind: "bot-details" };

export type ChromeEvent =
  | { type: "close" }
  | {
      type: "toggle-library";
      library: LibraryKind;
      botId: string;
      conversationId?: string;
    }
  | { type: "open-settings"; section: SettingsSection }
  | { type: "set-settings-section"; section: SettingsSection }
  | { type: "open-bot-details" }
  | { type: "bot-changed"; botId: string | null };

export function closedLibrary(): LibraryOverlay {
  return { status: "closed" };
}

export function libraryOverlayFromChrome(
  chrome: TransientChromeOverlay,
): LibraryOverlay {
  if (chrome.kind !== "library") {
    return closedLibrary();
  }
  if (chrome.conversationId !== undefined) {
    return {
      status: "open",
      library: chrome.library,
      botId: chrome.botId,
      conversationId: chrome.conversationId,
    };
  }
  return {
    status: "open",
    library: chrome.library,
    botId: chrome.botId,
  };
}

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
      const conversationId =
        event.conversationId ??
        (state.kind === "library" && state.botId === event.botId
          ? state.conversationId
          : undefined);
      if (conversationId !== undefined) {
        return {
          kind: "library",
          library: event.library,
          botId: event.botId,
          conversationId,
        };
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
