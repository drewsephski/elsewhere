// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FloatingBrowserPreview } from "./floating-browser-preview";

const dockPip = vi.fn();
const dismissPipForSession = vi.fn();
const refresh = vi.fn();

const previewState = {
  pipOpen: true,
  pipPosition: null,
  dockPip,
  dismissPipForSession,
  setPipPosition: vi.fn(),
  computerId: "comp_1",
  enabled: true,
  frame: {
    available: true,
    url: "https://example.com",
    title: "Example",
    contentType: "text/html",
    imageDataUrl: "data:image/png;base64,abc",
    capturedAt: "2026-01-01T00:00:00.000Z",
    version: 1,
  },
  loading: false,
  error: null,
  refresh,
  navigateBrowser: vi.fn(),
  closeBrowser: vi.fn(),
};

vi.mock("@/contexts/browser-preview-context", () => ({
  useBrowserPreviewContext: () => previewState,
  useOptionalBrowserPreviewContext: () => previewState,
}));

vi.mock("@/contexts/active-run-context", () => ({
  useOptionalActiveRun: () => null,
}));

vi.mock("@/hooks/use-browser-human-control", () => ({
  useBrowserHumanControl: () => ({
    humanActive: false,
    loading: false,
    error: null,
    takeControl: vi.fn(),
    returnControl: vi.fn(),
  }),
}));

describe("FloatingBrowserPreview", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    dockPip.mockClear();
    dismissPipForSession.mockClear();
    previewState.pipOpen = true;
  });
  it("renders over the chat pane with dock and hide actions", () => {
    previewState.pipOpen = true;
    render(
      <div className="relative" style={{ width: 800, height: 600 }}>
        <FloatingBrowserPreview />
      </div>,
    );
    expect(screen.getByRole("region", { name: "Floating live computer preview" })).toBeTruthy();
    expect(screen.getByLabelText("Navigate browser to URL")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Dock preview in sidebar" }));
    expect(dockPip).toHaveBeenCalledTimes(1);
  });

  it("does not render when the preview is docked", () => {
    previewState.pipOpen = false;
    render(<FloatingBrowserPreview />);
    expect(screen.queryByRole("region", { name: "Floating live computer preview" })).toBeNull();
  });
});
