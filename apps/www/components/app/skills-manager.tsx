"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-api";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/reui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { ChevronRight, Plus, Sparkles } from "@/components/icons/lucide";

interface SkillSummary {
  id: string;
  slug: string;
  name: string;
  description: string;
  status: string;
  currentVersion: number;
  updatedAt: string;
}

function SkillStatusBadge({ status }: { status: string }) {
  const active = status === "active";
  return (
    <Badge variant={active ? "success-light" : "warning-light"} size="sm">
      {status}
    </Badge>
  );
}

export function SkillsManager() {
  const router = useRouter();
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    const response = await cloudHostFetch("/v1/skills");
    if (!response.ok) {
      setError("Could not load skills");
      setLoading(false);
      return;
    }
    setSkills((await response.json()) as SkillSummary[]);
    setError(null);
    setLoading(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreate() {
    if (creating) return;
    setCreating(true);
    setError(null);
    try {
      const slug = `skill-${Date.now()}`;
      const skillMd = `---\nname: ${slug}\ndescription: New skill\n---\n\nDescribe the workflow here.\n`;
      const response = await cloudHostFetch("/v1/skills", {
        method: "POST",
        body: JSON.stringify({ slug, skillMd, files: [] }),
      });
      const body = (await response.json().catch(() => ({}))) as {
        id?: string;
        error?: string;
      };
      if (!response.ok) {
        setError(
          typeof body.error === "string" ? body.error : "Could not create skill",
        );
        return;
      }
      if (typeof body.id === "string") {
        router.push(`/app/skills/${body.id}`);
        return;
      }
      await load();
    } finally {
      setCreating(false);
    }
  }

  function renderCreateButton() {
    return (
      <Button
        type="button"
        onClick={() => void handleCreate()}
        disabled={creating}
        aria-label={creating ? "Creating skill" : "Create a new skill"}
      >
        <Plus data-icon="inline-start" aria-hidden />
        {creating ? "Creating…" : "New skill"}
      </Button>
    );
  }

  return (
    <div className="w-full max-w-3xl space-y-8">
      <WorkspacePageHeader
        title="Skills"
        description="Reusable Agent Skill packages for your bots — versioned, attachable, and snapshotted on every run. Product Skills use the open SKILL.md format."
        action={renderCreateButton()}
      />

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Could not update skills</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {loading ? (
        <ul className="divide-y divide-border overflow-hidden rounded-xl border border-border bg-card">
          {Array.from({ length: 3 }).map((_, index) => (
            <li key={index} className="flex items-center gap-4 px-4 py-4">
              <Skeleton className="size-9 rounded-lg" />
              <div className="min-w-0 flex-1 space-y-2">
                <Skeleton className="h-4 w-40" />
                <Skeleton className="h-3 w-3/4" />
              </div>
            </li>
          ))}
        </ul>
      ) : skills.length === 0 ? (
        <WorkspaceEmptyState
          title="No skills yet"
          description="Package a workflow once, then attach it to any bot. Each run snapshots the version so behavior stays reproducible."
          icon={<Sparkles aria-hidden />}
        >
          {renderCreateButton()}
        </WorkspaceEmptyState>
      ) : (
        <ul className="overflow-hidden rounded-xl border border-border bg-card">
          {skills.map((skill) => (
            <li key={skill.id} className="border-b border-border last:border-b-0">
              <Link
                href={`/app/skills/${skill.id}`}
                className="group flex items-start gap-3 px-4 py-3.5 transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
              >
                <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg bg-surface-active text-foreground">
                  <Sparkles className="size-4" aria-hidden />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <h2 className="truncate text-sm font-medium tracking-tight">
                      {skill.name}
                    </h2>
                    <SkillStatusBadge status={skill.status} />
                  </div>
                  <p className="mt-1 line-clamp-2 text-sm leading-6 text-muted-foreground">
                    {skill.description || "No description yet."}
                  </p>
                  <p className="mt-1.5 text-xs text-muted-foreground">
                    {skill.slug} · v{skill.currentVersion}
                  </p>
                </div>
                <ChevronRight
                  className="mt-2 size-4 shrink-0 text-muted-foreground/70 transition-colors group-hover:text-foreground"
                  aria-hidden
                />
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
