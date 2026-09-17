// @vitest-environment happy-dom

import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useComposerAttachments } from "./composer-attachments";

describe("useComposerAttachments reset stability", () => {
  it("keeps the same reset function across re-renders", () => {
    const { result, rerender } = renderHook(() =>
      useComposerAttachments({ botId: "bot-a" }),
    );
    const firstReset = result.current.reset;
    rerender();
    rerender();
    expect(result.current.reset).toBe(firstReset);
  });
});
