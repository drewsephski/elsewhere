"use client";

import { BotCreatureAvatar } from "@/components/app/bot-creature-avatar";
import { MultipleChoiceQuestionCard } from "@/components/app/multiple-choice-question-card";
import { Button } from "@/components/ui/button";
import { cloudHostFetch } from "@/lib/cloud-api";
import { cloudApiErrorFromResponse } from "@/lib/cloud-api-error";
import { formatUserFacingError } from "@/lib/format-api-error";
import {
  parseOnboardingState,
  type BotOnboardingQuestion,
  type BotOnboardingState,
} from "@/lib/bot-onboarding";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

interface BotOnboardingCardProps {
  botId: string;
  botName: string;
  avatarId?: string | null;
  autoStart?: boolean;
  workTakesPriority?: boolean;
  failedRunTakesPriority?: boolean;
  conversationEmpty?: boolean;
  onApplied?: () => void;
}

async function readOnboarding(response: Response, fallback: string): Promise<BotOnboardingState> {
  if (!response.ok) {
    throw await cloudApiErrorFromResponse(response, fallback);
  }
  const parsed = parseOnboardingState(await response.json());
  if (!parsed) {
    throw new Error(fallback);
  }
  return parsed;
}

export function BotOnboardingCard({
  botId,
  botName,
  avatarId,
  autoStart = false,
  workTakesPriority = false,
  failedRunTakesPriority = false,
  conversationEmpty = false,
  onApplied,
}: BotOnboardingCardProps) {
  const [state, setState] = useState<BotOnboardingState | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [customByQuestion, setCustomByQuestion] = useState<Record<string, string>>({});
  const [selectedByQuestion, setSelectedByQuestion] = useState<Record<string, string>>({});
  const [collapsed, setCollapsed] = useState(false);
  const startedRef = useRef(false);

  const load = useCallback(async () => {
    const response = await cloudHostFetch(`/v1/bots/${encodeURIComponent(botId)}/onboarding`);
    return readOnboarding(response, "Could not load Bot setup");
  }, [botId]);

  useEffect(() => {
    const controller = new AbortController();
    void load()
      .then((next) => {
        if (!controller.signal.aborted) {
          setState(next);
        }
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(formatUserFacingError(err, "Could not load Bot setup"));
        }
      });
    return () => controller.abort();
  }, [load]);

  async function handleStart(restart = false) {
    if (pending) {
      return;
    }
    startedRef.current = true;
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/bots/${encodeURIComponent(botId)}/onboarding/start`,
        {
          method: "POST",
          body: JSON.stringify({ restart }),
        },
      );
      const next = await readOnboarding(response, "Could not start Bot setup");
      setState(next);
      setCollapsed(false);
    } catch (err) {
      setError(formatUserFacingError(err, "Could not start Bot setup"));
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    if (!autoStart || !state || pending || workTakesPriority || failedRunTakesPriority) {
      return;
    }
    if (startedRef.current) {
      return;
    }
    if (state.status === "not_started" || (state.status === "in_progress" && !state.currentQuestion && !state.lastError)) {
      void handleStart(false);
    }
  }, [autoStart, failedRunTakesPriority, pending, state, workTakesPriority]);

  async function handleAnswer(question: BotOnboardingQuestion, optionId: string, customText?: string) {
    const isCustom =
      question.allowCustom &&
      (optionId === "other" ||
        question.options.find((option) => option.id === optionId)?.label.toLowerCase().startsWith("something else") ||
        question.options.find((option) => option.id === optionId)?.label.toLowerCase().startsWith("other"));
    if (isCustom && !customText?.trim()) {
      setSelectedByQuestion((current) => ({ ...current, [question.id]: optionId }));
      return;
    }
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/bots/${encodeURIComponent(botId)}/onboarding/answer`,
        {
          method: "POST",
          body: JSON.stringify({
            questionId: question.id,
            optionId: customText?.trim() ? undefined : optionId,
            customText: customText?.trim() || undefined,
            revision: state?.revision,
          }),
        },
      );
      const next = await readOnboarding(response, "Could not save that choice");
      setState(next);
    } catch (err) {
      setError(formatUserFacingError(err, "Could not save that choice"));
    } finally {
      setPending(false);
    }
  }

  async function handleApply() {
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/bots/${encodeURIComponent(botId)}/onboarding/apply`,
        { method: "POST", body: JSON.stringify({ revision: state?.revision }) },
      );
      const next = await readOnboarding(response, "Could not apply Bot setup");
      setState(next);
      onApplied?.();
    } catch (err) {
      setError(formatUserFacingError(err, "Could not apply Bot setup"));
    } finally {
      setPending(false);
    }
  }

  async function handleDismiss() {
    setPending(true);
    setError(null);
    try {
      const response = await cloudHostFetch(
        `/v1/bots/${encodeURIComponent(botId)}/onboarding/dismiss`,
        { method: "POST", body: JSON.stringify({ revision: state?.revision }) },
      );
      const next = await readOnboarding(response, "Could not skip Bot setup");
      setState(next);
      setCollapsed(true);
    } catch (err) {
      setError(formatUserFacingError(err, "Could not skip Bot setup"));
    } finally {
      setPending(false);
    }
  }

  if (!state) {
    return error ? (
      <p className="text-[12px] text-destructive" role="alert">
        {error}
      </p>
    ) : null;
  }

  const active =
    state.status === "in_progress" ||
    state.status === "ready_to_apply" ||
    (autoStart && state.status === "not_started");
  const quietOffer =
    !active &&
    (state.status === "not_started" || state.status === "dismissed") &&
    !conversationEmpty;

  if (workTakesPriority && active) {
    return null;
  }
  if (failedRunTakesPriority && (autoStart || active)) {
    if (quietOffer || state.status === "not_started" || state.status === "dismissed") {
      return (
        <button
          type="button"
          className="text-[12px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
          onClick={() => void handleStart(state.status !== "in_progress")}
        >
          Finish setting up {botName}
        </button>
      );
    }
    return null;
  }

  if (state.status === "completed" || (state.status === "dismissed" && !quietOffer)) {
    return null;
  }

  if (quietOffer || collapsed) {
    return (
      <div className="flex justify-center">
        <button
          type="button"
          className="text-[12px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
          onClick={() => void handleStart(state.status !== "in_progress")}
        >
          Finish setting up {botName}
        </button>
      </div>
    );
  }

  if (state.status === "not_started" && !autoStart && conversationEmpty) {
    return (
      <SetupShell name={botName} avatarId={avatarId}>
        <p className="text-[14px] font-medium tracking-tight">
          {botName} is ready. Want to spend about a minute tuning how it works?
        </p>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Button type="button" size="sm" disabled={pending} onClick={() => void handleStart(false)}>
            {pending ? "Starting…" : "Tune this Bot"}
          </Button>
          <Button type="button" size="sm" variant="ghost" disabled={pending} onClick={() => void handleDismiss()}>
            Skip for now
          </Button>
        </div>
        {error ? (
          <p className="mt-2 text-[11px] text-destructive" role="alert">
            {error}
          </p>
        ) : null}
      </SetupShell>
    );
  }

  if (state.status === "ready_to_apply" && state.draft) {
    const draft = state.draft;
    return (
      <SetupShell name={botName} avatarId={avatarId}>
        <p className="text-[14px] font-medium tracking-tight">Review {botName}&apos;s setup</p>
        <p className="mt-1 text-[12px] text-muted-foreground">{draft.summary}</p>
        <ReviewSection title={`What ${botName} owns`} body={draft.owns || draft.instructions} />
        <ReviewSection
          title={`How ${botName} should work`}
          body={
            draft.workingStyle ||
            "Existing approval and permission policies stay in place. Setup never enables tools, connectors, or extra access."
          }
        />
        {draft.context ? (
          <ReviewSection title={`Context ${botName} should remember`} body={draft.context} />
        ) : null}
        {draft.suggestedFirstTasks.length ? (
          <div className="mt-3">
            <p className="text-[11px] font-medium text-muted-foreground">Suggested first tasks</p>
            <ul className="mt-1 list-disc space-y-1 pl-4 text-[12px] text-foreground/85">
              {draft.suggestedFirstTasks.map((task) => (
                <li key={task}>{task}</li>
              ))}
            </ul>
          </div>
        ) : null}
        {draft.suggestedCapabilities.length ? (
          <p className="mt-2 text-[11px] text-muted-foreground">
            Later you can add: {draft.suggestedCapabilities.join(", ")}. Setup does not turn these on.
          </p>
        ) : null}
        <div className="mt-3 flex flex-wrap gap-2">
          <Button type="button" size="sm" disabled={pending} onClick={() => void handleApply()}>
            {pending ? "Applying…" : "Apply setup"}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={pending}
            onClick={() => void handleStart(true)}
          >
            Edit answers
          </Button>
          <Button type="button" size="sm" variant="ghost" disabled={pending} onClick={() => void handleDismiss()}>
            Skip for now
          </Button>
        </div>
        {error ? (
          <p className="mt-2 text-[11px] text-destructive" role="alert">
            {error}
          </p>
        ) : null}
      </SetupShell>
    );
  }

  const question = state.currentQuestion;
  if (!question) {
    return (
      <SetupShell name={botName} avatarId={avatarId} thinking={pending}>
        <p className="text-[14px] font-medium tracking-tight">
          {botName} is ready. Want to spend about a minute tuning how it works?
        </p>
        <p className="mt-1 text-[12px] text-muted-foreground">
          {pending ? "Luna is preparing a few questions…" : state.lastError ?? error ?? "Setup can continue when you are ready."}
        </p>
        <div className="mt-3 flex flex-wrap gap-2">
          <Button type="button" size="sm" disabled={pending} onClick={() => void handleStart(false)}>
            {pending ? "Working…" : "Retry"}
          </Button>
          <Button type="button" size="sm" variant="ghost" disabled={pending} onClick={() => void handleDismiss()}>
            Skip for now
          </Button>
        </div>
      </SetupShell>
    );
  }

  const selectedId = selectedByQuestion[question.id] ?? null;
  const customValue = customByQuestion[question.id] ?? "";

  return (
    <div className="space-y-3">
      <SetupShell name={botName} avatarId={avatarId}>
        <p className="text-[14px] font-medium tracking-tight">
          {botName} is ready. Want to spend about a minute tuning how it works?
        </p>
      </SetupShell>
      <MultipleChoiceQuestionCard
        title={`${botName} setup`}
        prompt={question.prompt}
        helper={question.helper}
        options={question.options}
        selectedId={selectedId}
        allowCustom={question.allowCustom}
        customValue={customValue}
        onCustomChange={(value) =>
          setCustomByQuestion((current) => ({ ...current, [question.id]: value }))
        }
        pending={pending}
        error={error ?? state.lastError ?? null}
        progress={`${question.index} of up to ${question.maxQuestions}`}
        tone="neutral"
        continuation="Recommended, and you can keep chatting either way."
        onSelect={(optionId) => void handleAnswer(question, optionId)}
        onSubmitCustom={() => void handleAnswer(question, selectedId ?? "other", customValue)}
        onSkip={() => void handleDismiss()}
      />
    </div>
  );
}

function SetupShell({
  name,
  avatarId,
  thinking = false,
  children,
}: {
  name: string;
  avatarId?: string | null;
  thinking?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="flex items-start gap-3">
      <BotCreatureAvatar
        name={name}
        avatarId={avatarId ?? DEFAULT_BOT_AVATAR_ID}
        size="lg"
        animated={thinking}
      />
      <div className="min-w-0 flex-1 rounded-xl border border-border/70 bg-muted/20 p-3">
        {children}
      </div>
    </div>
  );
}

function ReviewSection({ title, body }: { title: string; body: string }) {
  return (
    <section className="mt-3">
      <h3 className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        {title}
      </h3>
      <p className="mt-1 whitespace-pre-wrap text-[13px] leading-5 text-foreground/90">{body}</p>
    </section>
  );
}
