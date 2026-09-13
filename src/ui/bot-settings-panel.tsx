import type { Bot, ModelDescriptor } from "@/lib/definitions";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
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
import { ChevronDown, Save, Settings2, Trash2 } from "@/components/icons/lucide";

interface BotSettingsPanelProps {
  bot: Bot;
  models: ModelDescriptor[];
  modelsLoading: boolean;
  modelsError: string | null;
  onChange: (patch: Partial<Pick<Bot, "name" | "systemPrompt" | "model">>) => void;
  onSave: () => void;
  onArchive: () => void;
  onRename?: (name: string) => void;
  saving: boolean;
  variant?: "inline" | "sidebar";
}

export function BotSettingsPanel({
  bot,
  models,
  modelsLoading,
  modelsError,
  onChange,
  onSave,
  onArchive,
  onRename: _onRename,
  saving,
  variant = "inline",
}: BotSettingsPanelProps) {
  const isSidebar = variant === "sidebar";

  return (
    <Collapsible
      defaultOpen={false}
      className={
        isSidebar
          ? "group"
          : "group border-b border-border/80 bg-muted/20"
      }
    >
      <CollapsibleTrigger
        className={
          isSidebar
            ? "flex w-full items-center gap-2 rounded-lg px-1.5 py-1.5 text-left transition-colors hover:bg-white/70"
            : "flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left text-sm font-medium transition-colors hover:bg-muted/40 group-data-open:bg-muted/30 sm:px-4"
        }
      >
        {isSidebar ? (
          <>
            <Settings2
              className="size-3.5 shrink-0 text-muted-foreground/80"
              aria-hidden
            />
            <span className="min-w-0 flex-1 truncate text-[11px] font-semibold tracking-tight text-foreground/90">
              Assistant settings
            </span>
            <ChevronDown
              className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180"
              aria-hidden
            />
          </>
        ) : (
          <>
            <span>Assistant settings</span>
            <ChevronDown
              className="size-4 text-muted-foreground transition-transform group-data-open:rotate-180"
              aria-hidden
            />
          </>
        )}
      </CollapsibleTrigger>
      <CollapsibleContent
        className={
          isSidebar
            ? "px-1 pb-1 pt-0"
            : "px-3 pb-4 pt-1 sm:px-4"
        }
      >
        {isSidebar ? (
          <div className="flex flex-col gap-2 px-0.5 pb-0.5 pt-0.5">
            <div className="space-y-1">
              <Label
                htmlFor="bot-system"
                className="text-[10px] font-medium text-muted-foreground"
              >
                Instructions
              </Label>
              <ScrollArea
                className="h-[7rem] rounded-lg border border-border/50 bg-white/60"
              >
                <Textarea
                  id="bot-system"
                  value={bot.systemPrompt}
                  onChange={(e) => onChange({ systemPrompt: e.target.value })}
                  placeholder="Personality and rules…"
                  rows={4}
                  className="min-h-[7rem] resize-none rounded-none border-0 bg-transparent px-2 py-1.5 !text-[11px] !leading-[1.35] placeholder:!text-[11px] md:!text-[11px] shadow-none field-sizing-content focus-visible:ring-0"
                />
              </ScrollArea>
            </div>
            <div className="flex items-center gap-2">
              <Label
                htmlFor="bot-model"
                className="w-11 shrink-0 text-[10px] font-medium text-muted-foreground"
              >
                Model
              </Label>
              <Select
                value={bot.model}
                onValueChange={(value) => {
                  if (value) {
                    onChange({ model: value });
                  }
                }}
                disabled={modelsLoading || models.length === 0}
              >
                <SelectTrigger
                  id="bot-model"
                  className="h-7 flex-1 border-border/50 bg-white/60 text-[11px] shadow-none"
                >
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
            </div>
            {modelsError && (
              <p className="text-[10px] text-destructive">{modelsError}</p>
            )}
            <div className="flex items-center justify-end gap-1 pt-0.5">
              <Button
                type="button"
                variant="outline"
                size="xs"
                onClick={onSave}
                disabled={saving}
                className="h-6 gap-1 border-border/50 bg-white/50 px-2 text-[10px] shadow-none"
              >
                <Save className="size-3" aria-hidden />
                Save
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={onArchive}
                className="text-muted-foreground hover:text-destructive"
                aria-label="Archive assistant"
              >
                <Trash2 className="size-3.5" aria-hidden />
              </Button>
            </div>
          </div>
        ) : (
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
                <Save className="size-3.5" aria-hidden />
                Save changes
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={onArchive}
                className="gap-1.5 text-muted-foreground"
              >
                <Trash2 className="size-3.5" aria-hidden />
                Archive
              </Button>
            </div>
          </div>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}
