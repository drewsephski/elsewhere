function browserSessionStorage(): Storage | null {
  try {
    const storage = globalThis.sessionStorage;
    if (!storage) {
      return null;
    }
    const probeKey = "__elsewhere_session_storage_probe__";
    storage.setItem(probeKey, "1");
    storage.removeItem(probeKey);
    return storage;
  } catch {
    return null;
  }
}

export function readSessionStorage(key: string): string | null {
  try {
    return browserSessionStorage()?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

export function writeSessionStorage(key: string, value: string): boolean {
  try {
    const storage = browserSessionStorage();
    if (!storage) {
      return false;
    }
    storage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

export function removeSessionStorage(key: string): void {
  try {
    browserSessionStorage()?.removeItem(key);
  } catch {
    return;
  }
}
