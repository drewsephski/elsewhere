# Workspace

The signed-in `/app` shell is the main place to talk to bots, see overview status, and move into management pages.

## Sub-features

- `workspace-gate` keeps signed-out users on `/sign-in`.
- `workspace-shell` shows the product name `Elsewhere` after a valid session.
- `workspace-runner` shows live runner status. When the BFF cannot reach cloud-host, overview copy includes `Background work is temporarily unavailable`.

## How to get to it (user POV)

- Sign in, then land on `/app`.
- Choose `Open workspace` or `Open workspace now` while signed in.
- Choose `Back to chat` from a management page.

## Driving it with the Cursor browser

Preconditions:

- Doctor reports `web: ok`.
- A disposable signed-in session.
- For runner-ready copy, Doctor should also report `runner: ok`. If runner is down, assert the unavailable copy instead of the live dashboard.

- **Open workspace.** Navigate to `http://127.0.0.1:3000/app`. The URL stays `/app` (no bounce to `/sign-in`). The product name `Elsewhere` is visible.
- **Proof.** Snapshot and screenshot `/app` with the signed-in chrome. Save as `evidence/<run-id>/workspace/app.aria.yml` and `app.png`.

## Gotchas

- `/app` without a session is the sign-in recipe, not a workspace pass.
- Do not create bots or send work as part of this feature. Those are later map entries.
- ChatGPT device pairing is a separate connector flow. Skip it unless the bug is about that pairing.
