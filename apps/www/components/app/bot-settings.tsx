"use client";

import { useEffect, useState } from "react";
import type { BotSummary, ComputerSummary } from "@/lib/api-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BotAvatarPicker } from "@/components/app/bot-avatar-picker";
import { ComputerSelect } from "@/components/app/computer-select";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useRouter } from "next/navigation";

export function BotGeneralSettings({
  bot,
  onSaved,
}: {
  bot: BotSummary;
  onSaved: (bot: BotSummary) => void;
}) {
  const [name, setName] = useState(bot.name);
  const [instructions, setInstructions] = useState(bot.instructions);
  const [computer, setComputer] = useState(bot.computerId ?? "");
  const [avatarId, setAvatarId] = useState(bot.avatarId ?? DEFAULT_BOT_AVATAR_ID);
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    setName(bot.name);
    setInstructions(bot.instructions);
    setComputer(bot.computerId ?? "");
    setAvatarId(bot.avatarId ?? DEFAULT_BOT_AVATAR_ID);
    setNotice("");
    setError(null);
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
        body: JSON.stringify({
          name,
          instructions,
          computerId: computer,
          avatarId,
        }),
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

  return (
    <form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void save(event)}>
      <FormFields className="min-w-0 gap-5">
        <BotAvatarPicker
          value={avatarId}
          onChange={setAvatarId}
          disabled={busy}
          compact
        />
        <FormItem>
          <Label htmlFor="settings-name">Name</Label>
          <Input
            id="settings-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            maxLength={100}
            required
          />
        </FormItem>
        <FormItem>
          <Label htmlFor="settings-role">Role and instructions</Label>
          <Textarea
            id="settings-role"
            className="min-h-32 text-[13px] leading-5"
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
        />
        <p className="text-[12px] leading-snug text-muted-foreground">
          Computer changes apply to new work only. Queued and running work keeps its original snapshot.
        </p>
      </FormFields>
      <div className="sticky bottom-0 mt-6 flex items-center justify-end gap-3 border-t border-border bg-card pt-3">
        {notice ? (
          <p role="status" className="mr-auto text-[12px] text-muted-foreground">
            {notice}
          </p>
        ) : null}
        {error ? (
          <p role="alert" className="mr-auto text-[12px] text-destructive">
            {error}
          </p>
        ) : null}
        <Button type="submit" disabled={busy || !name.trim()}>
          {busy ? "Saving…" : "Save changes"}
        </Button>
      </div>
    </form>
  );
}

export function BotDeleteSettings({
  bot,
  onDeleted,
}: {
  bot: BotSummary;
  onDeleted?: () => void;
}) {
  const router = useRouter();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      setConfirmOpen(false);
      onDeleted?.();
      router.push("/app");
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delete bot");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="max-w-md space-y-3">
      <p className="text-[13px] leading-relaxed text-muted-foreground">
        Remove this Bot from the workspace. Work history may remain in your account.
      </p>
      <Button
        type="button"
        variant="ghost"
        className="text-destructive hover:bg-destructive/10"
        disabled={busy}
        onClick={() => setConfirmOpen(true)}
      >
        Delete Bot
      </Button>
      <ConfirmAlertDialog
        open={confirmOpen}
        onOpenChange={(open) => {
          if (!busy) {
            setConfirmOpen(open);
          }
        }}
        title={`Delete ${bot.name}?`}
        description="This removes the bot and its settings. Work history may remain in your account."
        confirmLabel="Delete"
        pendingLabel="Deleting…"
        destructive
        pending={busy}
        error={error}
        onConfirm={() => void handleDelete()}
      />
    </div>
  );
}

/** @deprecated Legacy collapsible used by unused BotChat. Prefer SettingsDialog. */
export function BotSettings({
  bot,
  onSaved,
}: {
  bot: BotSummary;
  onSaved: (bot: BotSummary) => void;
  embedded?: boolean;
}) {
  return <BotGeneralSettings bot={bot} onSaved={onSaved} />;
}
