"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { cloudHostErrorMessage, cloudHostFetch } from "@/lib/cloud-api";
import { ConfirmAlertDialog } from "@/components/app/confirm-alert-dialog";
import { InlineRenameLabel } from "@/components/app/inline-rename-label";
import { WorkspaceEmptyState } from "@/components/app/workspace-empty-state";
import { WorkspacePageHeader } from "@/components/app/workspace-page-header";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/reui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { ChevronRight, Delete, Plus, Sparkles, SquarePen } from "@/components/icons/lucide";

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
  const [skillToDelete, setSkillToDelete] = useState<SkillSummary | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [renamingSkillId, setRenamingSkillId] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);

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

  function handleRequestRename(skill: SkillSummary) {
    if (creating || deleting || renaming) {
      return;
    }
    setError(null);
    setRenamingSkillId(skill.id);
  }

  async function handleRename(skill: SkillSummary, next: string) {
    if (renaming) {
      return;
    }
    setRenaming(true);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/skills/${skill.id}`, {
        method: "PATCH",
        body: JSON.stringify({ name: next }),
      });
      if (!response.ok) {
        throw new Error(
          await cloudHostErrorMessage(response, "Could not rename skill"),
        );
      }
      const body = (await response.json()) as SkillSummary;
      setSkills((current) =>
        current.map((row) => (row.id === skill.id ? { ...row, ...body } : row)),
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not rename skill");
    } finally {
      setRenaming(false);
      setRenamingSkillId(null);
    }
  }

  function handleRequestDelete(skill: SkillSummary) {
    if (creating || deleting || renaming) {
      return;
    }
    setDeleteError(null);
    setSkillToDelete(skill);
  }

  async function handleDeleteConfirm() {
    if (!skillToDelete || deleting) {
      return;
    }
    setDeleting(true);
    setDeleteError(null);
    try {
      const response = await cloudHostFetch(`/v1/skills/${skillToDelete.id}`, {
        method: "DELETE",
      });
      if (!response.ok) {
        setDeleteError(
          await cloudHostErrorMessage(response, "Could not delete skill"),
        );
        return;
      }
      setSkillToDelete(null);
      await load();
    } catch (err) {
      setDeleteError(
        err instanceof Error ? err.message : "Could not delete skill",
      );
    } finally {
      setDeleting(false);
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
            <li
              key={skill.id}
              className="group/skill border-b border-border last:border-b-0 hover:bg-muted/40"
            >
              <div className="flex items-stretch">
                <Link
                  href={`/app/skills/${skill.id}`}
                  onClick={(event) => {
                    if (renamingSkillId === skill.id) {
                      event.preventDefault();
                    }
                  }}
                  className="group flex min-w-0 flex-1 items-start gap-3 px-4 py-3.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
                >
                  <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg bg-surface-active text-foreground">
                    <Sparkles className="size-4" aria-hidden />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                      <h2 className="min-w-0 max-w-full">
                        <InlineRenameLabel
                          value={skill.name}
                          nested
                          startEditing={renamingSkillId === skill.id}
                          disabled={creating || deleting || renaming}
                          onEditingChange={(editing) => {
                            if (!editing && renamingSkillId === skill.id) {
                              setRenamingSkillId(null);
                            }
                          }}
                          onCommit={(next) => handleRename(skill, next)}
                          className="truncate text-sm font-medium tracking-tight"
                          inputClassName="h-7 text-sm font-medium"
                          ariaLabel={`Rename ${skill.name}`}
                        />
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
                <div
                  className={
                    renamingSkillId === skill.id
                      ? "pointer-events-none flex shrink-0 items-start py-3 pr-3 opacity-0"
                      : "flex shrink-0 items-start py-3 pr-2"
                  }
                >
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="text-muted-foreground opacity-70 transition-opacity group-hover/skill:opacity-100 hover:opacity-100"
                    disabled={creating || deleting || renaming}
                    aria-label={`Rename ${skill.name}`}
                    onClick={() => handleRequestRename(skill)}
                  >
                    <SquarePen aria-hidden />
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="text-muted-foreground opacity-70 transition-opacity group-hover/skill:opacity-100 hover:bg-destructive/10 hover:text-destructive hover:opacity-100"
                    disabled={creating || deleting || renaming}
                    aria-label={`Delete ${skill.name}`}
                    onClick={() => handleRequestDelete(skill)}
                  >
                    <Delete aria-hidden />
                  </Button>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}

      <ConfirmAlertDialog
        open={Boolean(skillToDelete)}
        onOpenChange={(open) => {
          if (!open && !deleting) {
            setSkillToDelete(null);
            setDeleteError(null);
          }
        }}
        title={skillToDelete ? `Delete ${skillToDelete.name}?` : "Delete skill?"}
        description="This permanently removes the skill and its versions. Bots that have it attached will lose it. This cannot be undone."
        confirmLabel="Delete"
        pendingLabel="Deleting…"
        destructive
        pending={deleting}
        error={deleteError}
        onConfirm={handleDeleteConfirm}
      />
    </div>
  );
}
