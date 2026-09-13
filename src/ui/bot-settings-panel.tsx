import type { Bot, ModelDescriptor } from "@/lib/definitions";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ChevronDown, Save, Trash2 } from "lucide-react";

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
    <Collapsible
      defaultOpen={false}
      className="border-b border-border/80 bg-muted/20"
    >
      <CollapsibleTrigger
        className="group flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left text-sm font-medium transition-colors hover:bg-muted/40 data-panel-open:bg-muted/30 sm:px-4"
      >
        <span>Bot configuration</span>
        <ChevronDown
          className="size-4 text-muted-foreground transition-transform group-data-panel-open:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="px-3 pb-4 pt-1 sm:px-4">
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-[1fr_1.5fr_minmax(12rem,1fr)_auto] lg:items-end">
          <div className="space-y-1.5">
            <Label htmlFor="bot-name">Name</Label>
            <Input
              id="bot-name"
              value={bot.name}
              onChange={(e) => onChange({ name: e.target.value })}
            />
          </div>
          <div className="space-y-1.5 sm:col-span-2 lg:col-span-1">
            <Label htmlFor="bot-system">System instructions</Label>
            <Input
              id="bot-system"
              value={bot.systemPrompt}
              onChange={(e) => onChange({ systemPrompt: e.target.value })}
              placeholder="Optional personality or rules"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="bot-model">Model</Label>
            <Select
              value={bot.model}
              onValueChange={(value) => {
                if (value) {
                  onChange({ model: value });
                }
              }}
              disabled={modelsLoading || models.length === 0}
            >
              <SelectTrigger id="bot-model" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {models.length === 0 ? (
                  <SelectItem value={bot.model}>{bot.model}</SelectItem>
                ) : (
                  models.map((m) => (
                    <SelectItem key={m.id} value={m.id}>
                      {m.displayName}
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>
            {modelsError && (
              <p className="text-xs text-destructive">{modelsError}</p>
            )}
          </div>
          <div className="flex flex-wrap gap-2 sm:col-span-2 lg:col-span-1 lg:justify-end">
            <Button type="button" onClick={onSave} disabled={saving} className="gap-1.5">
              <Save className="size-4" />
              Save changes
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={onArchive}
              className="gap-1.5 text-muted-foreground"
            >
              <Trash2 className="size-4" />
              Archive
            </Button>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
