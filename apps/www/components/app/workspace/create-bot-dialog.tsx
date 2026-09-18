"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ComputerSummary } from "@/lib/api-types";
import { BotAvatarPicker } from "@/components/app/bot-avatar-picker";
import { BotModelSelect } from "@/components/app/bot-model-select";
import { ComputerSelect } from "@/components/app/computer-select";
import { botAvatarFormDefaults, DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

interface CreateBotDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateBotDialog({ open, onClose }: CreateBotDialogProps) {
  const router = useRouter();
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const defaultForm = botAvatarFormDefaults(DEFAULT_BOT_AVATAR_ID);
  const [name, setName] = useState(defaultForm.name);
  const [instructions, setInstructions] = useState(defaultForm.instructions);
  const [computerId, setComputerId] = useState("");
  const [model, setModel] = useState(DEFAULT_BOT_MODEL_ID);
  const [avatarId, setAvatarId] = useState(DEFAULT_BOT_AVATAR_ID);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  function handleAvatarChange(nextAvatarId: string) {
    setAvatarId(nextAvatarId);
    const defaults = botAvatarFormDefaults(nextAvatarId);
    setName(defaults.name);
    setInstructions(defaults.instructions);
  }

  useEffect(() => {
    if (!open) {
      return;
    }
    const defaults = botAvatarFormDefaults(DEFAULT_BOT_AVATAR_ID);
    setAvatarId(DEFAULT_BOT_AVATAR_ID);
    setName(defaults.name);
    setInstructions(defaults.instructions);
    setModel(DEFAULT_BOT_MODEL_ID);
    setError(null);
    setLoading(true);
    const controller = new AbortController();
    cloudHostFetch("/v1/computers", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error("Could not load computers");
        }
        const items: ComputerSummary[] = await response.json();
        setComputers(items);
        setComputerId(items[0]?.id ?? "");
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Could not load computers");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setLoading(false);
        }
      });
    return () => controller.abort();
  }, [open]);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (busy) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/bots", {
        method: "POST",
        body: JSON.stringify({
          name,
          instructions,
          computerId,
          model,
          enginePreference: "codex",
          avatarId,
        }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not create bot");
      }
      onClose();
      router.push(`/app/bots/${body.id}`);
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create bot");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent className="gap-3 overflow-visible p-4 sm:max-w-lg">
        <form onSubmit={(event) => void handleSubmit(event)}>
          <DialogHeader className="gap-1">
            <DialogTitle className="text-base">New bot</DialogTitle>
            <DialogDescription className="text-xs">
              Give it a name, a role, a model, and a computer to work on.
            </DialogDescription>
          </DialogHeader>
          <FormFields className="mt-2 gap-3">
            <BotAvatarPicker
              value={avatarId}
              onChange={handleAvatarChange}
              disabled={busy}
              compact
              className="min-w-0"
            />
            <FormItem>
              <Label htmlFor="create-bot-name">Name</Label>
              <Input
                id="create-bot-name"
                required
                maxLength={100}
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="Chief of Staff"
              />
            </FormItem>
            <FormItem>
              <Label htmlFor="create-bot-instructions">Role and instructions</Label>
              <Textarea
                id="create-bot-instructions"
                rows={5}
                className="field-sizing-fixed h-[7.5rem] resize-none overflow-y-auto"
                maxLength={16000}
                value={instructions}
                onChange={(event) => setInstructions(event.target.value)}
              />
            </FormItem>
            <BotModelSelect
              id="create-bot-model"
              value={model}
              onValueChange={setModel}
              disabled={busy}
            />
            <ComputerSelect
              id="create-bot-computer"
              value={computerId}
              onValueChange={setComputerId}
              computers={computers}
              loading={loading}
              disabled={loading}
            />
            {!loading && !computers.length ? (
              <div className="space-y-2 rounded-lg border border-border/60 bg-muted/30 p-3">
                <p className="text-sm text-muted-foreground">
                  Bots need a computer to work on. Create one first, then come back.
                </p>
                <Button
                  type="button"
                  className="w-full sm:w-auto"
                  onClick={() => {
                    onClose();
                    router.push("/app/computers");
                  }}
                >
                  Create a computer
                </Button>
              </div>
            ) : null}
          </FormFields>
          {error ? (
            <p className="mt-3 text-sm text-destructive" role="alert">{error}</p>
          ) : null}
          <DialogFooter className="mt-3">
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !computerId || !name.trim()}>
              {busy ? "Creating…" : "Create bot"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
