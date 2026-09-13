interface SettingsModalProps {
  open: boolean;
  apiKeyConfigured: boolean;
  apiKeyDraft: string;
  error: string | null;
  saving: boolean;
  onClose: () => void;
  onApiKeyChange: (value: string) => void;
  onSave: () => void;
  onClear: () => void;
}

export function SettingsModal({
  open,
  apiKeyConfigured,
  apiKeyDraft,
  error,
  saving,
  onClose,
  onApiKeyChange,
  onSave,
  onClear,
}: SettingsModalProps) {
  if (!open) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
    >
      <div className="w-full max-w-md rounded-lg border border-border bg-surface-1 shadow-xl">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 id="settings-title" className="text-sm font-semibold">
            Settings
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-muted hover:text-foreground text-sm"
            aria-label="Close settings"
          >
            Close
          </button>
        </div>
        <div className="p-4 space-y-3">
          <p className="text-xs text-muted">
            OpenAI API keys are stored in the macOS Keychain. They are never
            logged or written to SQLite.
          </p>
          <label className="flex flex-col gap-1 text-xs text-muted">
            OpenAI API key
            <input
              type="password"
              value={apiKeyDraft}
              onChange={(e) => onApiKeyChange(e.target.value)}
              placeholder={
                apiKeyConfigured ? "•••••••• (configured)" : "sk-…"
              }
              className="rounded border border-border bg-surface-0 px-3 py-2 text-sm"
              autoComplete="off"
            />
          </label>
          {error && <p className="text-danger text-sm">{error}</p>}
          <div className="flex gap-2 justify-end pt-2">
            {apiKeyConfigured && (
              <button
                type="button"
                onClick={onClear}
                disabled={saving}
                className="rounded-md border border-border px-3 py-1.5 text-sm text-muted hover:bg-surface-2"
              >
                Remove key
              </button>
            )}
            <button
              type="button"
              onClick={onSave}
              disabled={saving || !apiKeyDraft.trim()}
              className="rounded-md bg-accent px-3 py-1.5 text-sm text-white hover:bg-accent-hover disabled:opacity-50"
            >
              Save key
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
