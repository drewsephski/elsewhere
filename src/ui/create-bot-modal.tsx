interface CreateBotModalProps {
  open: boolean;
  name: string;
  systemPrompt: string;
  model: string;
  models: { id: string; displayName: string }[];
  modelsLoading: boolean;
  error: string | null;
  onClose: () => void;
  onNameChange: (v: string) => void;
  onSystemPromptChange: (v: string) => void;
  onModelChange: (v: string) => void;
  onCreate: () => void;
}

export function CreateBotModal({
  open,
  name,
  systemPrompt,
  model,
  models,
  modelsLoading,
  error,
  onClose,
  onNameChange,
  onSystemPromptChange,
  onModelChange,
  onCreate,
}: CreateBotModalProps) {
  if (!open) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="create-bot-title"
    >
      <div className="w-full max-w-md rounded-lg border border-border bg-surface-1">
        <div className="border-b border-border px-4 py-3 flex justify-between items-center">
          <h2 id="create-bot-title" className="text-sm font-semibold">
            New Bot
          </h2>
          <button type="button" onClick={onClose} className="text-sm text-muted">
            Cancel
          </button>
        </div>
        <div className="p-4 space-y-3">
          <label className="flex flex-col gap-1 text-xs text-muted">
            Name
            <input
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              className="rounded border border-border bg-surface-0 px-3 py-2 text-sm"
              autoFocus
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-muted">
            System instructions
            <textarea
              value={systemPrompt}
              onChange={(e) => onSystemPromptChange(e.target.value)}
              rows={3}
              className="rounded border border-border bg-surface-0 px-3 py-2 text-sm resize-none"
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-muted">
            Model
            <select
              value={model}
              onChange={(e) => onModelChange(e.target.value)}
              disabled={modelsLoading || models.length === 0}
              className="rounded border border-border bg-surface-0 px-3 py-2 text-sm"
            >
              {models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.displayName}
                </option>
              ))}
            </select>
          </label>
          {error && <p className="text-danger text-sm">{error}</p>}
          <button
            type="button"
            onClick={onCreate}
            disabled={!name.trim() || !model}
            className="w-full rounded-md bg-accent py-2 text-sm text-white hover:bg-accent-hover disabled:opacity-50"
          >
            Create Bot
          </button>
        </div>
      </div>
    </div>
  );
}
