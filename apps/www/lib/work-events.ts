import { isSubagentEvent } from "./subagent-events";

export function workStatus(status: string): string {
  return ({ queued: "Waiting", running: "Working", completed: "Finished", complete: "Finished", failed: "Needs attention", interrupted: "Interrupted", cancelled: "Stopped" } as Record<string, string>)[status] ?? status;
}

function toolLabel(tool: string, payload: Record<string, unknown>): string | null {
  const tools: Record<string, string> = {
    workspace_list: "Exploring files",
    workspace_read: "Reading a file",
    workspace_write: "Writing a file",
    workspace_exec: "Working in the terminal",
    browser_navigate: "Opening a web page",
    browser_snapshot: "Inspecting the web page",
    browser_click: "Clicking on the page",
    browser_type: "Typing on the page",
    browser_screenshot: "Capturing a screenshot",
    browser_download: "Downloading a file",
    browser_request_human: "Waiting for you in the browser",
    bot_list: "Checking available Bots",
    bot_delegate: "Handing work to another Bot",
    run_subagent: "Running a temporary helper",
    recall_memory: "Looking up remembered context",
    remember: "Saving a memory",
    forget_memory: "Forgetting a memory",
    ask_user: "Waiting for your choice",
    attachment_list: "Reviewing attachments",
    attachment_read: "Reading an attachment",
  };
  const base = tools[tool];
  if (!base) {
    return null;
  }
  const args =
    payload.arguments && typeof payload.arguments === "object"
      ? (payload.arguments as Record<string, unknown>)
      : null;
  if (tool === "browser_navigate" && args && typeof args.url === "string") {
    try {
      const host = new URL(args.url).hostname;
      return `${base} (${host})`;
    } catch {
      return base;
    }
  }
  return base;
}

export function activityText(event: string, payload: Record<string, unknown>): string | null {
  if (isSubagentEvent(event)) {
    return null;
  }
  if (event === "bot_delegation_queued") {
    const name = typeof payload.targetBotName === "string" ? payload.targetBotName : "another Bot";
    return `Handed work to ${name} (queued)`;
  }
  if (event === "bot_delegation_running") {
    return "Recipient Bot started working";
  }
  if (event === "bot_delegation_completed") {
    return "Delegated work finished";
  }
  if (event === "bot_delegation_failed") {
    return "Delegated work failed";
  }
  if (event === "human_intervention_requested") {
    return "Bot needs your help in the browser";
  }
  if (event === "human_intervention_resolved") {
    return "You returned control to the Bot";
  }
  if (event === "user_question_requested" || event === "user_question_answered" || event === "user_question_cancelled") {
    return null;
  }
  if (event === "approval_resolved") {
    const decision = String(payload.decision ?? "");
    if (decision === "approved") return "You allowed the action";
    if (decision === "denied") return "You denied the action";
    if (decision === "cancelled") return "Approval cancelled";
    if (decision === "expired") return "Approval expired";
    return null;
  }
  if (event === "tool_result" && payload.ok === false) {
    const tool = typeof payload.tool === "string" ? payload.tool : typeof payload.name === "string" ? payload.name : "";
    const labeled = tool ? toolLabel(tool, payload) : null;
    if (labeled) {
      return `${labeled} did not complete`;
    }
    return "A step did not complete";
  }
  if (event === "permission_policy_resolved") {
    if (payload.decision === "allow") {
      return null;
    }
    if (payload.decision === "deny") {
      const labeled = typeof payload.tool === "string" ? toolLabel(payload.tool, payload) : null;
      return labeled ? `Blocked: ${labeled}` : "This Bot is not allowed to do that";
    }
    return null;
  }
  if (event === "queued") return "Work saved. Waiting for an available computer.";
  if (event === "host_restart") return "Work was interrupted when the runner restarted. Completed actions have not been repeated.";
  if (event === "terminal") return typeof payload.status === "string" ? workStatus(payload.status) : "Work updated";
  if (event === "cancelled") return "Work stopped before it started.";
  const tool = typeof payload.tool === "string" ? payload.tool : typeof payload.name === "string" ? payload.name : "";
  const labeled = tool ? toolLabel(tool, payload) : null;
  if (labeled) return labeled;
  if (typeof payload.detail === "string") return payload.detail;
  if (typeof payload.status === "string") return workStatus(payload.status);
  if (/delta|token|assistant/.test(event)) return null;
  if (/start/.test(event)) return "Started work on the computer";
  if (/tool/.test(event)) return "Computer activity recorded";
  return null;
}
