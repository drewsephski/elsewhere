"use client";

import { useCallback, useEffect, useState } from "react";
import { cloudHostFetch } from "@/lib/cloud-api";
import {
  groupedPolicyActions,
  POLICY_DECISIONS,
  type PolicyAction,
  type PolicyCatalog,
  type PolicyDecision,
} from "@/lib/permission-policies";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { cn } from "cn";

interface PermissionPolicyEditorProps {
  endpoint: string;
  mode: "owner" | "bot";
  embedded?: boolean;
}

function inheritHint(action: PolicyAction, mode: "owner" | "bot"): string {
  if (mode === "owner") {
    return action.source === "default"
      ? "Asks before this action"
      : "Account default";
  }
  if (action.inherited) {
    return action.source === "owner"
      ? `Uses account default (${labelFor(action.inheritedDecision)})`
      : "Uses account default (Ask)";
  }
  return "This Bot";
}

function labelFor(decision: PolicyDecision): string {
  return POLICY_DECISIONS.find((item) => item.value === decision)?.label ?? decision;
}

export function PermissionPolicyEditor({
  endpoint,
  mode,
  embedded = false,
}: PermissionPolicyEditorProps) {
  const [catalog, setCatalog] = useState<PolicyCatalog | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const response = await cloudHostFetch(endpoint);
    if (!response.ok) {
      throw new Error("Could not load permissions");
    }
    setCatalog((await response.json()) as PolicyCatalog);
  }, [endpoint]);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    void load().catch((err: unknown) => {
      if (!cancelled) {
        setError(err instanceof Error ? err.message : "Could not load permissions");
      }
    });
    return () => {
      cancelled = true;
    };
  }, [load]);

  async function handleChange(action: string, decision: PolicyDecision | null) {
    if (busyAction) return;
    setBusyAction(action);
    setError(null);
    try {
      const response = await cloudHostFetch(endpoint, {
        method: "PUT",
        body: JSON.stringify({ policies: [{ action, decision }] }),
      });
      const body = (await response.json().catch(() => ({}))) as PolicyCatalog & {
        error?: string;
      };
      if (!response.ok) {
        throw new Error(body.error ?? "Could not update permissions");
      }
      setCatalog(body);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update permissions");
    } finally {
      setBusyAction(null);
    }
  }

  const groups = catalog ? groupedPolicyActions(catalog.actions) : [];
  const labelClass = embedded
    ? "text-[11px] font-medium text-muted-foreground"
    : "text-xs font-medium text-muted-foreground";

  return (
    <div className="min-w-0 space-y-3">
      <div>
        <p className={embedded ? "text-xs font-medium text-foreground" : "text-sm font-medium"}>
          Permissions
        </p>
        <p className={embedded ? "mt-0.5 text-[11px] leading-snug text-muted-foreground" : "mt-1 text-xs text-muted-foreground"}>
          {mode === "bot"
            ? "Applies whenever this Bot works on its own, including Routines."
            : "Applies to every Bot unless that Bot has its own setting."}
        </p>
      </div>
      {groups.map((group) => (
        <section key={group.group} className="min-w-0 space-y-1.5">
          <h3 className={labelClass}>{group.groupLabel}</h3>
          {group.actions.map((action) => (
            <PolicyActionRow
              key={action.action}
              action={action}
              mode={mode}
              embedded={embedded}
              busy={busyAction === action.action}
              disabled={busyAction !== null}
              onChange={(decision) => void handleChange(action.action, decision)}
              onReset={
                mode === "bot" && !action.inherited
                  ? () => void handleChange(action.action, null)
                  : undefined
              }
            />
          ))}
        </section>
      ))}
      {mode === "bot" || mode === "owner" ? (
        <p className={embedded ? "text-[11px] leading-snug text-muted-foreground" : "text-xs text-muted-foreground"}>
          Connected apps stay read-only for now. Mutation permissions will appear here when those actions exist.
        </p>
      ) : null}
      {error ? (
        <p role="alert" className={embedded ? "text-[11px] text-destructive" : "text-sm text-destructive"}>
          {error}
        </p>
      ) : null}
    </div>
  );
}

function PolicyActionRow({
  action,
  mode,
  embedded,
  busy,
  disabled,
  onChange,
  onReset,
}: {
  action: PolicyAction;
  mode: "owner" | "bot";
  embedded: boolean;
  busy: boolean;
  disabled: boolean;
  onChange: (decision: PolicyDecision) => void;
  onReset?: () => void;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <Label className={embedded ? "text-xs text-foreground" : "text-sm text-foreground"}>
          {action.label}
        </Label>
        <p className="text-[11px] leading-snug text-muted-foreground">
          {inheritHint(action, mode)}
          {onReset ? " · " : null}
          {onReset ? (
            <button
              type="button"
              className="underline-offset-2 hover:underline disabled:opacity-50"
              disabled={disabled}
              onClick={onReset}
            >
              Use account default
            </button>
          ) : null}
        </p>
      </div>
      <div
        className="inline-flex shrink-0 rounded-md border border-border p-0.5"
        role="radiogroup"
        aria-label={`${action.label} permission`}
      >
        {POLICY_DECISIONS.map((option) => {
          const selected = action.decision === option.value;
          return (
            <Button
              key={option.value}
              type="button"
              size="sm"
              role="radio"
              aria-checked={selected}
              variant={selected ? "default" : "ghost"}
              className={cn(
                "h-7 px-2 text-[11px]",
                selected ? "" : "text-muted-foreground",
              )}
              disabled={disabled}
              onClick={() => onChange(option.value)}
            >
              {busy && selected ? "…" : option.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}
