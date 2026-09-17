export type MessageActionIntent = "archive" | "delete";

export interface MessageActionCopy {
  label: string;
  confirmTitle: string;
  confirmMessage: string;
  confirmLabel: string;
  pendingLabel: string;
  destructive: boolean;
}

export function messageActionCopy(intent: MessageActionIntent): MessageActionCopy {
  if (intent === "archive") {
    return {
      label: "Archive",
      confirmTitle: "Archive from chat?",
      confirmMessage:
        "This removes the turn from your chat. You can find it again under Work → Archived.",
      confirmLabel: "Archive",
      pendingLabel: "Archiving…",
      destructive: false,
    };
  }
  return {
    label: "Delete",
    confirmTitle: "Delete message?",
    confirmMessage: "Delete this message from the conversation? This cannot be undone.",
    confirmLabel: "Delete",
    pendingLabel: "Deleting…",
    destructive: true,
  };
}
