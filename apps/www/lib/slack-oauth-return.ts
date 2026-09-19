import {
  readSessionStorage,
  removeSessionStorage,
  writeSessionStorage,
} from "./session-storage";

const SLACK_OAUTH_RETURN_KEY = "elsewhere:slack-oauth-return";

export function rememberSlackOAuthReturn(path: string) {
  writeSessionStorage(SLACK_OAUTH_RETURN_KEY, path);
}

export function consumeSlackOAuthReturn(fallback: string): string {
  const stored = readSessionStorage(SLACK_OAUTH_RETURN_KEY);
  removeSessionStorage(SLACK_OAUTH_RETURN_KEY);
  return stored && stored.startsWith("/app/") ? stored : fallback;
}
