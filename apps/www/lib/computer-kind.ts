export function isLocalMacProvider(provider: string): boolean {
  return provider === "local_mac";
}

export function computerProviderLabel(provider: string): string {
  return isLocalMacProvider(provider) ? "This Mac" : "Cloud";
}
