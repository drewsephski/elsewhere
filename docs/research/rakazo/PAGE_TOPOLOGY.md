# Rakazo → Elsewhere landing topology

Source: https://rakazo.com (desktop 1440px). Adapted in content and palette to Elsewhere; interaction model preserved.

## Visual order (top → bottom)

1. **Site header** (in-flow, not overlay) — logo + wordmark left, section links, GitHub/source pill, primary CTA. Mobile: hamburger.
2. **Hero** — centered badge, large headline, subtitle, primary/secondary CTAs, “set up with your agent” copy control.
3. **Product demo** (`#demo`) — dark macOS-style workspace window. Left bot list, center thread, right computer + routines. Caption under the window.
4. **Features** (`#selfhost`) — eyebrow + H2 + 3 cards (computers, routines, approvals).
5. **Bot roster** (`#roster`) — eyebrow + H2 + 4×2 template cards with creature avatars.
6. **Compare** (`#open-source`) — two-column Desktop vs Cloud cards.
7. **Stats band** — 4 facts.
8. **Closing CTA** — “Meet your first bot” + buttons.
9. **Footer** — wordmark, product links.

Get started is a **modal overlay**, not a page section.

## Layout

- Page background: Elsewhere cream `#f3f1f8` (Rakazo was `#fdfdfd`).
- Content column: `max-width: 1150px`, centered, horizontal padding 24–32px.
- Demo window: rounded ~24px, dark Elsewhere workspace tokens (`.dark` subtree).
- No Lenis / scroll-snap. Native smooth scroll via existing `html { scroll-behavior: smooth }`.

## Interaction model by section

| Section | Model |
|---|---|
| Header | click (anchor + mobile menu + get started) |
| Hero | click (CTAs, copy command, get started dialog) |
| Demo | click-driven local mock (bot select, computer toggle, composer, search, routines) |
| Features / roster / compare / stats / footer | static + hover + links |
| Get started dialog | click-driven modal with two paths |

## Dependencies

- Header, hero, demo, CTA, and footer all open the same Get started dialog.
- Demo does not talk to the API; mock data only.
- Creature avatars come from `@/components/app/bot-creature-avatar` + `BOT_AVATAR_PRESETS`.
