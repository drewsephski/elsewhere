/** True when the UI runs inside the Tauri webview (desktop shell). */
export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  return (
    "__TAURI_INTERNALS__" in window ||
    "__TAURI__" in window
  );
}

/** Call once at startup so CSS can target the desktop shell (`html.tauri`). */
export function markTauriDocument(): void {
  if (!isTauriRuntime()) {
    return;
  }
  document.documentElement.classList.add("tauri");
  document.documentElement.dataset.platform = "desktop";

  const platform = (
    window as Window & { __TAURI_INTERNALS__?: { metadata?: { currentPlatform?: string } } }
  ).__TAURI_INTERNALS__?.metadata?.currentPlatform;
  if (platform) {
    document.documentElement.dataset.tauriPlatform = platform;
  }
}
