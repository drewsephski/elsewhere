export const NEW_CHAT_ACTIVE_WORK_HINT =
  "Finish or stop the current work before starting a new chat.";

export function newChatButtonState(options: {
  starting: boolean;
  hasActiveWork: boolean;
}): { disabled: boolean; label: string } {
  if (options.hasActiveWork) {
    return { disabled: true, label: NEW_CHAT_ACTIVE_WORK_HINT };
  }
  return { disabled: options.starting, label: "New chat" };
}
