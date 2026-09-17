# Computers

Computers is the management page for agent Linux environments. A user opens it from workspace navigation at `/app/computers`.

## Sub-features

- `computers-nav` opens `/app/computers` from the `Computers` nav item.
- `computers-empty-or-list` shows either an empty-state call to create a computer or an existing computer row.

## How to get to it (user POV)

- While signed in, choose `Computers` in app navigation.
- Open `/app/computers` directly.

## Driving it with the Cursor browser

Preconditions:

- Workspace recipe already passed for this session.
- Doctor reports `web: ok`. Runner-down still allows the page to render. Creating a computer needs `SPRITE_TOKEN` and a healthy `cloud-host`.

- **Open the page.** Click the nav link `Computers` (`aria-label="App navigation"`). The URL is `/app/computers`.
- **Proof.** Snapshot and screenshot the computers page with `Elsewhere` visible in chrome. Save as `evidence/<run-id>/computers/page.aria.yml` and `page.png`.
- **Create (optional).** Only when the bug is computer provisioning. Follow the page's create control, then reload and confirm the new name in the list. Do not provision against the shared production Fly token from an unattended run unless the operator asked.

## Gotchas

- Nav label is `Computers`, not "Machines" or "Sprites".
- A successful page render is not proof that a sprite booted. Boot proof needs a second view of the computer status after create.
- Do not drive Tauri `:1420` for this feature. The web app on `:3000` is the source of truth.
