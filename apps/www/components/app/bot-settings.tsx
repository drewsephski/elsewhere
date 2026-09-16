"use client";

import { useEffect, useState } from "react";
import type { BotSummary, ComputerSummary } from "@/lib/api-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { useRouter } from "next/navigation";
import { ComputerSelect } from "@/components/app/computer-select";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ChevronDown } from "@/components/icons/lucide";
import { cn } from "cn";
import { BotSkillsSettings } from "@/components/app/bot-skills-settings";

export function BotSettings({
  bot,
  onSaved,
  embedded = false,
}: {
  bot: BotSummary;
  onSaved: (bot: BotSummary) => void;
  embedded?: boolean;
}) {
  const router = useRouter();
  const [name, setName] = useState(bot.name);
  const [instructions, setInstructions] = useState(bot.instructions);
  const [computer, setComputer] = useState(bot.computerId ?? "");
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    setName(bot.name);
    setInstructions(bot.instructions);
    setComputer(bot.computerId ?? "");
  }, [bot]);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch("/v1/computers", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error("Could not load computers");
        setComputers(await response.json());
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Could not load computers");
        }
      });
    return () => controller.abort();
  }, []);

  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    setNotice("");
    try {
      const response = await cloudHostFetch(`/v1/bots/${bot.id}`, {
        method: "PATCH",
        body: JSON.stringify({ name, instructions, computerId: computer }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not update bot");
      onSaved(body);
      setNotice("Saved. Existing work keeps its original instructions and computer.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update bot");
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete() {
    if (busy) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/bots/${bot.id}`, { method: "DELETE" });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(
          typeof body.error === "string" ? body.error : "Could not delete bot",
        );
      }
      setDeleteOpen(false);
      router.push("/app");
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delete bot");
    } finally {
      setBusy(false);
    }
  }

  const labelClass = embedded
    ? "text-[11px] font-medium text-muted-foreground"
    : undefined;

  const form = (
    <form
      className={embedded ? "min-w-0" : "mt-5"}
      onSubmit={(event) => void save(event)}
    >
      <FormFields className={embedded ? "min-w-0 gap-2" : undefined}>
          <FormItem>
            <Label htmlFor="settings-name" className={labelClass}>Name</Label>
            <Input
              id="settings-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              maxLength={100}
              required
              className={embedded ? "h-7 text-xs" : undefined}
            />
          </FormItem>
          <FormItem>
            <Label htmlFor="settings-role" className={labelClass}>Role and instructions</Label>
            <Textarea
              id="settings-role"
              className={embedded ? "min-h-20 text-xs leading-5" : "min-h-28 text-xs leading-5"}
              value={instructions}
              onChange={(event) => setInstructions(event.target.value)}
              maxLength={16000}
            />
          </FormItem>
          <ComputerSelect
            id="settings-computer"
            label="Assigned computer"
            value={computer}
            onValueChange={setComputer}
            computers={computers}
            allowEmpty
            unavailableId={computer}
            compact={embedded}
          />
          <BotSkillsSettings botId={bot.id} embedded={embedded} />
          <p className={embedded ? "text-[11px] leading-snug text-muted-foreground" : "text-xs text-muted-foreground"}>
            {embedded
              ? "Computer changes apply to new work only."
              : "Changing computers does not move files. Queued work stays on its original computer."}
          </p>
          <div className="flex min-w-0 items-center gap-1.5">
            <Button type="submit" size={embedded ? "sm" : "default"} disabled={busy || !name.trim()}>
              {busy ? "Saving…" : embedded ? "Save" : "Save settings"}
            </Button>
            <Button
              type="button"
              variant={embedded ? "ghost" : "outline"}
              size={embedded ? "sm" : "default"}
              className="text-destructive hover:bg-destructive/10"
              disabled={busy}
              onClick={() => setDeleteOpen(true)}
            >
              Delete bot
            </Button>
          </div>
          {deleteOpen ? (
            <div
              className="rounded-lg border border-destructive/25 bg-destructive/10 p-2.5 text-sm"
              role="alertdialog"
              aria-labelledby="delete-bot-title"
            >
              <p id="delete-bot-title" className="font-medium text-destructive-foreground">
                Delete {bot.name}?
              </p>
              <p className="mt-1 text-[11px] leading-snug text-destructive-foreground/90">
                This removes the bot and its settings. Work history may remain in your account.
              </p>
              <div className="mt-2.5 flex gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={busy}
                  onClick={() => setDeleteOpen(false)}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="destructive"
                  disabled={busy}
                  onClick={() => void handleDelete()}
                >
                  {busy ? "Deleting…" : "Delete"}
                </Button>
              </div>
            </div>
          ) : null}
          {notice ? (
            <p role="status" className={embedded ? "text-[11px] text-muted-foreground" : "text-sm text-muted-foreground"}>
              {embedded ? "Saved." : notice}
            </p>
          ) : null}
          {error ? (
            <p role="alert" className={embedded ? "text-[11px] text-destructive" : "text-sm text-destructive"}>
              {error}
            </p>
          ) : null}
      </FormFields>
    </form>
  );

  if (embedded) {
    return form;
  }

  return (
    <Collapsible defaultOpen={false} className="group surface-card">
      <CollapsibleTrigger className="flex w-full cursor-pointer items-center justify-between gap-2 text-base font-semibold">
        Bot settings
        <ChevronDown
          className={cn(
            "size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-open:rotate-180",
          )}
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent>{form}</CollapsibleContent>
    </Collapsible>
  );
}
