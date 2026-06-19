# Issue Tracker

## How to use
- Add new issues under **Open**, give each a short ID (e.g. `#1`).
- Move to **In Progress** when you start work.
- Move to **Done** when resolved (keep a short log for history).
- Use labels in brackets: `[bug]`, `[feature]`, `[chore]`, `[docs]`.

### Auto-fixer `Status:` values (set by the unattended runs on `## In Progress` items)
- `in-progress` — claimed/being worked. If a run ends here, it was interrupted (e.g. usage window) and the next run will **resume** it (up to 3 attempts).
- `in-review` — a `fix/issue-N` branch is ready; test + merge it, then move the item to `## Done`. Any manual UI test steps are in that branch's commit message.
- `blocked - <reason>` — the agent gave up; needs a human. Not auto-retried.
- `needs-human` — hit the attempt cap. Not auto-retried.

---

## Open

- [ ] #2 [bug] Gifts Ungiven allows a player to choose 2 cards with the same name
  - Details: Card says choose 4 cards with different names, but in current implementation cards with the same name can be chosen
  - Priority: Medium

- [ ] #3 [feature] Using Cockatrice UI elements when possible
  - Details: For Brainstorm, when choosing cards to put on top, a popup shows up with a list of cards in hand. This can be improved visually by just having the user click cards in hand and numbering them like when putting cards on top after mulliganing. Also, it has the first chosen go to the top, which is the opposite of what should happen intuitively. Similarly, Gifts Ungiven has the player searching for cards in library choose from a similar text list, when Cockatrice already has search library functionality. The player choosing cards to go to the graveyard could also get a custom zone view that shows the card images instead of just text (like what we did with the stack). I want this train of thought to be used for other custom resolution + custom zone cards in the future as well.
  - Priority: Low

- [ ] #4 [bug] Tap/Untap animations not working
  - Details: Lands used to have a quick animation when tapping, but this stopped working after the engine-owned mana update and lands tap instantly. Animations also never worked for untapping.
  - Priority: Low

- [ ] #5 [bug] Closing a custom resolution prompt softlocks the game
  - Details: Prompts for cards like Brainstorm and Gifts Ungiven should have the X button disabled. Note that this might not be an issue depending how the custom zone refactor works.
  - Priority: Medium

- [ ] #6 [bug] Copying a spell with Twincast causes two copies in the engine, but the copy doesn't show up on the stack visually
  - Details: After resolving Twincast, only a single copy of the spell remains on the stack. There should be two of the same spell on the stack after resolution, and the copy should have an annotation that says "Copy".
  - Priority: Medium

- [ ] #7 [bug] Copying a spell that targets with Twincast uses same targets instead of having Twincast's controller choose new ones
  - Details: After casting Twincast on a Lightning Bolt, both bolts will damage the same target. There should be a targeting prompt for the Twincast player to choose their own targets for the copy
  - Priority: Medium

---

## In Progress

- [ ] #1 [feature] Restore mana behavior when tapping lands while casting a spell
  - Details: Before the engine-owned mana pool commit, lands tapped after clicking a spell would automatically go to the spell instead of adding to the mana pool. After the engine-owned mana update, this no longer happens. Note that a related issue was fixed in a previous commit where mana would both go toward the spell and to the mana pool. These two methods of paying mana are mutually exclusive, going to the mana pool if activating mana abilities without clicking a spell/ability first, and going directly toward the spell/ability if the spell/ability is first clicked then the user starts activating mana abilities with the cost remaining prompt up.
  - Priority: High
  - Status: in-review
  - Attempts: 1
  - Branch: fix/issue-1
  - Session: be6b0445-1dd4-4fff-bfe0-87a56d8f72dc
  - Started: 2026-06-19
  - Notes: Tapping a mana land while a ruled spell/ability is mid-payment now routes the produced mana straight into the pending cost (reactively, via the ManaPoolUpdated SetCounter increase) instead of leaving it floating; pre-floated mana keeps the manual pool-counter-click path. Client-only, ruled mode. Needs manual UI verification.

---

## Done

_(none yet)_

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul