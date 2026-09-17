// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { AnimatedTabs, AnimatedTabsTrigger } from "./animated-tabs";
import { useState } from "react";

function TabsExample({
  variant,
  selection,
}: {
  variant: "underline" | "pill" | "vertical";
  selection?: "tab" | "radio";
}) {
  const [value, setValue] = useState("one");
  return (
    <AnimatedTabs
      value={value}
      onValueChange={setValue}
      variant={variant}
      selection={selection}
      aria-label="Example"
    >
      <AnimatedTabsTrigger value="one">One</AnimatedTabsTrigger>
      <AnimatedTabsTrigger value="two">Two</AnimatedTabsTrigger>
    </AnimatedTabs>
  );
}

describe("AnimatedTabs", () => {
  afterEach(() => {
    cleanup();
  });
  it("switches underline tabs and exposes tab semantics", () => {
    render(<TabsExample variant="underline" />);
    const one = screen.getByRole("tab", { name: "One" });
    const two = screen.getByRole("tab", { name: "Two" });
    expect(one.getAttribute("aria-selected")).toBe("true");
    fireEvent.click(two);
    expect(two.getAttribute("aria-selected")).toBe("true");
    expect(one.getAttribute("aria-selected")).toBe("false");
  });

  it("keeps permission selectors as radios", () => {
    render(<TabsExample variant="pill" selection="radio" />);
    expect(screen.getByRole("radiogroup", { name: "Example" })).toBeTruthy();
    const two = screen.getByRole("radio", { name: "Two" });
    fireEvent.click(two);
    expect(two.getAttribute("aria-checked")).toBe("true");
  });

  it("scopes layout ids per instance so sibling controls do not share one global indicator", () => {
    function Pair() {
      const [left, setLeft] = useState("allow");
      const [right, setRight] = useState("ask");
      return (
        <>
          <AnimatedTabs value={left} onValueChange={setLeft} variant="pill" aria-label="Left">
            <AnimatedTabsTrigger value="allow">Allow</AnimatedTabsTrigger>
            <AnimatedTabsTrigger value="ask">Ask</AnimatedTabsTrigger>
            <AnimatedTabsTrigger value="deny">Deny</AnimatedTabsTrigger>
          </AnimatedTabs>
          <AnimatedTabs value={right} onValueChange={setRight} variant="pill" aria-label="Right">
            <AnimatedTabsTrigger value="allow">Allow</AnimatedTabsTrigger>
            <AnimatedTabsTrigger value="ask">Ask</AnimatedTabsTrigger>
            <AnimatedTabsTrigger value="deny">Deny</AnimatedTabsTrigger>
          </AnimatedTabs>
        </>
      );
    }
    render(<Pair />);
    const scopes = Array.from(document.querySelectorAll("[data-animated-tabs]"));
    expect(scopes).toHaveLength(2);
    expect(scopes[0]?.getAttribute("data-animated-tabs")).not.toBe(
      scopes[1]?.getAttribute("data-animated-tabs"),
    );
    fireEvent.click(screen.getAllByRole("radio", { name: "Deny" })[0]!);
    expect(screen.getAllByRole("radio", { name: "Deny" })[0]?.getAttribute("aria-checked")).toBe(
      "true",
    );
    expect(screen.getAllByRole("radio", { name: "Ask" })[1]?.getAttribute("aria-checked")).toBe(
      "true",
    );
  });

  it("moves selection with arrow keys", () => {
    render(<TabsExample variant="underline" />);
    const list = screen.getByRole("tablist", { name: "Example" });
    fireEvent.keyDown(list, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Two" }).getAttribute("aria-selected")).toBe("true");
  });
});
