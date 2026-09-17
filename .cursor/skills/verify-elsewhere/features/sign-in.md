# Sign in

Sign in is the email and password gate to the workspace. Logged-out visits to `/app` land here.

## Sub-features

- `sign-in-form` shows heading `Sign in`, textboxes `Email` and `Password`, and button `Sign in`.
- `sign-in-redirect` sends an unauthenticated `/app` visit to `/sign-in`.
- `sign-in-toggle` offers `Need an account? Sign up` to `/sign-up`.
- `sign-in-home` offers `Back to home` to `/`.
- `sign-in-submit` authenticates with email and password and then shows `/app`.

## How to get to it (user POV)

- Open `/sign-in`.
- Open `/app` while signed out.
- Choose `Open workspace` or `Open workspace now` while signed out.
- Choose `Already have an account? Sign in` from `/sign-up`.

## Driving it with the Cursor browser

Preconditions:

- Doctor reports `web: ok`.
- For `sign-in-submit` only: a disposable account that already exists. Do not use the operator's production password in chat logs.

- **Open the form.** Navigate to `http://127.0.0.1:3000/sign-in`. Heading `Sign in` is visible. Fill is possible in textboxes `Email` and `Password`.
- **Unauthenticated workspace.** Navigate to `http://127.0.0.1:3000/app`. The URL is `/sign-in` and the heading is `Sign in`.
- **Toggle sign up.** Click `Need an account? Sign up`. The URL is `/sign-up` and the heading is `Create account`.
- **Return home.** From `/sign-in`, click `Back to home`. The URL is `/` and the hero heading `Run your agents` is visible.
- **Submit (session recipes only).** Fill `Email` and `Password`, click button `Sign in`. The URL becomes `/app` and the workspace chrome is visible. A failed login shows `role="alert"` text, not `/app`.
- **Proof.** Snapshot and screenshot `/sign-in` with the heading and email field visible. Save as `evidence/<run-id>/sign-in/form.aria.yml` and `form.png`. For the redirect, also save `redirect.png` showing `/sign-in` after visiting `/app`.

## Gotchas

- The submit button label is `Sign in` on this page and `Sign up` on `/sign-up`. Do not click the wrong one.
- Google is only present when `NEXT_PUBLIC_GOOGLE_AUTH_ENABLED=1`. Absence is not a failure.
- Alpha invitation appears on sign-up only when `NEXT_PUBLIC_ELSEWHERE_ALPHA_INVITE_REQUIRED=1`.
- A 200 on `/sign-in` is not a session. Workspace recipes must see `/app` after submit.
