# Approvals

Approvals is the human-in-the-loop queue at `/app/approvals`. A user reviews pending tool use from here.

## Sub-features

- `approvals-nav` opens `/app/approvals` from the `Approvals` nav item.
- `approvals-queue` shows an empty queue or at least one pending approval when a run is waiting.

## How to get to it (user POV)

- While signed in, choose `Approvals` in app navigation.
- Open `/app/approvals` directly.

## Driving it with the Cursor browser

Preconditions:

- Workspace recipe already passed for this session.
- Doctor reports `web: ok`.

- **Open the page.** Click the nav link `Approvals`. The URL is `/app/approvals`.
- **Proof.** Snapshot and screenshot the approvals page with `Elsewhere` visible in chrome. Save as `evidence/<run-id>/approvals/page.aria.yml` and `page.png`.
- **Pending item (optional).** Only when a run is actually waiting. Approve or deny through the UI, then confirm the queue no longer shows that item. Do not inject approvals through internal test helpers and call that a user proof.

## Gotchas

- An empty queue is a valid baseline. Do not fail the page recipe because no run is pending.
- Creating a pending approval means sending work that hits a gated tool. That is a work-run recipe, not this page's baseline.
