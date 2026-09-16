# Behaviors — Rakazo source, Elsewhere adaptation

## Scroll sweep

- Header does **not** shrink or change background on scroll (in-flow).
- No scroll-driven tab switching, parallax, or snap.
- Smooth native CSS scroll for `#demo`, `#roster`, `#selfhost`, `#open-source`.

## Click sweep (source)

- **Get started** → centered modal: “How do you want to start?” with two large option cards. Self-host drills into docs; Cloud waitlist shows email form.
- **Bot list items** → switch selected bot: thread, header name, computer panel, routines all swap. No page navigation.
- **Toggle computer panel** (`aria-pressed`) → hide/show right rail; chat expands.
- **Take control** → source product takeover (Elsewhere: link to `/app`).
- **Routines** → highlight/select locally.
- **+ New bot / + New routine** → source creates locally; Elsewhere opens Get started (real bots live in workspace).
- **Composer send** → appends user bubble then bot reply (local mock).
- **Search** → filters bot list.
- **Set up with your agent** → copies a setup prompt to clipboard.

## Hover

- Primary buttons: slight brightness on gradient.
- Ghost buttons: border/background tighten.
- Nav links: opacity ~0.7.
- Feature/template cards: 1px border + faint lift (`translateY(-2px)`, shadow).
- Bot rows: background `#ffffff08`.

## Responsive

- **1440px:** 3-column demo; 3 feature cards; 4-column roster; 2-column compare.
- **768px:** demo drops computer rail (toggle still available); roster 2 columns; compare stacks.
- **390px:** header hamburger; hero type ~32px; demo is chat + bot picker; roster 1 column; CTAs full width.

## Elsewhere mapping

- Primary CTA: Open workspace (`/app`) or Get started dialog.
- Dialog options: **Cloud workspace** (live) and **Download for macOS** — cloud is not a waitlist.
- Demo uses Elsewhere dark workspace tokens and creature avatars, not Rakazo ghosts.
- No copied Rakazo 3D illustrations.
