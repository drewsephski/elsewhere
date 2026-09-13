import type { Bot, ModelDescriptor } from "@/lib/definitions";

interface BotSettingsPanelProps {
  bot: Bot;
  models: ModelDescriptor[];
  modelsLoading: boolean;
  modelsError: string | null;
  onChange: (patch: Partial<Pick<Bot, "name" | "systemPrompt" | "model">>) => void;
  onSave: () => void;
  onArchive: () => void;
  saving: boolean;
}

export function BotSettingsPanel({
  bot,
  models,
  modelsLoading,
  modelsError,
  onChange,
  onSave,
  onArchive,
  saving,
}: BotSettingsPanelProps) {
  return (
    <div className="border-b border-border bg-surface-1 px-4 py-3 flex flex-wrap gap-3 items-end">
      <label className="flex flex-col gap-1 text-xs text-muted min-w-[140px]">
        Name
        <input
          value={bot.name}
          onChange={(e) => onChange({ name: e.target.value })}
          className="rounded border border-border bg-surface-0 px-2 py-1.5 text-sm text-foreground"
        />
      </label>
      <label className="flex flex-col gap-1 text-xs text-muted min-w-[200px] flex-1">
        System instructions
        <input
          value={bot.systemPrompt}
          onChange={(e) => onChange({ systemPrompt: e.target.value })}
          className="rounded border border-border bg-surface-0 px-2 py-1.5 text-sm text-foreground"
          placeholder="Optional system prompt"
        />
      </label>
      <label className="flex flex-col gap-1 text-xs text-muted min-w-[180px]">
        Model
        <select
          value={bot.model}
          onChange={(e) => onChange({ model: e.target.value })}
          disabled={modelsLoading || models.length === 0}
          className="rounded border border-border bg-surface-0 px-2 py-1.5 text-sm text-foreground disabled:opacity-50"
        >
          {models.length === 0 ? (
            <option value={bot.model}>{bot.model}</option>
          ) : (
            models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.displayName}
              </option>
            ))
          )}
        </select>
        {modelsError && (
          <span className="text-danger text-[11px]">{modelsError}</span>
        )}
      </label>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onSave}
          disabled={saving}
          className="rounded-md bg-accent px-3 py-1.5 text-sm text-white hover:bg-accent-hover disabled:opacity-50"
        >
          Save
        </button>
        <button
          type="button"
          onClick={onArchive}
          className="rounded-md border border-border px-3 py-1.5 text-sm text-muted hover:text-foreground hover:bg-surface-2"
        >
          Archive
        </button>
      </div>
    </div>
  );
}
