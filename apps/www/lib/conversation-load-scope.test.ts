import { describe, expect, it } from "vitest";
import { bumpLoadScope, createLoadScopeRef, isActiveLoadScope } from "./conversation-load-scope";

describe("conversation load scope", () => {
  it("invalidates in-flight loads after identity bump", () => {
    const scope = createLoadScopeRef();
    const first = bumpLoadScope(scope);
    expect(isActiveLoadScope(scope, first)).toBe(true);

    const second = bumpLoadScope(scope);
    expect(isActiveLoadScope(scope, first)).toBe(false);
    expect(isActiveLoadScope(scope, second)).toBe(true);
  });
});
