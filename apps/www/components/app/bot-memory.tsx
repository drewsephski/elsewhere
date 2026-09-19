"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import { BotContext } from "@/components/app/bot-context";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

export interface BotMemoryItem {
  id: string;
  kind: string;
  content: string;
  sourceKind: string;
  status: string;
  createdAt: string;
  lastConfirmedAt: string;
}

function provenanceLabel(memory: BotMemoryItem): string {
  const date = new Date(memory.createdAt);
  const when = Number.isNaN(date.getTime())
    ? ""
    : date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  if (memory.sourceKind === "manual") {
    return when ? `Added by you on ${when}` : "Added by you";
  }
  if (memory.sourceKind === "explicit_tool") {
    return when ? `Saved by this Bot on ${when}` : "Saved by this Bot";
  }
  return when ? `Learned from your conversation on ${when}` : "Learned from a conversation";
}

function kindLabel(kind: string): string {
  return kind.charAt(0).toUpperCase() + kind.slice(1);
}

export function BotMemoryPanel({
  botId,
  learnFromConversations,
  onLearnChanged,
  embedded = false,
}: {
  botId: string;
  learnFromConversations: boolean;
  onLearnChanged?: (enabled: boolean) => void;
  embedded?: boolean;
}) {
  const [memories, setMemories] = useState<BotMemoryItem[]>([]);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [learn, setLearn] = useState(learnFromConversations);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    setLearn(learnFromConversations);
  }, [learnFromConversations]);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const params = new URLSearchParams({ status: "active", limit: "50" });
      if (query.trim()) params.set("query", query.trim());
      const response = await cloudHostFetch(`/v1/bots/${botId}/memories?${params.toString()}`);
      if (!response.ok) throw new Error("Could not load memories");
      const body: unknown = await response.json();
      setMemories(Array.isArray(body) ? (body as BotMemoryItem[]) : []);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load memories");
    } finally {
      setBusy(false);
    }
  }, [botId, query]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleLearnChange(enabled: boolean) {
    setLearn(enabled);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}`, {
        method: "PATCH",
        body: JSON.stringify({ learnFromConversations: enabled }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not update learning");
      onLearnChanged?.(Boolean(body.learnFromConversations));
    } catch (err) {
      setLearn(!enabled);
      setError(err instanceof Error ? err.message : "Could not update learning");
    }
  }

  async function handleForget(memory: BotMemoryItem) {
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/memories/${memory.id}`, {
        method: "DELETE",
      });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(body.error ?? "Could not forget memory");
      }
      setMemories((current) => current.filter((item) => item.id !== memory.id));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not forget memory");
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveEdit(memory: BotMemoryItem) {
    setBusy(true);
    setError(null);
    try {
      const response = await cloudHostFetch(`/v1/bots/${botId}/memories/${memory.id}`, {
        method: "PATCH",
        body: JSON.stringify({ content: draft }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not update memory");
      setMemories((current) =>
        current.map((item) => (item.id === memory.id ? { ...item, ...body } : item)),
      );
      setEditingId(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update memory");
    } finally {
      setBusy(false);
    }
  }

  const empty = useMemo(() => memories.length === 0 && !query.trim(), [memories, query]);

  return (
    <div className={embedded ? "min-w-0 space-y-3" : "space-y-6"}>
      <section className={embedded ? "min-w-0" : "surface-card space-y-3"}>
        <div className={embedded ? "" : "p-4"}>
          <h2 className="text-sm font-semibold">Pinned Context</h2>
          <p className="mt-1 text-[11px] leading-snug text-muted-foreground">
            Information this Bot should always have available.
          </p>
          <div className="mt-2">
            <BotContext botId={botId} embedded />
          </div>
        </div>
      </section>

      <section className={embedded ? "min-w-0 space-y-2" : "surface-card space-y-3 p-4"}>
        <div>
          <h2 className="text-sm font-semibold">Learned Memories</h2>
          <p className="mt-1 text-[11px] leading-snug text-muted-foreground">
            Individual facts retrieved when they are relevant. These are not instructions.
          </p>
        </div>
        <div className="flex items-start gap-2">
          <Checkbox
            id={`learn-${botId}`}
            checked={learn}
            onCheckedChange={(value) => void handleLearnChange(value === true)}
            aria-label="Learn from conversations"
          />
          <div className="min-w-0">
            <Label htmlFor={`learn-${botId}`} className="text-xs font-medium">
              Learn from conversations
            </Label>
            <p className="text-[11px] leading-snug text-muted-foreground">
              When enabled, this Bot may remember stable preferences and project facts from your
              conversations.
            </p>
          </div>
        </div>
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search memories"
          aria-label="Search memories"
          className="h-8 text-xs"
        />
        {error ? (
          <p role="alert" className="text-[11px] text-destructive">
            {error}
          </p>
        ) : null}
        {empty ? (
          <p className="text-[11px] leading-snug text-muted-foreground">
            This Bot hasn't learned anything yet. It can remember useful preferences, project facts,
            and working conventions as you use it.
          </p>
        ) : (
          <ul className="space-y-2">
            {memories.map((memory) => (
              <li
                key={memory.id}
                className="rounded-lg border border-border bg-surface-raised p-2"
              >
                {editingId === memory.id ? (
                  <div className="space-y-2">
                    <Textarea
                      value={draft}
                      onChange={(event) => setDraft(event.target.value)}
                      maxLength={2048}
                      className="min-h-16 text-xs"
                      aria-label="Edit memory"
                    />
                    <div className="flex justify-end gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => setEditingId(null)}
                      >
                        Cancel
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        disabled={busy || !draft.trim()}
                        onClick={() => void handleSaveEdit(memory)}
                      >
                        Save
                      </Button>
                    </div>
                  </div>
                ) : (
                  <>
                    <p className="text-xs leading-5 text-foreground">{memory.content}</p>
                    <p className="mt-1 text-[11px] text-muted-foreground">
                      {kindLabel(memory.kind)} · {provenanceLabel(memory)}
                    </p>
                    <div className="mt-2 flex justify-end gap-2">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-xs"
                        onClick={() => {
                          setEditingId(memory.id);
                          setDraft(memory.content);
                        }}
                      >
                        Edit
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-xs text-muted-foreground hover:text-destructive"
                        disabled={busy}
                        onClick={() => void handleForget(memory)}
                      >
                        Forget
                      </Button>
                    </div>
                  </>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
