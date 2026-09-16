# ProductDemo Specification

## Overview
- **Target file:** `apps/www/components/marketing/product-demo.tsx`
- **Interaction model:** click-driven local mock (NOT scroll-driven)
- Wrap root of the window in `className="dark"` so workspace tokens apply.

## DOM Structure
section#demo
  .demo-window (rounded 24px, overflow hidden, ~660px tall)
    traffic lights
    grid: sidebar 260px | thread 1fr | computer 348px
  caption

## Sidebar
- Header: “Bots” + plus (opens get started) + search
- Rows: avatar, name 14.5px medium, time 13px muted, preview 13.5px 1-line
- Selected: bg white/8
- Footer: user initials chip + “You”

## Thread
- Topbar: avatar + bot name, computer toggle (aria-pressed)
- Messages: assistant dark bubbles `#262626`, user light `#f2f2f2` on the right
- Checklist rows with green checks
- Composer: rounded-full, + , placeholder “Message {bot}”, send arrow

## Computer rail
- Title “{bot}’s computer”
- Live mock browser: tab, URL bar, site-specific page (mail / calendar / sheet / github / figma / ads / expenses / accounts)
- Pointer sits on the active row the bot is working
- Take control in the browser chrome → `/app`
- Routines list with clock glyph + “+ New routine”

## Behaviors
- Click bot → swap thread, computer, routines
- Toggle computer → hide rail, thread expands
- Send → append user then delayed assistant reply
- Search filters list
- New bot / new routine → get started

## Responsive
- Window is a fixed height (`min(78dvh, 680px)`), never stacked to page height.
- sm+ (640px, tablet and desktop): left sidebar + thread. Computer is a right overlay until lg, then an in-flow third column.
- <sm: chat-first. Horizontal bot chips, header opens a full-window bot picker, Computer is a bottom sheet.
