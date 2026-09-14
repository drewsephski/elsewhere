export function workStatus(status: string): string {
  return ({ queued: "Waiting", running: "Working", completed: "Finished", complete: "Finished", failed: "Needs attention", interrupted: "Interrupted", cancelled: "Stopped" } as Record<string, string>)[status] ?? status;
}

export function activityText(event: string, payload: Record<string, unknown>): string | null {
  if (event === "approval_resolved") return `Approval ${String(payload.decision ?? "updated")}`;
  if (event === "queued") return "Work saved. Waiting for an available computer.";
  if (event === "host_restart") return "Work was interrupted when the runner restarted. Completed actions have not been repeated.";
  if (event === "terminal") return typeof payload.status === "string" ? workStatus(payload.status) : "Work updated";
  if (event === "cancelled") return "Work stopped before it started.";
  const tool = typeof payload.tool === "string" ? payload.tool : typeof payload.name === "string" ? payload.name : "";
  const tools: Record<string, string> = { workspace_list: "Exploring files", workspace_read: "Reading a file", workspace_write: "Writing a file", workspace_exec: "Working in the terminal" };
  if (tool && tools[tool]) return tools[tool];
  if (typeof payload.detail === "string") return payload.detail;
  if (typeof payload.status === "string") return workStatus(payload.status);
  if (/delta|token|assistant/.test(event)) return null;
  if (/start/.test(event)) return "Started work on the computer";
  if (/tool/.test(event)) return "Computer activity recorded";
  return null;
}
