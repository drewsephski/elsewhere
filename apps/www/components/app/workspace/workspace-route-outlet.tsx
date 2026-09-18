/**
 * Empty route outlet for workspace chat URLs (`/app`, `/app/bots/:id`, `/app/groups/:id`).
 *
 * Conversation UI is owned by `WorkspaceShell` inside `WorkspaceAuthenticatedFrame`
 * (sidebar + `BotConversationView` / `GroupConversationView`), which reads the URL.
 * Next.js app routes and Tauri cloud-shell import this component so
 * route elements match web while the shell mounts the real chat UI.
 */
export function WorkspaceRouteOutlet() {
  return null;
}
