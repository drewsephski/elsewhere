/** Returns true when an async chat load should commit results to React state. */
export function shouldCommitChatLoad(
  loadEpoch: number,
  currentEpoch: number,
  loadBotId: string,
  selectedBotId: string | null,
): boolean {
  return loadEpoch === currentEpoch && loadBotId === selectedBotId;
}
