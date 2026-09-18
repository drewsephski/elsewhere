/**
 * Empty route outlet for workspace chat URLs (`/app`, `/app/bots/:id`, `/app/groups/:id`).
 *
 * Conversation UI is owned by `WorkspaceShell` (sidebar + `BotConversationView` /
 * `GroupConversationView`), which reads the pathname. Next.js pages and the Tauri
 * `cloud-shell` router mount this component only so the layout outlet is satisfied.
 */
export function WorkspaceRouteOutlet() {
  return null;
}
