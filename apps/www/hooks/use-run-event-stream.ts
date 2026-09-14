"use client";

import { useRequireActiveRun } from "@/contexts/active-run-context";

/** @deprecated Prefer `useActiveRun` from `@/contexts/active-run-context`. */
export function useRunEventStream(_runId: string | null) {
  const active = useRequireActiveRun();
  return {
    detail: active.detail,
    timeline: active.timeline,
    assistantStream: active.assistantStream,
    error: active.error,
    connection: active.connection,
  };
}

export type { RunActivityItem } from "@/contexts/active-run-context";
