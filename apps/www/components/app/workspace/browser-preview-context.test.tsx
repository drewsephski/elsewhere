// @vitest-environment happy-dom

import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import {
  BrowserPreviewProvider,
  useBrowserPreviewContext,
} from "@/contexts/browser-preview-context";

vi.mock("@/hooks/use-browser-preview", () => ({
  useBrowserPreview: () => ({
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
    refresh: vi.fn(),
    navigateBrowser: vi.fn(),
  }),
}));

function PreviewAndRailControls() {
  const preview = useBrowserPreviewContext();
  const [railCollapsed, setRailCollapsed] = useState(false);
  return (
    <div>
      <p>{preview.pipOpen ? "floating" : "docked"}</p>
      <p>{railCollapsed ? "rail-collapsed" : "rail-expanded"}</p>
      <button type="button" onClick={() => preview.openPip()}>
        Float
      </button>
      <button type="button" onClick={() => preview.dockPip()}>
        Dock
      </button>
      <button type="button" onClick={() => setRailCollapsed(true)}>
        Collapse sidebar
      </button>
    </div>
  );
}

describe("browser preview and sidebar state", () => {
  it("toggles dock and float without sharing sidebar collapse state", () => {
    render(
      <BrowserPreviewProvider computerId="comp_1" enabled>
        <PreviewAndRailControls />
      </BrowserPreviewProvider>,
    );

    expect(screen.getByText("docked")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Float" }));
    expect(screen.getByText("floating")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(screen.getByText("floating")).toBeTruthy();
    expect(screen.getByText("rail-collapsed")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Dock" }));
    expect(screen.getByText("docked")).toBeTruthy();
    expect(screen.getByText("rail-collapsed")).toBeTruthy();
  });
});
