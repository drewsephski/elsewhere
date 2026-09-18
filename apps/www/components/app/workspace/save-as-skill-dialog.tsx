"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { parseSkillFrontmatterName } from "@/lib/skill-frontmatter";
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

interface SaveAsSkillDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  runId: string;
  botId: string;
  botName?: string;
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
  const [slug, setSlug] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedSkillId, setSavedSkillId] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    setSavedSkillId(null);
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
          typeof body.parsedName === "string"
            ? body.parsedName
            : parseSkillFrontmatterName(body.skillMd ?? "") ?? "";
        setSlug(draftSlug);
        setName(
          typeof body.parsedName === "string"
            ? body.parsedName.replace(/-/g, " ")
            : draftSlug.replace(/-/g, " "),
        );
        setDescription(typeof body.parsedDescription === "string" ? body.parsedDescription : "");
        setSkillMd(typeof body.skillMd === "string" ? body.skillMd : "");
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
      const draftResponse = await cloudHostFetch(`/v1/runs/${runId}/skill-draft`, {
        method: "POST",
      });
      const draftBody = await draftResponse.json();
      if (!draftResponse.ok) {
        throw new Error(
          typeof draftBody.error === "string" ? draftBody.error : "Could not refresh draft",
        );
      }
      let nextMd = draftBody.skillMd as string;
      const nextSlug = parseSkillFrontmatterName(nextMd) ?? slug;
      if (name.trim() || description.trim()) {
        const lines = nextMd.split("\n");
        const rebuilt = lines.map((line: string) => {
          if (line.startsWith("name:") && name.trim()) {
            return `name: ${nextSlug}`;
          }
          if (line.startsWith("description:") && description.trim()) {
            return `description: ${description.trim()}`;
          }
          return line;
        });
        nextMd = rebuilt.join("\n");
      }
      const createResponse = await cloudHostFetch("/v1/skills", {
        method: "POST",
        body: JSON.stringify({
          slug: nextSlug,
          skillMd: nextMd,
          files: draftBody.files ?? [],
        }),
      });
      const created = await createResponse.json();
      if (!createResponse.ok) {
        throw new Error(
          typeof created.error === "string" ? created.error : "Could not save skill",
        );
      }
      if (attachToBot) {
        const attachResponse = await cloudHostFetch(`/v1/bots/${botId}/skills`, {
          method: "POST",
          body: JSON.stringify({ skillId: created.id }),
        });
        if (!attachResponse.ok) {
          throw new Error("Skill saved but could not attach to this Bot");
        }
      }
      setSavedSkillId(created.id as string);
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
        {savedSkillId ? (
          <>
            <DialogHeader>
              <DialogTitle>Skill saved</DialogTitle>
              <DialogDescription>
                {name.trim() || slug}
                {attachToBot ? ` · Attached to ${targetBot} · v1` : " · v1"}
              </DialogDescription>
            </DialogHeader>
            <DialogFooter className="gap-2 sm:justify-start">
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                Close
              </Button>
              <Link
                href={`/app/skills/${savedSkillId}`}
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
