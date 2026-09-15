"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { cloudHostFetch } from "@/lib/cloud-host";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

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

  useEffect(() => {
    void (async () => {
      const detail = await cloudHostFetch(`/v1/skills/${skillId}`);
      if (!detail.ok) return;
      const body = (await detail.json()) as SkillDetail;
      setSkill(body);
      const versions = await cloudHostFetch(`/v1/skills/${skillId}/versions`);
      if (!versions.ok) return;
      const list = (await versions.json()) as SkillVersion[];
      setSkillMd(list[0]?.skillMd ?? "");
    })();
  }, [skillId]);

  async function handleSaveVersion() {
    const response = await cloudHostFetch(`/v1/skills/${skillId}/versions`, {
      method: "POST",
      body: JSON.stringify({ skillMd, files: [] }),
    });
    setMessage(response.ok ? "Saved new version." : "Validation failed.");
    if (response.ok) {
      const detail = await cloudHostFetch(`/v1/skills/${skillId}`);
      if (detail.ok) setSkill((await detail.json()) as SkillDetail);
    }
  }

  if (!skill) {
    return <p className="text-sm text-muted-foreground">Loading skill…</p>;
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div className="space-y-1">
        <Link href="/app/skills" className="text-sm text-muted-foreground hover:underline">
          ← Skills
        </Link>
        <h1 className="text-2xl font-semibold">{skill.name}</h1>
        <p className="text-sm text-muted-foreground">
          {skill.slug} · v{skill.currentVersion} · {skill.status}
        </p>
      </div>
      <Textarea
        value={skillMd}
        onChange={(event) => setSkillMd(event.target.value)}
        className="min-h-[320px] font-mono text-xs"
        aria-label="SKILL.md editor"
      />
      <Button type="button" onClick={handleSaveVersion}>
        Save new version
      </Button>
      {message ? <p className="text-sm text-muted-foreground">{message}</p> : null}
    </div>
  );
}
