export type RunRecoveryAction =
  | { kind: "connect_chatgpt"; label: string; settingsSection: "chatgpt" }
  | { kind: "open_settings"; label: string; settingsSection: "general" | "advanced" }
  | { kind: "take_control"; label: string }
  | { kind: "retry_message"; label: string }
  | { kind: "view_details"; label: string; href: string };

export interface RunRecoveryGuide {
  title: string;
  reason: string;
  continuation: string;
  actions: RunRecoveryAction[];
}

export function runRecoveryGuide(
  status: string,
  errorCode: string | null | undefined,
  runId: string,
): RunRecoveryGuide | null {
  if (status === "completed" || status === "running" || status === "queued") {
    return null;
  }

  const code = errorCode?.trim() ?? "";

  if (code === "codex_not_authenticated" || code === "codex_not_installed") {
    return {
      title: "Connect ChatGPT to continue",
      reason:
        "This bot runs on your ChatGPT plan. Connect once in settings, then send your message again.",
      continuation: "Your draft stays in the composer until you send it.",
      actions: [
        { kind: "connect_chatgpt", label: "Connect ChatGPT", settingsSection: "chatgpt" },
        { kind: "retry_message", label: "Try again" },
      ],
    };
  }

  if (code === "codex_not_chatgpt") {
    return {
      title: "ChatGPT plan required",
      reason: "Sign in with a ChatGPT account that includes Codex access.",
      continuation: "Update ChatGPT connection, then try again.",
      actions: [
        { kind: "connect_chatgpt", label: "Open ChatGPT settings", settingsSection: "chatgpt" },
      ],
    };
  }

  if (status === "interrupted" || code === "host_restart") {
    return {
      title: "Work was interrupted",
      reason:
        "The runner restarted while this assignment was in progress. Completed steps were not repeated.",
      continuation: "Send a follow-up in chat to continue where you left off.",
      actions: [
        { kind: "retry_message", label: "Continue in chat" },
        { kind: "view_details", label: "View details", href: `/app/work/${runId}` },
      ],
    };
  }

  if (status === "cancelled") {
    return {
      title: "Work stopped",
      reason: "This assignment was cancelled before it finished.",
      continuation: "Send a new message if you still want this done.",
      actions: [{ kind: "retry_message", label: "Send again" }],
    };
  }

  if (status === "failed") {
    const computerHint =
      code.includes("computer") || code.includes("sprite") || code.includes("provision");
    return {
      title: "Could not finish",
      reason: computerHint
        ? "The workspace was not ready or became unavailable during this assignment."
        : "Something blocked this assignment before it could complete.",
      continuation: "Fix the issue below, then try again in chat.",
      actions: [
        ...(computerHint
          ? [
              {
                kind: "open_settings" as const,
                label: "Open workspace settings",
                settingsSection: "advanced" as const,
              },
            ]
          : []),
        { kind: "retry_message", label: "Try again" },
        { kind: "view_details", label: "View details", href: `/app/work/${runId}` },
      ],
    };
  }

  return null;
}
