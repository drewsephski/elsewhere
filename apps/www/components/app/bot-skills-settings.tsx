"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { cloudHostFetch } from "@/lib/cloud-api";
import { appRoutes } from "@/lib/app-routes";
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

export function BotSkillsSettings({
  botId,
  embedded = false,
}: {
  botId: string;
  embedded?: boolean;
}) {
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

  const labelClass = embedded
    ? "text-[11px] font-medium text-muted-foreground"
    : undefined;

  return (
    <div className="min-w-0 space-y-3">
      {embedded ? (
        <p className="text-[11px] leading-snug text-muted-foreground">
          Skills run with this bot. Pin a version to freeze behavior.
        </p>
      ) : (
        <p className="text-[13px] text-muted-foreground">
          <Link
            href={appRoutes.skills}
            className="underline underline-offset-4 hover:text-foreground"
          >
            Manage catalog
          </Link>
        </p>
      )}
      {attached.length ? (
        <ul className={embedded ? "space-y-0.5" : "space-y-2 text-sm"}>
          {attached.map((row) => (
            <li
              key={row.skillId}
              className={
                embedded
                  ? "flex min-w-0 items-center gap-2 rounded-md px-1 py-1"
                  : "flex flex-wrap items-center justify-between gap-2 rounded-lg bg-muted/40 px-3 py-2"
              }
            >
              <span className="min-w-0 flex-1 truncate text-[13px] leading-tight">
                {row.name}
                <span className="text-muted-foreground">
                  {" "}
                  ({row.slug}
                  {row.pinnedVersion
                    ? ` · v${row.pinnedVersion}`
                    : ` · v${row.currentVersion}`}
                  )
                </span>
              </span>
              <Button
                type="button"
                variant={embedded ? "ghost" : "outline"}
                size={embedded ? "xs" : "sm"}
                className={embedded ? "h-6 shrink-0 px-1.5 text-[11px] text-muted-foreground" : undefined}
                disabled={busy}
                onClick={() => void handleDetach(row.skillId)}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-[11px] text-muted-foreground">No skills attached yet.</p>
      )}
      <div
        className={
          embedded
            ? "grid min-w-0 gap-2"
            : "grid gap-3 sm:grid-cols-[1fr_auto_auto] sm:items-end"
        }
      >
        <FormItem>
          <Label htmlFor="attach-skill" className={labelClass}>
            Add skill
          </Label>
          <Select
            value={attachSkillId}
            onValueChange={(value) => {
              if (value) setAttachSkillId(value);
            }}
            disabled={busy || !available.length}
          >
            <SelectTrigger
              id="attach-skill"
              size={embedded ? "sm" : "default"}
              className="w-full min-w-0"
              aria-label="Skill to attach"
            >
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
        <div className={embedded ? "grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-end gap-2" : "contents"}>
          <FormItem>
            <Label htmlFor="pin-version" className={labelClass}>
              {embedded ? "Pin version" : "Pin version (optional)"}
            </Label>
            <input
              id="pin-version"
              className={
                embedded
                  ? "h-7 w-full min-w-0 rounded-md border border-input bg-background px-2 text-xs"
                  : "h-9 w-full min-w-[7rem] rounded-md border border-input bg-background px-3 text-sm"
              }
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
            size={embedded ? "sm" : "default"}
            className={embedded ? "shrink-0" : undefined}
            disabled={busy || !attachSkillId}
            onClick={() => void handleAttach()}
          >
            Attach
          </Button>
        </div>
      </div>
      {error ? (
        <p className="text-xs text-destructive" role="alert">{error}</p>
      ) : null}
    </div>
  );
}
