const SLACK_OAUTH_RETURN_KEY = "elsewhere:slack-oauth-return";

export function rememberSlackOAuthReturn(path: string) {
  sessionStorage.setItem(SLACK_OAUTH_RETURN_KEY, path);
}

export function consumeSlackOAuthReturn(fallback: string): string {
  const stored = sessionStorage.getItem(SLACK_OAUTH_RETURN_KEY);
  sessionStorage.removeItem(SLACK_OAUTH_RETURN_KEY);
  return stored && stored.startsWith("/app/") ? stored : fallback;
}
