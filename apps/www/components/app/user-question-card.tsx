"use client";

import { cloudHostFetch } from "@/lib/cloud-api";
import { cn } from "cn";
import { useState } from "react";

export interface UserQuestionPayload {
  questionId: string;
  runId: string;
  question: string;
  options: string[];
  selectedIndex?: number | null;
  status?: string;
}

export function userQuestionFromPayload(
  runId: string,
  payload: Record<string, unknown>,
): UserQuestionPayload | null {
  const questionId = typeof payload.questionId === "string" ? payload.questionId : null;
  const question = typeof payload.question === "string" ? payload.question : null;
  const options = Array.isArray(payload.options)
    ? payload.options.filter((item): item is string => typeof item === "string")
    : [];
  if (!questionId || !question || options.length < 2) {
    return null;
  }
  return {
    questionId,
    runId,
    question,
    options,
    selectedIndex: typeof payload.selectedIndex === "number" ? payload.selectedIndex : null,
    status: typeof payload.status === "string" ? payload.status : undefined,
  };
}

export function UserQuestionCard({
  question,
  botName,
}: {
  question: UserQuestionPayload;
  botName?: string;
}) {
  const [selected, setSelected] = useState<number | null>(
    typeof question.selectedIndex === "number" ? question.selectedIndex : null,
  );
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const answered = selected !== null || question.status === "answered";

  async function handleSelect(index: number) {
    if (answered || pending) {
      return;
    }
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/runs/${encodeURIComponent(question.runId)}/questions/${encodeURIComponent(question.questionId)}/answer`,
        {
          method: "POST",
          body: JSON.stringify({ selectedIndex: index }),
        },
      );
      if (!response.ok) {
        const body = (await response.json().catch(() => ({}))) as { error?: string };
        throw new Error(body.error ?? "Could not save that choice");
      }
      setSelected(index);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save that choice");
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="rounded-xl border border-border bg-card p-3">
      <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        {botName ? `${botName} needs a choice` : "Needs a choice"}
      </p>
      <p className="mt-1 text-[13px] font-medium text-foreground">{question.question}</p>
      <div className="mt-2 flex flex-wrap gap-2">
        {question.options.map((option, index) => {
          const isSelected = selected === index;
          return (
            <button
              key={`${option}-${index}`}
              type="button"
              disabled={answered || pending}
              onClick={() => void handleSelect(index)}
              className={cn(
                "rounded-full border px-3 py-1.5 text-[12px] transition-colors",
                isSelected
                  ? "border-primary bg-primary text-primary-foreground"
                  : "border-border bg-surface-raised text-foreground hover:bg-surface-hover",
                (answered || pending) && !isSelected ? "opacity-50" : null,
              )}
              aria-pressed={isSelected}
            >
              {option}
            </button>
          );
        })}
      </div>
      {error ? (
        <p className="mt-2 text-[11px] text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
