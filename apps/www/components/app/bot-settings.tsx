"use client";

import { useEffect, useState } from "react";
import type { BotSummary, ComputerSummary } from "@/lib/api-types";
import { BotModelSelect } from "@/components/app/bot-model-select";
import { ComputerSelect } from "@/components/app/computer-select";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BotAvatarPicker } from "@/components/app/bot-avatar-picker";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { botAvatarFormDefaults, DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { BOT_DELETE_COPY } from "@/lib/bot-delete-copy";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { botConversationHref } from "@/lib/bot-onboarding";
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
  const [avatarId, setAvatarId] = useState(bot.avatarId ?? DEFAULT_BOT_AVATAR_ID);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    setName(bot.name);
    setInstructions(bot.instructions);
    setAvatarId(bot.avatarId ?? DEFAULT_BOT_AVATAR_ID);
    setNotice("");
    setError(null);
  }, [bot]);

  function handleAvatarChange(nextAvatarId: string) {
    if (nextAvatarId === avatarId) return;
    setAvatarId(nextAvatarId);
    if (!bot.avatarId) {
      const defaults = botAvatarFormDefaults(nextAvatarId);
      setName(defaults.name);
      setInstructions(defaults.instructions);
    }
  }

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
          avatarId,
        }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not update bot");
      onSaved(body);
      setNotice("Saved. Work already in progress keeps its earlier instructions.");
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
          onChange={handleAvatarChange}
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
        <BotSetupAgain botId={bot.id} botName={bot.name} />
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

function BotSetupAgain({ botId, botName }: { botId: string; botName: string }) {
  const router = useRouter();
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/bots/${botId}/onboarding`, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) return;
        const body = (await response.json()) as { status?: string };
        setStatus(body.status ?? "not_started");
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, [botId]);

  const label =
    status === "completed" || status === "in_progress" || status === "ready_to_apply"
      ? "Run setup again"
      : "Finish Bot setup";

  return (
    <div className="rounded-lg border border-border/70 bg-muted/20 p-3">
      <p className="text-[13px] font-medium">Tune how {botName} works</p>
      <p className="mt-1 text-[12px] leading-5 text-muted-foreground">
        A short Luna-guided setup that updates instructions and pinned context. It does not
        change approvals, tools, or connectors.
      </p>
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="mt-2"
        onClick={() => router.push(botConversationHref(botId, { setup: true }))}
      >
        {label}
      </Button>
    </div>
  );
}

export function BotAdvancedSettings({
  bot,
  onSaved,
}: {
  bot: BotSummary;
  onSaved: (bot: BotSummary) => void;
}) {
  const [computer, setComputer] = useState(bot.computerId ?? "");
  const [model, setModel] = useState(bot.model || DEFAULT_BOT_MODEL_ID);
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState("");

  useEffect(() => {
    setComputer(bot.computerId ?? "");
    setModel(bot.model || DEFAULT_BOT_MODEL_ID);
    setNotice("");
    setError(null);
  }, [bot]);

  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch("/v1/computers", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error("Could not load workspaces");
        setComputers(await response.json());
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Could not load workspaces");
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
          computerId: computer,
          model,
        }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not update bot");
      onSaved(body);
      setNotice("Saved. New messages use these settings; in-flight work is unchanged.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update bot");
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void save(event)}>
      <FormFields className="min-w-0 gap-5">
        <BotModelSelect
          id="settings-model"
          label="Model"
          value={model}
          onValueChange={setModel}
          disabled={busy}
        />
        <ComputerSelect
          id="settings-computer"
          label="Workspace"
          value={computer}
          onValueChange={setComputer}
          computers={computers}
          allowEmpty
          unavailableId={computer}
        />
        <p className="text-[12px] leading-snug text-muted-foreground">
          Workspace changes apply to new work only. Queued and running work keeps its original
          environment.
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
        <Button type="submit" disabled={busy}>
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
        {BOT_DELETE_COPY}
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
        description={BOT_DELETE_COPY}
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
