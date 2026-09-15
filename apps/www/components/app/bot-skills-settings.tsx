"use client";

import { useCallback, useEffect, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { FormItem } from "@/components/ui/form-item";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface SkillSummary {
  id: string;
  slug: string;
  name: string;
  status: string;
  currentVersion: number;
}

interface BotSkillAttachment {
  skillId: string;
  slug: string;
  name: string;
  enabled: boolean;
  pinnedVersion: number | null;
  currentVersion: number;
}

export function BotSkillsSettings({ botId }: { botId: string }) {
  const [catalog, setCatalog] = useState<SkillSummary[]>([]);
  const [attached, setAttached] = useState<BotSkillAttachment[]>([]);
  const [attachSkillId, setAttachSkillId] = useState("");
  const [pinVersion, setPinVersion] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const [skillsRes, attachedRes] = await Promise.all([
      cloudHostFetch("/v1/skills"),
      cloudHostFetch(`/v1/bots/${botId}/skills`),
    ]);
    if (skillsRes.ok) {
      setCatalog((await skillsRes.json()) as SkillSummary[]);
    }
    if (attachedRes.ok) {
      setAttached((await attachedRes.json()) as BotSkillAttachment[]);
    }
  }, [botId]);

  useEffect(() => {
    void load();
  }, [load]);

  const available = catalog.filter(
    (skill) =>
      skill.status === "active" &&
      !attached.some((row) => row.skillId === skill.id),
  );

  async function handleAttach() {
    if (!attachSkillId || busy) return;
    setBusy(true);
    setError(null);
    try {
      const pinned = pinVersion.trim()
        ? Number.parseInt(pinVersion, 10)
        : undefined;
      const response = await cloudHostFetch(`/v1/bots/${botId}/skills`, {
        method: "POST",
        body: JSON.stringify({
          skillId: attachSkillId,
          pinnedVersion: pinned,
        }),
      });
      if (!response.ok) {
        throw new Error("Could not attach skill");
      }
      setAttachSkillId("");
      setPinVersion("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not attach skill");
    } finally {
      setBusy(false);
    }
  }

  async function handleDetach(skillId: string) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/bots/${botId}/skills/${skillId}`,
        { method: "DELETE" },
      );
      if (!response.ok) {
        throw new Error("Could not remove skill");
      }
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not remove skill");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-3 rounded-xl border border-border/60 p-4">
      <div>
        <p className="text-sm font-medium">Enabled skills</p>
        <p className="text-xs text-muted-foreground">
          Attached skills are materialized for each run on this bot. Pin a version to freeze
          behavior.
        </p>
      </div>
      {attached.length ? (
        <ul className="space-y-2 text-sm">
          {attached.map((row) => (
            <li
              key={row.skillId}
              className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-muted/40 px-3 py-2"
            >
              <span>
                {row.name}{" "}
                <span className="text-muted-foreground">
                  ({row.slug}
                  {row.pinnedVersion
                    ? ` · pinned v${row.pinnedVersion}`
                    : ` · latest v${row.currentVersion}`}
                  )
                </span>
              </span>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={() => void handleDetach(row.skillId)}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-xs text-muted-foreground">No skills attached yet.</p>
      )}
      <div className="grid gap-3 sm:grid-cols-[1fr_auto_auto] sm:items-end">
        <FormItem>
          <Label htmlFor="attach-skill">Add skill</Label>
          <Select
            value={attachSkillId}
            onValueChange={(value) => {
              if (value) setAttachSkillId(value);
            }}
            disabled={busy || !available.length}
          >
            <SelectTrigger id="attach-skill" aria-label="Skill to attach">
              <SelectValue placeholder={available.length ? "Choose a skill" : "No skills available"} />
            </SelectTrigger>
            <SelectContent>
              {available.map((skill) => (
                <SelectItem key={skill.id} value={skill.id}>
                  {skill.name} (v{skill.currentVersion})
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </FormItem>
        <FormItem>
          <Label htmlFor="pin-version">Pin version (optional)</Label>
          <input
            id="pin-version"
            className="h-9 w-full min-w-[7rem] rounded-md border border-input bg-background px-3 text-sm"
            inputMode="numeric"
            placeholder="Latest"
            value={pinVersion}
            onChange={(event) => setPinVersion(event.target.value)}
            disabled={busy}
            aria-label="Pinned skill version"
          />
        </FormItem>
        <Button
          type="button"
          disabled={busy || !attachSkillId}
          onClick={() => void handleAttach()}
        >
          Attach
        </Button>
      </div>
      {error ? (
        <p className="text-xs text-destructive" role="alert">{error}</p>
      ) : null}
    </div>
  );
}
