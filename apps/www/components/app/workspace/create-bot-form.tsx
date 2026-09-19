"use client";

import { BotAvatarPicker } from "@/components/app/bot-avatar-picker";
import { BotModelSelect } from "@/components/app/bot-model-select";
import { ComputerSelect } from "@/components/app/computer-select";
import { ProviderStatusCard } from "@/components/app/provider-status-card";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { FormFields, FormItem } from "@/components/ui/form-item";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useProviderStatus } from "@/hooks/use-provider-status";
import type { ComputerSummary } from "@/lib/api-types";
import { DEFAULT_BOT_AVATAR_ID } from "@/lib/bot-avatars";
import { DEFAULT_BOT_MODEL_ID } from "@/lib/bot-models";
import {
  createBotAndMaybeStartWork,
  DEFAULT_BOT_INSTRUCTIONS,
  rememberQuickStartDraft,
  type CreateBotOutcome,
  type QuickStartSession,
} from "@/lib/bot-quick-start";
import { cloudHostFetch } from "@/lib/cloud-api";
import { cn } from "cn";
import { ChevronDown } from "@/components/icons/lucide";
import Link from "next/link";
import { useThisMacStatus } from "@/hooks/use-this-mac-status";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { isThisMacLiveForQuickStart } from "@/lib/this-mac-status";
import { useEffect, useRef, useState, type ReactNode } from "react";

export interface CreateBotFormProps {
  className?: string;
  /** When true, first task is required (first-run quick start). */
  requireTask?: boolean;
  submitLabel?: string;
  showProviderCard?: boolean;
  footer?: ReactNode;
  disabled?: boolean;
  onBusyChange?: (busy: boolean) => void;
  onOutcome: (outcome: CreateBotOutcome) => void;
}

export function CreateBotForm({
  className,
  requireTask = false,
  submitLabel,
  showProviderCard = true,
  footer,
  disabled = false,
  onBusyChange,
  onOutcome,
}: CreateBotFormProps) {
  const {
    connected,
    checking,
    checkFailed,
    providerUnavailable,
    challenge,
  } = useProviderStatus();

  const [name, setName] = useState("");
  const [role, setRole] = useState(DEFAULT_BOT_INSTRUCTIONS);
  const [task, setTask] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [avatarId, setAvatarId] = useState(DEFAULT_BOT_AVATAR_ID);
  const [model, setModel] = useState(DEFAULT_BOT_MODEL_ID);
  const [computers, setComputers] = useState<ComputerSummary[]>([]);
  const [computerId, setComputerId] = useState("");
  const [computersLoading, setComputersLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sessionRef = useRef<QuickStartSession>({});
  const { status: thisMac } = useThisMacStatus(isTauriRuntime());
  const thisMacLive = isThisMacLiveForQuickStart(thisMac);

  const needsCodex =
    showProviderCard &&
    (!connected || Boolean(challenge) || checkFailed || providerUnavailable);

  const taskTrimmed = task.trim();
  const canSubmit =
    !busy &&
    !disabled &&
    connected &&
    !checking &&
    !challenge &&
    Boolean(name.trim()) &&
    (requireTask ? Boolean(taskTrimmed) : true);

  useEffect(() => {
    if (!advancedOpen) {
      return;
    }
    const controller = new AbortController();
    setComputersLoading(true);
    cloudHostFetch("/v1/computers", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error("Could not load computers");
        }
        const items: ComputerSummary[] = await response.json();
        setComputers(items);
        setComputerId(items[0]?.id ?? "");
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Could not load computers");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setComputersLoading(false);
        }
      });
    return () => controller.abort();
  }, [advancedOpen]);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!canSubmit || busy) {
      return;
    }
    if (!connected || challenge) {
      setError("Connect ChatGPT to create a Bot.");
      return;
    }
    setBusy(true);
    onBusyChange?.(true);
    setError(null);
    try {
      const outcome = await createBotAndMaybeStartWork(
        {
          name,
          instructions: role,
          task: taskTrimmed || undefined,
          computerId: advancedOpen && computerId ? computerId : undefined,
          model: advancedOpen ? model : undefined,
          avatarId,
          thisMac: thisMacLive ? thisMac : null,
        },
        sessionRef.current,
      );
      if (outcome.status === "bot_ready_run_failed") {
        rememberQuickStartDraft(outcome.botId, outcome.task);
      }
      if (
        outcome.status === "failed" ||
        outcome.status === "needs_codex" ||
        outcome.status === "bot_ready_run_failed"
      ) {
        setError(outcome.error);
      }
      onOutcome(outcome);
    } finally {
      setBusy(false);
      onBusyChange?.(false);
    }
  }

  const defaultSubmitLabel = requireTask
    ? busy
      ? "Starting…"
      : "Start working"
    : taskTrimmed
      ? busy
        ? "Starting…"
        : "Create and start"
      : busy
        ? "Creating…"
        : "Create bot";

  return (
    <div className={cn("flex flex-col gap-4", className)}>
      {needsCodex ? (
        <ProviderStatusCard variant="featured" className="w-full max-w-none" />
      ) : null}
      <form onSubmit={(event) => void handleSubmit(event)}>
        <FormFields className="gap-3">
          <BotAvatarPicker
            value={avatarId}
            onChange={setAvatarId}
            disabled={busy || disabled}
            compact
            className="min-w-0"
          />
          <FormItem>
            <Label htmlFor="create-bot-name">Bot name</Label>
            <Input
              id="create-bot-name"
              required
              maxLength={100}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Scout"
              disabled={busy || disabled}
              autoComplete="off"
            />
          </FormItem>
          <FormItem>
            <Label htmlFor="create-bot-role">What should this Bot own?</Label>
            <Textarea
              id="create-bot-role"
              rows={4}
              className="field-sizing-fixed min-h-[6rem] resize-none overflow-y-auto text-base md:text-sm"
              maxLength={16000}
              value={role}
              onChange={(event) => setRole(event.target.value)}
              disabled={busy || disabled}
            />
          </FormItem>
          {thisMacLive && !advancedOpen ? (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="size-2 rounded-full bg-emerald-500" aria-hidden />
              Runs on · This Mac
            </p>
          ) : null}
          {requireTask ? (
            <FormItem>
              <Label htmlFor="create-bot-task">What should this Bot work on?</Label>
              <Textarea
                id="create-bot-task"
                required
                maxLength={100000}
                value={task}
                onChange={(event) => setTask(event.target.value)}
                placeholder="Research competitors and write a one-page brief"
                disabled={busy || disabled}
                className="min-h-28 text-base md:text-sm"
              />
            </FormItem>
          ) : null}

          <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
            <CollapsibleTrigger
              type="button"
              className="group flex w-full cursor-pointer items-center justify-between gap-2 rounded-md py-1.5 text-left text-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              Advanced
              <ChevronDown
                className="size-4 shrink-0 transition-transform duration-200 group-data-open:rotate-180"
                aria-hidden
              />
            </CollapsibleTrigger>
            <CollapsibleContent className="mt-2 space-y-3 border-t border-border/60 pt-3">
              {!requireTask ? (
                <FormItem>
                  <Label htmlFor="create-bot-task">First task (optional)</Label>
                  <Textarea
                    id="create-bot-task"
                    maxLength={100000}
                    value={task}
                    onChange={(event) => setTask(event.target.value)}
                    placeholder="Research competitors and write a one-page brief"
                    disabled={busy || disabled}
                    className="min-h-28 text-base md:text-sm"
                  />
                </FormItem>
              ) : null}
              <BotModelSelect
                id="create-bot-model"
                value={model}
                onValueChange={setModel}
                disabled={busy || disabled}
              />
              <ComputerSelect
                id="create-bot-computer"
                value={computerId}
                onValueChange={setComputerId}
                computers={computers}
                loading={computersLoading}
                disabled={computersLoading || busy || disabled}
              />
              {!computersLoading && advancedOpen && !computers.length ? (
                <p className="text-sm text-muted-foreground">
                  No computers yet. Elsewhere creates one automatically unless you pick
                  one here after{" "}
                  <Link href="/app/computers" className="underline">
                    adding a computer
                  </Link>
                  .
                </p>
              ) : null}
            </CollapsibleContent>
          </Collapsible>

          {error ? (
            <p className="text-sm text-destructive" role="alert">{error}</p>
          ) : null}

          <Button type="submit" size="lg" className="w-full" disabled={!canSubmit}>
            {submitLabel ?? defaultSubmitLabel}
          </Button>
          {!connected && !checking ? (
            <p className="text-center text-xs text-muted-foreground">
              Connect ChatGPT to create a Bot. Your answers stay here.
            </p>
          ) : (
            <p className="text-center text-xs text-muted-foreground">
              Uses your Codex allowance. Model and computer live in Advanced or Bot
              settings.
            </p>
          )}
          {footer}
        </FormFields>
      </form>
    </div>
  );
}
