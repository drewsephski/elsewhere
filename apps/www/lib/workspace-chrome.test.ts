import { describe, expect, it } from "vitest";
import {
  closedLibrary,
  libraryOverlayFromChrome,
  reduceChrome,
  type TransientChromeOverlay,
} from "./workspace-chrome";

const none: TransientChromeOverlay = { kind: "none" };

describe("reduceChrome", () => {
  it("opens routines then skills as one library overlay", () => {
    const afterRoutines = reduceChrome(none, {
      type: "toggle-library",
      library: "routines",
      botId: "bot_1",
    });
    expect(afterRoutines).toEqual({
      kind: "library",
      library: "routines",
      botId: "bot_1",
    });

    expect(
      reduceChrome(afterRoutines, {
        type: "toggle-library",
        library: "skills",
        botId: "bot_1",
      }),
    ).toEqual({
      kind: "library",
      library: "skills",
      botId: "bot_1",
    });
  });

  it("toggles the same library closed", () => {
    const opened = reduceChrome(none, {
      type: "toggle-library",
      library: "routines",
      botId: "bot_1",
    });
    expect(
      reduceChrome(opened, {
        type: "toggle-library",
        library: "routines",
        botId: "bot_1",
      }),
    ).toEqual({ kind: "none" });
  });

  it("replaces a library overlay with settings", () => {
    const library = reduceChrome(none, {
      type: "toggle-library",
      library: "routines",
      botId: "bot_1",
    });
    expect(
      reduceChrome(library, { type: "open-settings", section: "general" }),
    ).toEqual({ kind: "settings", section: "general" });
  });

  it("closes the library when the selected bot changes", () => {
    const library = reduceChrome(none, {
      type: "toggle-library",
      library: "skills",
      botId: "bot_1",
    });
    expect(
      reduceChrome(library, { type: "bot-changed", botId: "bot_2" }),
    ).toEqual({ kind: "none" });
  });

  it("ignores set-settings-section while chrome is none", () => {
    expect(
      reduceChrome(none, { type: "set-settings-section", section: "skills" }),
    ).toEqual({ kind: "none" });
  });

  it("keeps conversationId when replacing routines with skills if the event includes it", () => {
    const opened = reduceChrome(none, {
      type: "toggle-library",
      library: "routines",
      botId: "bot_1",
      conversationId: "conv_1",
    });
    expect(
      reduceChrome(opened, {
        type: "toggle-library",
        library: "skills",
        botId: "bot_1",
        conversationId: "conv_1",
      }),
    ).toEqual({
      kind: "library",
      library: "skills",
      botId: "bot_1",
      conversationId: "conv_1",
    });
  });
});

describe("libraryOverlayFromChrome", () => {
  it("projects a closed overlay from none", () => {
    expect(libraryOverlayFromChrome(none)).toEqual(closedLibrary());
    expect(closedLibrary()).toEqual({ status: "closed" });
  });

  it("projects an open overlay from a library chrome variant", () => {
    expect(
      libraryOverlayFromChrome({
        kind: "library",
        library: "routines",
        botId: "bot_1",
        conversationId: "conv_1",
      }),
    ).toEqual({
      status: "open",
      library: "routines",
      botId: "bot_1",
      conversationId: "conv_1",
    });
  });
});
