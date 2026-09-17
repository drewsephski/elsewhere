// @vitest-environment happy-dom

import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useConversationIdentityLayout } from "./use-conversation-identity-layout";

describe("useConversationIdentityLayout", () => {
  it("does not reset on mount or when identity is unchanged across re-renders", () => {
    const onIdentityChange = vi.fn();
    const { rerender } = renderHook(
      ({ id }: { id: string }) => useConversationIdentityLayout(id, onIdentityChange),
      { initialProps: { id: "bot-a" } },
    );

    expect(onIdentityChange).not.toHaveBeenCalled();

    rerender({ id: "bot-a" });
    rerender({ id: "bot-a" });

    expect(onIdentityChange).not.toHaveBeenCalled();
  });

  it("runs reset when identity changes", () => {
    const onIdentityChange = vi.fn();
    const { rerender } = renderHook(
      ({ id }: { id: string }) => useConversationIdentityLayout(id, onIdentityChange),
      { initialProps: { id: "bot-a" } },
    );

    rerender({ id: "bot-b" });

    expect(onIdentityChange).toHaveBeenCalledTimes(1);

    rerender({ id: "bot-b" });
    expect(onIdentityChange).toHaveBeenCalledTimes(1);

    rerender({ id: "bot-c" });
    expect(onIdentityChange).toHaveBeenCalledTimes(2);
  });
});
