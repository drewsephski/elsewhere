"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { cloudHostFetch } from "@/lib/cloud-api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface SkillSummary {
  id: string;
  slug: string;
  name: string;
  description: string;
  status: string;
  currentVersion: number;
  updatedAt: string;
}

export function SkillsManager() {
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    const response = await cloudHostFetch("/v1/skills");
    if (!response.ok) {
      setError("Could not load skills");
      return;
    }
    setSkills((await response.json()) as SkillSummary[]);
    setError(null);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreate() {
    setCreating(true);
    try {
      const slug = `skill-${Date.now()}`;
      const skillMd = `---\nname: ${slug}\ndescription: New skill\n---\n\nDescribe the workflow here.\n`;
      const response = await cloudHostFetch("/v1/skills", {
        method: "POST",
        body: JSON.stringify({ slug, skillMd, files: [] }),
      });
      if (!response.ok) {
        setError("Could not create skill");
        return;
      }
      await load();
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          Product Skills use the open SKILL.md format. Repository `.cursor/skills` remain developer tooling only.
        </p>
        <Button type="button" onClick={handleCreate} disabled={creating}>
          {creating ? "Creating…" : "New skill"}
        </Button>
      </div>
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <div className="grid gap-3 md:grid-cols-2">
        {skills.map((skill) => (
          <Card key={skill.id}>
            <CardHeader className="pb-2">
              <CardTitle className="text-base">
                <Link href={`/app/skills/${skill.id}`} className="hover:underline">
                  {skill.name}
                </Link>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm text-muted-foreground">
              <p>{skill.description}</p>
              <p>
                v{skill.currentVersion} · {skill.status}
              </p>
            </CardContent>
          </Card>
        ))}
      </div>
      {!skills.length && !error ? (
        <p className="text-sm text-muted-foreground">No skills yet.</p>
      ) : null}
    </div>
  );
}
