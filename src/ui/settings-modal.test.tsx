// @vitest-environment happy-dom

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { SettingsModal } from "./settings-modal";

describe("SettingsModal", () => {
  test("exposes Connect to Elsewhere for unpaired Mac Companion pairing", () => {
    const onConnectElsewhere = vi.fn();
    render(
      <SettingsModal
        open
        apiKeyConfigured={false}
        apiKeyDraft=""
        elsewhereConnected={false}
        elsewhereReauthRequired={false}
        elsewherePairing={false}
        error={null}
        saving={false}
        onClose={vi.fn()}
        onApiKeyChange={vi.fn()}
        onSave={vi.fn()}
        onClear={vi.fn()}
        onConnectElsewhere={onConnectElsewhere}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Connect to Elsewhere" }));
    expect(onConnectElsewhere).toHaveBeenCalledTimes(1);
  });
});
