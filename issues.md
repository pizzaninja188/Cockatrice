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

- [ ] #4 [feature] Using Cockatrice UI elements when possible
  - Details: For Brainstorm, when choosing cards to put on top, a popup shows up with a list of cards in hand. This can be improved visually by just having the user click cards in hand and numbering them like when putting cards on top after mulliganing. Also, it has the first chosen go to the top, which is the opposite of what should happen intuitively. Similarly, Gifts Ungiven has the player searching for cards in library choose from a similar text list, when Cockatrice already has search library functionality. The player choosing cards to go to the graveyard could also get a custom zone view that shows the card images instead of just text (like what we did with the stack). I want this train of thought to be used for other custom resolution + custom zone cards in the future as well.
  - Priority: Low

- [ ] #5 [bug] Tap/Untap animations not working
  - Details: Lands used to have a quick animation when tapping, but this stopped working after the engine-owned mana update and lands tap instantly. Animations also never worked for untapping.
  - Priority: Low

- [ ] #6 [bug] Closing a custom resolution prompt softlocks the game
  - Details: Prompts for cards like Brainstorm and Gifts Ungiven should have the X button disabled. Note that this might not be an issue depending how the custom zone refactor works.
  - Priority: Medium

- [ ] #7 [bug] Copying a spell with Twincast causes two copies in the engine, but the copy doesn't show up on the stack visually
  - Details: After resolving Twincast, only a single copy of the spell remains on the stack. There should be two of the same spell on the stack after resolution, and the copy should have an annotation that says "Copy".
  - Priority: Medium

- [ ] #8 [bug] Copying a spell that targets with Twincast uses same targets instead of having Twincast's controller choose new ones
  - Details: After casting Twincast on a Lightning Bolt, both bolts will damage the same target. There should be a targeting prompt for the Twincast player to choose their own targets for the copy
  - Priority: Medium

- [ ] #9 [feature] Dual lands
  - Details: Right now, basic lands are the only implemented lands. I want to implement the 10 original dual lands. These lands should have the context menu open on left or right click to choose which mana ability to activate.
  - Priority: Medium

- [ ] #10 [feature] Auras
  - Details: Implement basic auras. Choose some effects that already exist from the engine and implement auras that use those effects. Auras should stack underneath the permanent they are enchanting in the client, and should die when the permanent is removed.
  - Priority: Medium

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
