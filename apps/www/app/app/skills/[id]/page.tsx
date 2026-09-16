"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/reui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { ChevronLeft } from "@/components/icons/lucide";

interface SkillDetail {
  id: string;
  slug: string;
  name: string;
  description: string;
  status: string;
  currentVersion: number;
}

interface SkillVersion {
  version: number;
  skillMd: string;
}

export default function SkillDetailPage() {
  const params = useParams<{ id: string }>();
  const skillId = params.id;
  const [skill, setSkill] = useState<SkillDetail | null>(null);
  const [skillMd, setSkillMd] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    void (async () => {
      const detail = await cloudHostFetch(`/v1/skills/${skillId}`);
      if (!detail.ok) {
        setLoadFailed(true);
        return;
      }
      const body = (await detail.json()) as SkillDetail;
      setSkill(body);
      const versions = await cloudHostFetch(`/v1/skills/${skillId}/versions`);
      if (!versions.ok) return;
      const list = (await versions.json()) as SkillVersion[];
      setSkillMd(list[0]?.skillMd ?? "");
    })();
  }, [skillId]);

  async function handleSaveVersion() {
    if (saving) return;
    setSaving(true);
    setMessage(null);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/skills/${skillId}/versions`, {
        method: "POST",
        body: JSON.stringify({ skillMd }),
      });
      if (!response.ok) {
        setError("Validation failed. Check the SKILL.md frontmatter and try again.");
        return;
      }
      setMessage("Saved a new version.");
      const detail = await cloudHostFetch(`/v1/skills/${skillId}`);
      if (detail.ok) setSkill((await detail.json()) as SkillDetail);
    } finally {
      setSaving(false);
    }
  }

  if (loadFailed) {
    return (
      <div className="space-y-4">
        <Link
          href="/app/skills"
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        >
          <ChevronLeft className="size-4" aria-hidden />
          Skills
        </Link>
        <Alert variant="destructive">
          <AlertTitle>Skill not found</AlertTitle>
          <AlertDescription>
            This skill may have been removed. Return to the catalog and pick another.
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  if (!skill) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-4 w-24" />
        <div className="space-y-2">
          <Skeleton className="h-7 w-56" />
          <Skeleton className="h-4 w-40" />
        </div>
        <Skeleton className="h-[320px] w-full rounded-xl" />
      </div>
    );
  }

  const active = skill.status === "active";

  return (
    <div className="space-y-6">
      <header className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0 space-y-2">
          <Link
            href="/app/skills"
            className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
          >
            <ChevronLeft className="size-4" aria-hidden />
            Skills
          </Link>
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-xl font-semibold tracking-tight">{skill.name}</h1>
            <Badge variant={active ? "success-light" : "warning-light"} size="sm">
              {skill.status}
            </Badge>
          </div>
          <p className="text-sm text-muted-foreground">
            {skill.slug} · v{skill.currentVersion}
          </p>
        </div>
        <Button
          type="button"
          onClick={() => void handleSaveVersion()}
          disabled={saving}
          className="shrink-0"
        >
          {saving ? "Saving…" : "Save new version"}
        </Button>
      </header>

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Could not save</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}
      {message ? (
        <Alert variant="success">
          <AlertTitle>{message}</AlertTitle>
        </Alert>
      ) : null}

      <Textarea
        value={skillMd}
        onChange={(event) => setSkillMd(event.target.value)}
        className="min-h-[min(60vh,32rem)] font-mono text-xs leading-5"
        aria-label="SKILL.md editor"
      />
    </div>
  );
}
