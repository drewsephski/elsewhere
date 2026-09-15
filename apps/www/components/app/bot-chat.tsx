"use client";
import { cloudHostFetch } from "@/lib/cloud-api";
import type { BotSummary, CreateRunResponse } from "@/lib/api-types";
import {
  resolveSkillSlashInvocation,
  type SkillCatalogEntry,
} from "@/lib/skill-invocation";
import { useEffect, useMemo, useRef, useState } from "react";
import { Badge } from "@/components/reui/badge";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Label } from "@/components/ui/label";
import { BotSettings } from "./bot-settings";
import { BotContext } from "./bot-context";
import { RecentRunsPanel } from "./recent-runs-panel";

export function BotChat({ botId }: { botId: string }) {
  const router = useRouter();
  const [bot, setBot] = useState<BotSummary | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [skills, setSkills] = useState<SkillCatalogEntry[]>([]);
  const request = useRef<{
    task: string;
    skillId: string | null;
    key: string;
  } | null>(null);
  const skillInvocation = useMemo(
    () => resolveSkillSlashInvocation(message, skills),
    [message, skills],
  );
  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch("/v1/skills", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) return;
        setSkills((await response.json()) as SkillCatalogEntry[]);
      })
      .catch(() => undefined);
    return () => controller.abort();
  }, []);
  useEffect(() => {
    const controller = new AbortController();
    cloudHostFetch(`/v1/bots/${botId}`, { signal: controller.signal }).then(async response => {
      if (!response.ok) throw new Error("Bot not found");
      setBot(await response.json());
    }).catch(err => { if (!controller.signal.aborted) setError(err instanceof Error ? err.message : "Could not load bot"); });
    return () => controller.abort();
  }, [botId]);
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const trimmed = message.trim();
    const resolved = resolveSkillSlashInvocation(trimmed, skills);
    const task = resolved?.task ?? trimmed;
    if (pending || !task) return;
    setPending(true); setError(null);
    const skillId = resolved?.skill.id ?? null;
    if (
      request.current?.task !== task ||
      request.current?.skillId !== skillId
    ) {
      request.current = { task, skillId, key: crypto.randomUUID() };
    }
    try {
      const response = await cloudHostFetch("/v1/runs", {
        method: "POST",
        headers: { "Idempotency-Key": request.current.key },
        body: JSON.stringify({
          botId,
          message: task,
          skillInvocation: resolved
            ? { skillId: resolved.skill.id }
            : undefined,
        }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not delegate work");
      router.push(`/app/work/${(body as CreateRunResponse).runId}`);
    } catch (err) { setError(err instanceof Error ? err.message : "Could not delegate work"); }
    finally { setPending(false); }
  }
  return (
    <div className="mt-6 space-y-6">
      <section className="surface-card">
        <h1 className="text-2xl font-semibold">{bot?.name ?? "Your bot"}</h1>
        <p className="mt-2 text-sm text-muted-foreground">Give your bot a job, useful context, and a clear description of the result you want.</p>
        <form className="mt-5" onSubmit={event => void submit(event)}>
          <FormFields>
            <FormItem>
              <Label htmlFor="work-message">What needs doing?</Label>
              <textarea id="work-message" className="min-h-32 w-full rounded-xl border border-border bg-background px-4 py-3 text-sm" placeholder="Type /skill-slug to invoke a skill, or describe the work normally." value={message} onChange={event => setMessage(event.target.value)} disabled={pending} maxLength={100000} />
              {skillInvocation ? (
                <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                  <span>Skill invocation:</span>
                  <Badge variant="primary-light">{skillInvocation.skill.name}</Badge>
                  <span>Task sent without the /{skillInvocation.skill.slug} prefix.</span>
                </div>
              ) : null}
            </FormItem>
            <div className="flex flex-wrap items-center justify-between gap-3"><p className="text-xs text-muted-foreground">Progress is saved. Computer changes need your approval.</p><Button type="submit" disabled={pending || !(skillInvocation?.task ?? message.trim()) || !bot?.computerId}>{pending ? "Saving work…" : "Delegate work"}</Button></div>
          </FormFields>
        </form>
        {bot && !bot.computerId ? <p className="mt-3 text-sm text-amber-700">Assign this bot a computer before delegating work.</p> : null}
        {error ? <p className="mt-3 text-sm text-red-700" role="alert">{error}</p> : null}
      </section>
      <BotContext botId={botId} />
      <RecentRunsPanel botId={botId} />
      {bot ? <BotSettings bot={bot} onSaved={setBot} /> : null}
    </div>
  );
}
