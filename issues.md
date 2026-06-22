# Issue Tracker

This file is **your input** to the automated fixer. You own it — edit it (ideally
on your Windows machine) and push. The automation reads it but never writes it;
it records progress in `AUTOMATION_STATUS.md` instead.

## How to use
- Add issues under **Open**, each with a unique short ID (`#1`, `#2`, …). Don't
  reuse IDs.
- Give each a `Priority:` (High / Medium / Low) — the automation works High first.
- Use labels in brackets: `[bug]`, `[feature]`, `[chore]`, `[docs]`.
- When a `fix/issue-N` branch is merged, remove that issue from here (its status is
  tracked in `AUTOMATION_STATUS.md` until then).
- Workflow: you add issues here → the box (cron) fixes them on `fix/issue-N`
  branches and pushes them → you pull, UI-test, and merge to `master`. Status and
  per-branch manual UI test steps live in `AUTOMATION_STATUS.md` / the branch
  commit message.

---

## Open

- [ ] #5 [bug] Tap/Untap animations not working
  - Details: Lands used to have a quick animation when tapping, but this stopped working after the engine-owned mana update and lands tap instantly. Animations also never worked for untapping.
  - Priority: Low

- [ ] #9 [feature] Dual lands
  - Details: Right now, basic lands are the only implemented lands. I want to implement the 10 original dual lands. These lands should have the context menu open on left or right click to choose which mana ability to activate.
  - Priority: Medium

- [ ] #10 [feature] Auras
  - Details: Implement basic auras. Choose some effects that already exist from the engine and implement auras that use those effects. Auras should stack underneath the permanent they are enchanting in the client, and should die when the permanent is removed.
  - Priority: Medium

- [ ] #11 [feature] Gifts Ungiven UI improvement
  - Details: Brainstorm was recently updated to have the player click cards in hand to put them to top instead of the text popup. Similarly, I want Gifts Ungiven to also get a UI overhaul. When tutoring cards, the player should choose cards from the library search window, which already exists in non ruled games. For when the other player chooses cards to go to the graveyard, I want that player to get a custom zone popup, similar to what the stack popup looks like, where they choose the cards instead of the text list.
  - Priority: Medium

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
