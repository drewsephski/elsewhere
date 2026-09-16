# LandingHeader Specification

## Overview
- **Target file:** `apps/www/components/marketing/landing-header.tsx`
- **Screenshot:** rakazo desktop header — 86px tall, in-flow
- **Interaction model:** click-driven (nav, CTA, mobile menu)

## DOM Structure
header.site-header > left (logo + nav) | right (source pill + primary CTA + mobile toggle)

## Computed Styles (Rakazo, adapted tokens)
- height: 86px, padding: 22px 0, display: flex, justify: space-between, gap: 24px
- Content width: 1150px
- Wordmark: ~20px, tracking-tight, color `#2b2735`, lowercase/product name
- Nav links: 15px, color `#3d3d3d` → Elsewhere `#2b2735`, hover opacity 0.7
- Source pill: 14px, rounded-full, 1px border `#e4e0ec`, white bg
- Primary CTA: height 42px, radius 14px, violet gradient, white text, 14px/500

## States & Behaviors
- Mobile < md: hide nav + CTAs; hamburger opens overlay with large links
- Get started / Open workspace from CTA

## Text Content
Product · Bots · Computers · Source · Open workspace

## Responsive
- Desktop 1440: full nav
- Mobile 390: hamburger overlay
