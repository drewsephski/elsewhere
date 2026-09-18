# Landing

The public home page explains Elsewhere and routes a visitor into the cloud workspace without requiring a session.

## Sub-features

- `landing-home` renders the hero `Run your agents` / `elsewhere.`
- `landing-open-workspace` follows header `Open workspace` to `/app` (unauthenticated visitors land on `/sign-in`).
- `landing-get-started` opens the dialog `How do you want to start?`
- `landing-open-workspace-now` follows dialog link `Open workspace now` to `/app`.

## How to get to it (user POV)

- Open `http://127.0.0.1:3000/`.
- Choose `Open workspace` in the desktop header.
- Choose `Get started` in the hero, then `Open workspace now`.
- Choose the home link named `Elsewhere home`.

## Driving it with the Cursor browser

Preconditions:

- Doctor reports `web: ok`.
- Browser is not already on a signed-in `/app` session. Use a clean profile or accept that `/app` will skip sign-in.

- **Open home.** Navigate to `http://127.0.0.1:3000/`. The heading contains `Run your agents` and `elsewhere.` The accessible home link is `Elsewhere home`.
- **Open workspace from header.** Click the link `Open workspace`. The URL becomes `/app` or `/sign-in`. If there is no session, the sign-in heading `Sign in` is visible.
- **Get started dialog.** Return to `/`. Click the button `Get started`. A dialog heading `How do you want to start?` appears with a link `Open workspace now`.
- **Proof.** Snapshot and screenshot the home hero and the get-started dialog. Save as `evidence/<run-id>/landing/home.aria.yml`, `home.png`, `dialog.aria.yml`, `dialog.png`.

## Gotchas

- Several controls are labeled `Get started`. Use the hero button on `/` unless the recipe names the header or mobile menu.
- `Open workspace` goes to `/app`, which redirects to `/sign-in` when logged out. That redirect is success for an unauthenticated run.
- `View on GitHub` and `Source` leave the app. Do not treat GitHub as Elsewhere proof.
