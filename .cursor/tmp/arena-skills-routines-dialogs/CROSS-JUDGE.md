# Arena cross-judge (different-family)

Judge: Composer (cross-judge pool). Parent arena runner: Grok. Read-only review of candidate sketches; no application source touched.

Scoring: integers 1–5 per criterion (higher is better). Tie-break: smaller public API / cleaner boundary.

---

## 1. Score table

| Criterion | candidate-1 | candidate-2 | candidate-3 |
|-----------|:-----------:|:-----------:|:-----------:|
| 1. Rail product fit | 5 | 4 | 4 |
| 2. Dialog quality | 5 | 4 | 5 |
| 3. Trigger reach | 5 | 4 | 3 |
| 4. Overlay exclusivity encoded | 5 | 3 | 3 |
| 5. Interface depth / reader load | 5 | 3 | 3 |
| 6. Diff restraint | 4 | 3 | 3 |
| **Total** | **29** | **21** | **22** |

### Criterion notes (by candidate)

**candidate-1**

1. **Rail product fit (5):** Rail body is Computer (static heading, live panel, Files nested under `RailSection`) plus Memory only. No Routines/Skills collapsibles, inline list bodies, or catalog empty copy in the rail.
2. **Dialog quality (5):** Single `BotLibraryDialog` matches CreateBotDialog density (`p-4`, `sm:max-w-lg`, title/description scale); reuses existing sidebars with `variant="minimal"`; conditional mount is the poll gate.
3. **Trigger reach (5):** Routines/Skills live only in the conversation header on every viewport (rail open, collapsed, mobile) — one location, no rail dock duplicate.
4. **Overlay exclusivity (5):** `TransientChromeOverlay` discriminated union (`none` | `settings` | `library` | `bot-details`) makes illegal stacks unrepresentable; library carries `botId`; `reduceChrome` is the single writer.
5. **Interface depth (5):** Callers dispatch `ChromeEvent` / read `openLibrary` for pressed state; no overlay context, no trigger wrapper module, no sidebar poll props; `LibraryOverlay` projection keeps the dialog ignorant of settings/bot-details variants.
6. **Diff restraint (4):** Pays for exclusivity with consolidating settings + mobile sheet into the chrome reducer (larger than “library only” but bounded and purposeful). Does not touch `/app/*` managers or enlarge Settings as primary UX. No `pollWhileOpen`.

**candidate-2**

1. **Rail product fit (4):** Computer + nested files and Memory are correct, but `BotOverlayTriggers` is a third rail affordance between them (not a catalog collapsible, still not “only two content areas”).
2. **Dialog quality (4):** Dialog host and copy are fine; behavior preserved via sidebars. Loses a point for `pollWhileOpen` when mount already gates polling (redundant policy surface).
3. **Trigger reach (4):** Rail row plus header icons with `railCollapsed` / `lg:hidden` split reaches open/collapsed/mobile, but duplicates triggers instead of one canonical row.
4. **Overlay exclusivity (3):** `WorkspaceBotOverlay` only models routines/skills; `settingsOpen` stays a separate boolean — catalog+settings exclusivity is imperative (`close` / `onConflict`), not structurally one slot.
5. **Interface depth (3):** `WorkspaceBotOverlayProvider` + `useWorkspaceBotOverlay` + `BotOverlayTriggers` + `BotListDialogHost`; sidebar API grows `pollWhileOpen`. More modules and props than the ask requires.
6. **Diff restraint (3):** Context layer, trigger component, dialog host, and sidebar signature changes for polling; settings state left parallel to overlay union.

**candidate-3**

1. **Rail product fit (4):** `ComputerRailSection` + Memory matches Computer/files nesting, but `BotContextActivityDock` in the rail is extra chrome (buttons, not lists — same deduction as candidate-2).
2. **Dialog quality (5):** Host dialog sizing and mount-gated lists align with CreateBotDialog and grounding; no extra poll props.
3. **Trigger reach (3):** Header actions only when `railCollapsed` with `hidden lg:flex`, so they are absent on mobile viewports; mobile access depends on opening the bot-details sheet to hit the rail dock — extra step vs header-always.
4. **Overlay exclusivity (3):** `BotContextOverlayState` covers routines/skills only; settings remains separate with callback mutual exclusion — not a single chrome discriminant (same class of gap as candidate-2, plus open questions on sheet vs dialog stacking).
5. **Interface depth (3):** Provider, host, dock, `BotContextHeaderActions` (explicit thin wrapper), optional `ComputerRailSection` — wider import surface than dispatch + one dialog.
6. **Diff restraint (3):** New overlay package plus extracted computer section; shell policy split across provider callbacks and settings handlers rather than one reducer module.

---

## 2. Totals

| Candidate | Total |
|-----------|------:|
| candidate-1 | **29** |
| candidate-3 | 22 |
| candidate-2 | 21 |

---

## 3. Red-flag screening (design-red-flags.md)

| Candidate | Shallow module | Information leakage | Temporal decomposition | Pass-through method |
|-----------|----------------|---------------------|------------------------|---------------------|
| **candidate-1** | Low risk: `reduceChrome` concentrates overlay policy; dialog is thin presentation. | Low: wire types stay in `workspace-chrome.ts`; dialog sees `LibraryOverlay` only. | Low: no load/validate/save stages; mount/unmount owns poll lifetime. | Explicitly rejects `BotLibraryTriggers`; no one-caller wrapper. |
| **candidate-2** | Medium: context value is mostly reducer forwarding plus `canOpen`. | Medium: settings open/section live outside `WorkspaceBotOverlay`; exclusivity rules duplicated at shell + provider. | Medium: `pollWhileOpen` splits “when to poll” from mount lifecycle. | `BotOverlayTriggers` forwards to `openRoutines`/`openSkills` (justified by two placements, still extra layer). |
| **candidate-3** | Medium: several small files (`dock`, `header-actions`, `computer-rail-section`) with modest hidden policy each. | Medium: settings vs overlay coordination via callbacks; host props repeat `botId`/`conversationId` already in provider scope. | Low for polling (mount-gated). | **High:** `BotContextHeaderActions` documented as thin wrapper over dock internals — classic pass-through. |

---

## 4. Recommended base

**Use candidate-1 as the implementation base.** It is the only proposal that keeps the rail strictly Computer (+ nested Files) and Memory, puts catalog access in a single always-visible header trigger row, and encodes all transient shell overlays (settings, library, bot-details) in one discriminated union with a pure reducer — so mutual exclusion is a type invariant rather than paired booleans and ad hoc `close()` calls. Public surface stays small: shell dispatches events, conversation gets toggle + pressed projection, one `BotLibraryDialog` at shell level; lists poll by existing mount semantics without new sidebar props or React context.

---

## 5. Graft from losers

| Loser | Worth grafting |
|-------|----------------|
| **candidate-2** | **Reducer test matrix wording** for scope rebind on conversation change (if product wants keep-open-same-bot behavior) — only if product answers that open question; candidate-1 already closes library on bot id change. |
| **candidate-3** | **Explicit toggle-on-second-click** in `open()` UX is already mirrored in candidate-1’s `toggle-library`; optionally borrow **dialog description copy** tone from candidate-3’s table if it reads closer to today’s empty-state intent (cosmetic, not structural). |

---

## 6. Reject from losers (and why)

| Loser | Reject | Why |
|-------|--------|-----|
| **candidate-2** | `WorkspaceBotOverlayProvider` + `useWorkspaceBotOverlay` as the primary integration path | Context that mostly holds a reducer increases reader load without hiding settings/sheet policy; candidate-1’s shell-local `reduceChrome` achieves the same with fewer concepts. |
| **candidate-2** | `pollWhileOpen` on sidebars | Unmount already stops intervals; extra prop is unused API per rubric and splits polling policy across host and list. |
| **candidate-2** | Rail + header duplicate `BotOverlayTriggers` | Violates preference for one trigger location when header-only already satisfies collapsed and mobile. |
| **candidate-2** | Partial overlay union (lists only) while settings stays boolean | Fails criterion 4: catalog+settings stack remains representable in state until runtime closes. |
| **candidate-3** | `BotContextActivityDock` in the rail | Reintroduces catalog affordances into the rail body; product fit and trigger strategy are strictly weaker than header-only. |
| **candidate-3** | `BotContextHeaderActions` wrapper | Pass-through module with no policy; candidate-1 inlines header buttons once. |
| **candidate-3** | Mobile trigger plan (`hidden lg:flex` header, dock only in sheet) | Header triggers not on mobile when sheet is closed; worse trigger reach than candidate-1. |
| **candidate-3** | Settings/overlay exclusivity via callbacks only | Same structural gap as candidate-2; does not unify with bot-details sheet in one chrome type. |

---

## Method

All three candidate directories (`USAGE.md`, `SKETCH.md`, `RATIONALE.md`) and `GROUNDING.md` read end to end. Candidates screened against `design-red-flags.md`. Scores are per-criterion against the stated rubric, not holistic preference.
