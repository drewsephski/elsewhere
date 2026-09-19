"use client";

import { MultipleChoiceQuestionCard } from "@/components/app/multiple-choice-question-card";
import { NeedsYouCard } from "@/components/app/needs-you-card";
import { cloudHostFetch } from "@/lib/cloud-api";
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
  const [saved, setSaved] = useState(
    question.status === "answered" || typeof question.selectedIndex === "number",
  );
  const cancelled = question.status === "cancelled";

  async function handleSelect(index: number) {
    if (saved || pending || cancelled) {
      return;
    }
    setSelected(index);
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
      setSaved(true);
    } catch (err) {
      setSelected(null);
      setError(err instanceof Error ? err.message : "Could not save that choice");
    } finally {
      setPending(false);
    }
  }

  const chosenIndex =
    selected !== null
      ? selected
      : typeof question.selectedIndex === "number"
        ? question.selectedIndex
        : null;
  const chosenLabel =
    chosenIndex !== null && question.options[chosenIndex]
      ? question.options[chosenIndex]
      : null;

  if (cancelled) {
    return (
      <NeedsYouCard
        tone="resolved"
        title="Choice cancelled"
        reason="This question was dismissed before an answer was saved."
      />
    );
  }

  if (saved && chosenLabel) {
    return (
      <NeedsYouCard
        tone="resolved"
        title={`Choice saved · ${chosenLabel}`}
        reason={question.question}
      />
    );
  }

  return (
    <MultipleChoiceQuestionCard
      title={botName ? `${botName} needs your choice` : "Your bot needs your choice"}
      prompt={question.question}
      options={question.options.map((option, index) => ({
        id: String(index),
        label: option,
      }))}
      selectedId={chosenIndex !== null ? String(chosenIndex) : null}
      pending={pending}
      disabled={saved}
      error={error}
      loadingLabel="Saving your choice…"
      continuation="Pick one option to continue this assignment."
      onSelect={(optionId) => void handleSelect(Number(optionId))}
    />
  );
}
