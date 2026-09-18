"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import Link from "next/link";
import { useEffect, useState } from "react";

interface SkillDraftFile {
  relativePath: string;
  content: string;
  contentType?: string | null;
}

interface SaveAsSkillDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  runId: string;
  botId: string;
  botName?: string;
}

interface SavedSkillSummary {
  id: string;
  slug: string;
  name: string;
  currentVersion: number;
  attachedToBot: boolean;
}

export function SaveAsSkillDialog({
  open,
  onOpenChange,
  runId,
  botId,
  botName,
}: SaveAsSkillDialogProps) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [attachToBot, setAttachToBot] = useState(true);
  const [skillMd, setSkillMd] = useState("");
  const [packageFiles, setPackageFiles] = useState<SkillDraftFile[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedSkill, setSavedSkill] = useState<SavedSkillSummary | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setSavedSkill(null);
    setError(null);
    setBusy(true);
    void cloudHostFetch(`/v1/runs/${runId}/skill-draft`, { method: "POST" })
      .then(async (response) => {
        const body = await response.json();
        if (!response.ok) {
          throw new Error(
            typeof body.error === "string" ? body.error : "Could not prepare a skill draft",
          );
        }
        const draftSlug =
          typeof body.parsedName === "string" ? body.parsedName : "";
        setSkillMd(typeof body.skillMd === "string" ? body.skillMd : "");
        setPackageFiles(Array.isArray(body.files) ? body.files : []);
        setName(
          draftSlug
            ? draftSlug
                .split("-")
                .filter(Boolean)
                .map((part: string) => part.charAt(0).toUpperCase() + part.slice(1))
                .join(" ")
            : "",
        );
        setDescription(
          typeof body.parsedDescription === "string" ? body.parsedDescription : "",
        );
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : "Could not prepare a skill draft");
      })
      .finally(() => setBusy(false));
  }, [open, runId]);

  async function handleSave() {
    if (busy || !skillMd.trim()) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const saveResponse = await cloudHostFetch(`/v1/runs/${runId}/skill-save`, {
        method: "POST",
        body: JSON.stringify({
          skillMd,
          files: packageFiles,
          name: name.trim() || undefined,
          description: description.trim() || undefined,
          attachToBot,
          botId,
        }),
      });
      const saved = await saveResponse.json();
      if (!saveResponse.ok) {
        throw new Error(
          typeof saved.error === "string" ? saved.error : "Could not save skill",
        );
      }
      setSavedSkill({
        id: saved.id as string,
        slug: saved.slug as string,
        name: saved.name as string,
        currentVersion: saved.currentVersion as number,
        attachedToBot: saved.attachedToBot === true,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save skill");
    } finally {
      setBusy(false);
    }
  }

  const targetBot = botName?.trim() || "this Bot";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        {savedSkill ? (
          <>
            <DialogHeader>
              <DialogTitle>Skill saved</DialogTitle>
              <DialogDescription>
                {savedSkill.name}
                {savedSkill.attachedToBot
                  ? ` · Attached to ${targetBot} · v${savedSkill.currentVersion}`
                  : ` · v${savedSkill.currentVersion}`}
              </DialogDescription>
            </DialogHeader>
            <DialogFooter className="gap-2 sm:justify-start">
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                Close
              </Button>
              <Link
                href={`/app/skills/${savedSkill.id}`}
                className="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground"
              >
                View skill
              </Link>
            </DialogFooter>
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>Save as skill</DialogTitle>
              <DialogDescription>
                Turn this completed assignment into a reusable skill for {targetBot}.
              </DialogDescription>
            </DialogHeader>
            <div className="space-y-3">
              <FormItem>
                <Label htmlFor="skill-save-name">Name</Label>
                <Input
                  id="skill-save-name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  disabled={busy}
                />
              </FormItem>
              <FormItem>
                <Label htmlFor="skill-save-description">Description</Label>
                <Input
                  id="skill-save-description"
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  disabled={busy}
                />
              </FormItem>
              <label className="flex items-center gap-2 text-sm">
                <Checkbox
                  checked={attachToBot}
                  onCheckedChange={(checked) => setAttachToBot(checked === true)}
                  disabled={busy}
                />
                Attach to {targetBot}
              </label>
              {error ? (
                <p className="text-sm text-destructive" role="alert">
                  {error}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                Cancel
              </Button>
              <Button type="button" disabled={busy} onClick={() => void handleSave()}>
                {busy ? "Saving…" : "Save skill"}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
