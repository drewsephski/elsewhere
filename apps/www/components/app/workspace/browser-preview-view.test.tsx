// @vitest-environment happy-dom

import type { BrowserPreviewFrame } from "@/hooks/use-browser-preview";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserPreviewView } from "./browser-preview-view";

const frame: BrowserPreviewFrame = {
  available: true,
  url: "https://example.com",
  title: "Example",
  contentType: "text/html",
  imageDataUrl: "data:image/png;base64,abc",
  capturedAt: "2026-01-01T00:00:00.000Z",
  version: 1,
};

const dockPip = vi.fn();
const openPip = vi.fn();

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

const previewState = {
  pipOpen: false,
  dockPip,
  openPip,
  computerId: "comp_1",
  enabled: true,
  frame,
  loading: false,
  error: null,
  refresh: vi.fn(),
  navigateBrowser: vi.fn(),
};

vi.mock("@/contexts/browser-preview-context", () => ({
  useOptionalBrowserPreviewContext: () => previewState,
  useBrowserPreviewContext: () => previewState,
}));

describe("BrowserPreviewView dock/float", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    dockPip.mockClear();
    openPip.mockClear();
  });
  it("renders the docked preview once while pip is closed", () => {
    previewState.pipOpen = false;
    render(<BrowserPreviewView variant="embedded" />);
    expect(screen.getByRole("img", { name: "Browser: Example" })).toBeTruthy();
    expect(screen.queryByText(/Preview is floating over the chat/)).toBeNull();
    expect(screen.getByRole("button", { name: "Float preview over chat" })).toBeTruthy();
  });

  it("shows the dock placeholder instead of a second live preview while floating", () => {
    previewState.pipOpen = true;
    render(<BrowserPreviewView variant="embedded" />);
    expect(screen.queryByRole("img", { name: "Browser: Example" })).toBeNull();
    expect(screen.getByText(/Preview is floating over the chat/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Dock it here" }));
    expect(dockPip).toHaveBeenCalledTimes(1);
  });

  it("floats from the docked header without duplicating the screen", () => {
    previewState.pipOpen = false;
    render(<BrowserPreviewView variant="embedded" />);
    fireEvent.click(screen.getByRole("button", { name: "Float preview over chat" }));
    expect(openPip).toHaveBeenCalledTimes(1);
  });

  it("places expand on the live screen, separate from the float control", () => {
    previewState.pipOpen = false;
    render(<BrowserPreviewView variant="embedded" />);
    const expand = screen.getByRole("button", { name: "Expand preview" });
    const float = screen.getByRole("button", { name: "Float preview over chat" });
    expect(expand).toBeTruthy();
    expect(float).toBeTruthy();
    expect(expand).not.toBe(float);
    expect(expand.textContent).toContain("Expand");
  });

  it("fills the work pane instead of a centered thumbnail", () => {
    previewState.pipOpen = false;
    const { container } = render(<BrowserPreviewView variant="work" />);
    expect(screen.queryByText("Browser")).toBeNull();
    expect(screen.getByRole("button", { name: "Open expanded browser preview" })).toBeTruthy();
    expect(container.firstElementChild?.className).not.toContain("max-w-xs");
    expect(container.firstElementChild?.className).not.toContain("max-w-sm");
  });
});
