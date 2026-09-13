/** Returns true when an async chat load should commit results to React state. */
export function shouldCommitChatLoad(
  loadEpoch: number,
  currentEpoch: number,
): boolean {
  return loadEpoch === currentEpoch;
}
