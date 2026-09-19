// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MultipleChoiceQuestionCard } from "./multiple-choice-question-card";

afterEach(() => {
  cleanup();
});

describe("MultipleChoiceQuestionCard", () => {
  it("renders options, progress, and skip", () => {
    const onSelect = vi.fn();
    const onSkip = vi.fn();
    render(
      <MultipleChoiceQuestionCard
        title="Scout setup"
        prompt="What should Scout own?"
        options={[
          { id: "research", label: "Research briefs", description: "Sourced one-pagers" },
          { id: "ops", label: "Launch ops" },
          { id: "other", label: "Something else…" },
        ]}
        allowCustom
        progress="1 of up to 3"
        onSelect={onSelect}
        onSkip={onSkip}
      />,
    );
    expect(screen.getByText("1 of up to 3")).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: /Research briefs/ }));
    expect(onSelect).toHaveBeenCalledWith("research");
    fireEvent.click(screen.getByRole("button", { name: "Skip for now" }));
    expect(onSkip).toHaveBeenCalled();
  });

  it("reveals a custom answer field for Other", () => {
    const onSubmitCustom = vi.fn();
    render(
      <MultipleChoiceQuestionCard
        title="Scout setup"
        prompt="What should Scout own?"
        options={[
          { id: "research", label: "Research briefs" },
          { id: "other", label: "Something else…" },
        ]}
        allowCustom
        selectedId="other"
        customValue="Own the weekly launch checklist"
        onCustomChange={vi.fn()}
        onSelect={vi.fn()}
        onSubmitCustom={onSubmitCustom}
      />,
    );
    expect(screen.getByLabelText("Custom answer")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(onSubmitCustom).toHaveBeenCalled();
  });

  it("adds a Something else option when custom answers are allowed", () => {
    render(
      <MultipleChoiceQuestionCard
        title="Scout setup"
        prompt="What should Scout own?"
        options={[
          { id: "research", label: "Research briefs" },
          { id: "ops", label: "Launch ops" },
        ]}
        allowCustom
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByRole("radio", { name: "Something else…" })).toBeTruthy();
  });

  it("shows a thinking loader for the chosen option instead of dimming the list", () => {
    render(
      <MultipleChoiceQuestionCard
        title="Scout setup"
        prompt="What should Scout own?"
        options={[
          { id: "research", label: "Research briefs", description: "Sourced one-pagers" },
          { id: "ops", label: "Launch ops" },
        ]}
        selectedId="research"
        pending
        loadingLabel="Preparing the next question…"
        onSelect={vi.fn()}
        onSkip={vi.fn()}
      />,
    );
    expect(screen.getByRole("status").textContent).toContain("Preparing the next question…");
    expect(screen.getByText("Research briefs")).toBeTruthy();
    expect(screen.queryByRole("radio")).toBeNull();
    expect(screen.queryByRole("button", { name: "Skip for now" })).toBeNull();
  });
});
