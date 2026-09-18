"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

interface BotSkillAttachment {
  skillId: string;
  slug: string;
  name: string;
  pinnedVersion: number | null;
  currentVersion: number;
}

export function BotSkillsSidebar({
  botId,
  variant = "default",
}: {
  botId: string;
  variant?: "default" | "minimal";
}) {
  const [attached, setAttached] = useState<BotSkillAttachment[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/skills`);
      if (!response.ok) {
        throw new Error("Could not load skills");
      }
      setAttached((await response.json()) as BotSkillAttachment[]);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Skills unavailable");
    }
  }, [botId]);

  useEffect(() => {
    void load();
    const timer = setInterval(() => void load(), 15000);
    return () => clearInterval(timer);
  }, [load]);

  if (error) {
    return (
      <p className="text-[11px] text-destructive" role="alert">
        {error}
      </p>
    );
  }

  if (attached.length === 0) {
    return (
      <p className="text-[11px] text-muted-foreground">
        No skills attached yet. Save successful work from chat or manage skills in settings.
      </p>
    );
  }

  return (
    <div className={variant === "minimal" ? "space-y-1" : "space-y-2"}>
      <ul className="space-y-0.5">
        {attached.map((skill) => (
          <li key={skill.skillId} className="text-[11px] text-foreground/90">
            <span className="font-medium">{skill.name}</span>
            <span className="text-muted-foreground">
              {" "}
              · v{skill.pinnedVersion ?? skill.currentVersion}
            </span>
          </li>
        ))}
      </ul>
      <Link
        href="/app/skills"
        className="inline-block text-[11px] font-medium text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
      >
        Manage skills
      </Link>
    </div>
  );
}
