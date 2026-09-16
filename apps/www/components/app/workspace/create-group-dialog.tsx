"use client";

import type { WorkspaceBotPresence } from "@/lib/workspace-types";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useState } from "react";

interface CreateGroupDialogProps {
  open: boolean;
  bots: WorkspaceBotPresence[];
  onClose: () => void;
  onCreated: (groupId: string) => void;
}

export function CreateGroupDialog({
  open,
  bots,
  onClose,
  onCreated,
}: CreateGroupDialogProps) {
  const [name, setName] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function handleToggle(botId: string) {
    setSelected((current) =>
      current.includes(botId)
        ? current.filter((id) => id !== botId)
        : current.length >= 6
          ? current
          : [...current, botId],
    );
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (selected.length < 2) {
      setError("Choose at least 2 bots.");
      return;
    }
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch("/v1/conversations/groups", {
        method: "POST",
        body: JSON.stringify({ name: name.trim() || "Group", botIds: selected }),
      });
      const body = await response.json();
      if (!response.ok) {
        throw new Error(body.error ?? "Could not create group");
      }
      onCreated(body.id as string);
      setName("");
      setSelected([]);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create group");
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create group conversation</DialogTitle>
          <DialogDescription>
            Pick 2–6 bots to share one transcript. Mention routing comes in a later update.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={(event) => void handleSubmit(event)} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="group-name">Group title</Label>
            <Input
              id="group-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Product launch"
              maxLength={200}
            />
          </div>
          <div className="max-h-48 space-y-1 overflow-y-auto rounded-md border border-border p-2">
            {bots.map((bot) => {
              const checked = selected.includes(bot.id);
              return (
                <label
                  key={bot.id}
                  className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 hover:bg-muted"
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => handleToggle(bot.id)}
                    aria-label={`Include ${bot.name}`}
                  />
                  <span className="text-sm">{bot.name}</span>
                </label>
              );
            })}
          </div>
          <p className="text-xs text-muted-foreground">{selected.length} of 2–6 selected</p>
          {error ? (
            <p className="text-sm text-destructive" role="alert">{error}</p>
          ) : null}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={pending || selected.length < 2}>
              {pending ? "Creating…" : "Create group"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
