"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import type { ComputerSummary } from "@/lib/api-types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

interface CreateBotDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateBotDialog({ open, onClose }: CreateBotDialogProps) {
  const router = useRouter();
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [name, setName] = useState("");
  const [instructions, setInstructions] = useState(
    "Complete delegated work carefully, keep useful files on your computer, and explain your results clearly. Ask for approval before making changes.",
  );
  const [computerId, setComputerId] = useState("");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
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

  useEffect(() => {
    if (!open) {
      return;
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  if (!open) {
    return null;
  }

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
          enginePreference: "codex",
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
    <div className="fixed inset-0 z-[60] flex items-end justify-center p-4 sm:items-center" role="dialog" aria-modal="true" aria-labelledby="create-bot-title">
      <button
        type="button"
        className="absolute inset-0 bg-black/30 backdrop-blur-[2px]"
        aria-label="Close"
        onClick={onClose}
      />
      <form
        onSubmit={(event) => void handleSubmit(event)}
        className="relative z-10 w-full max-w-md rounded-2xl border border-border bg-card p-5 shadow-2xl"
      >
        <h2 id="create-bot-title" className="text-lg font-semibold">New bot</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Give it a name, a role, and a computer to work on.
        </p>
        <div className="mt-4 space-y-3">
          <div>
            <Label htmlFor="create-bot-name">Name</Label>
            <Input
              id="create-bot-name"
              required
              maxLength={100}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Chief of Staff"
            />
          </div>
          <div>
            <Label htmlFor="create-bot-instructions">Role and instructions</Label>
            <textarea
              id="create-bot-instructions"
              className="mt-1 min-h-28 w-full rounded-lg border border-border bg-background p-3 text-sm leading-6"
              maxLength={16000}
              value={instructions}
              onChange={(event) => setInstructions(event.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="create-bot-computer">Computer</Label>
            <select
              id="create-bot-computer"
              className="mt-1 w-full rounded-lg border border-border bg-background p-3 text-sm"
              value={computerId}
              onChange={(event) => setComputerId(event.target.value)}
              disabled={loading}
            >
              <option value="">
                {loading ? "Loading computers…" : "Choose a computer"}
              </option>
              {computers.map((computer) => (
                <option key={computer.id} value={computer.id}>
                  {computer.displayName}
                </option>
              ))}
            </select>
            {!loading && !computers.length ? (
              <p className="mt-2 text-sm text-muted-foreground">
                <Link href="/app/computers" className="underline">Create a computer</Link> first.
              </p>
            ) : null}
          </div>
        </div>
        {error ? (
          <p className="mt-3 text-sm text-red-700" role="alert">{error}</p>
        ) : null}
        <div className="mt-5 flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy || !computerId || !name.trim()}>
            {busy ? "Creating…" : "Create bot"}
          </Button>
        </div>
      </form>
    </div>
  );
}
