const DESKTOP_NOTIFICATIONS_KEY = "elsewhere:desktop-native-notifications";

export function desktopNotificationsEnabled(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    const stored = window.localStorage.getItem(DESKTOP_NOTIFICATIONS_KEY);
    if (stored === "0") {
      return false;
    }
  } catch {
    return true;
  }
  return true;
}

export function setDesktopNotificationsEnabled(enabled: boolean): void {
  try {
    window.localStorage.setItem(DESKTOP_NOTIFICATIONS_KEY, enabled ? "1" : "0");
  } catch {
    /* ignore */
  }
}
