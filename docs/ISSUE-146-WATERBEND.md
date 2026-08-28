# Waterbend payment contract and acceptance

Bounded implementation of [#146](https://github.com/pizzaninja188/Cockatrice/issues/146): Foggy Swamp Vinebender and Waterbending Lesson. Watery Grasp remains deferred to the attachment-movement work in [#155](https://github.com/pizzaninja188/Cockatrice/issues/155).

## Rules interaction checklist

1. **Rules and authority.** `AbilityCost::Waterbend` and `ResolutionCost::Waterbend` serve Vinebender and Lesson. Current Oracle text and rulings were checked on August 28, 2026. Under CR 701.67, each untapped artifact or creature controlled by the payer can pay one generic unit. This does not produce mana, pay colored or explicit colorless requirements, or require haste. The contribution cap comes from the Waterbend component before total-cost modifiers. A successful payment emits one internal `Waterbent` event, including all-mana and zero-cost payment. No observer card is added. Unbound X is rejected by card validation.
2. **State and identity.** Rust alone computes candidates, remaining cost, and payment completion. Source and selected objects use `ObjectId` plus zone-change generation, and submissions carry the state revision. The existing atomic cost transaction handles taps and exact ordinary/restricted mana. Definition IDs, Oracle names, hand slots, and physical `Server_Card.id` are not interchangeable. Preview queries are read-only and absent from the deterministic command log; the final activation or resolution answer is logged.
3. **Timing and interactions.** Vinebender requires its controller's turn and ordinary priority, including combat and a nonempty stack; it is not restricted to sorcery timing. Lesson draws three before its mandatory Waterbend-or-discard branch. A payment-time mana ability refreshes the same parked choice. Declining returns to the branch without drawing again; it cannot skip the discard obligation. Existing tap events, triggers, APNAP ordering, state-based actions, counters, and continuation machinery remain authoritative. Derived types and controller determine Waterbend eligibility. Double tapping one permanent is forbidden. New attachment movement, combat assignment, delayed effects, and layer machinery are N/A.
4. **Players and errors.** Payment logic indexes the authenticated player and current controller without opponent arithmetic. The existing session constructor still supports two seats; this change does not expand that product boundary. Duplicate, stale, wrong-controller, unavailable, and excessive selections fail before any debit. Preview reconciliation retains valid contributions and explains removals. Payloads attached to unrelated mana abilities or resolution choices fail closed.
5. **Visibility.** Staged selections and previews are per-player. The relay sends a standalone reply only to the requester, without changing gameplay revision, legal actions, physical cards, or replay. Printed costs and committed taps are public. Lesson's discard candidates use the existing private hand-choice path. The observer sees a wait rather than the payer's selected objects.
6. **Propagation.** Convoke's controller and protocol types are generalized to `RuledPayment`, `RuledPaymentUi`, and `PreviewPayment`; existing Convoke field numbers and colored contributions are preserved. Activation and resolution proposals share that controller and transaction. Waterbend's published object option is generic-only. Qt reuses the existing prompt, highlights, mana counters, cancellation, and mana-ability suspension. The final intentional contribution submits once automatically. Stale/duplicate replies cannot submit, and rejection restores staging even if a prompt refresh arrived before the acknowledgement. All hooks remain ruled-only; freeform is unchanged. New public-zone ordering behavior is N/A because the existing physical binding applies the committed taps.
7. **Verification.** Focused red/green tests cover the missing ability, mixed payment, client contribution kind, ignored payload rejection, stale readiness, and rejection/refresh races. Engine tests cover every mana/object split for both cards, duplicate/generation rejection, cap/modifier interaction, source double taps, completion events, own-turn timing, decline/recovery, and serialized replay. Client tests retain Convoke coverage. The real Servatrice E2E exercises both Waterbend cards, requester-only previews, exact physical taps on both seats, and a mana ability during Lesson payment. Full gates follow `AGENT-VERIFICATION.md`; the card checklist must be regenerated and checked.

## Two-client desktop verification (2026-08-28)

**Performed through computer use on two real Windows Qt clients.** These are agent-operated desktop checks, separate from CTest and from human acceptance. Ruled sessions used `-Dev -Trace -Seed 146`; a separate `-Freeform` session checked mode isolation.

| Check | Observed result |
|---|---|
| Vinebender staging, cancellation, privacy | Selecting Goldvein Pick did not tap it or spend the five floating mana. The other client saw no payment selection. Cancel cleared staging without committing anything. |
| Mixed and all-mana activation | Two objects plus three mana submitted once, tapped precisely those objects on both seats, and resolved to one counter. A separate all-mana payment left the creature untapped and spent exactly five mana. |
| All-object activation and eligibility | With no floating mana, Vinebender, Ornithopter, Grizzly Bears, Merfolk of the Pearl Trident, and Silvercoat Lion each contributed once. All five tapped on both seats. Newly controlled creatures were eligible; a tapped creature and an opposing creature could not contribute. An untapped plain Forest was not highlighted and remained untapped. The all-object check ran in second main phase because automatic yields skipped combat; combat timing remains covered by engine tests. |
| Off-turn restriction | The other seat's Vinebender had five mana available and priority over Unexpected Assistance, but its Waterbend menu action was disabled during the first player's turn. |
| Nested mana ability and recovery | Initially, right-clicking Llanowar Elves during an activation payment incorrectly opened the generic card menu. Fixed the shared menu path, added a red/green regression, rebuilt, and repeated successfully. The mana ability tapped Elves, removed its now-stale contribution, restored the payment prompt and Cancel, and generated one green mana. Cancel preserved that committed tap and mana without activating Vinebender. |
| Lesson payment and privacy | Lesson drew three before its branch. Opening Waterbend did not spend existing mana. A staged Goldvein Pick stayed private; Island's mana ability finished payment once. Both clients showed the exact Pick/Island taps, with no discard or second draw. |
| Lesson decline and discard | A second Lesson drew three (hand 13, library 27). After staging Vinebender, Decline returned to the mandatory branch without another draw, tap, or mana debit. Selecting a Mountain for discard remained private until confirmation; both clients then showed hand 12, library 27, and a graveyard containing that Mountain and both Lessons. |
| Convoke regression | Unexpected Assistance retained both blue and generic contribution options. Forest's context-menu mana ability preserved the staged blue creature; Cancel cleared staging but retained the committed Forest tap and green mana. A fresh payment consumed one blue creature contribution plus the selected mana exactly once. Opponent staging stayed private, followed by a private Hill Giant discard and the same committed result on both seats. |
| Freeform payment isolation | Both cards moved from hand to the table using ordinary freeform controls. Vinebender's menu contained normal tap/untap, movement, annotation, and counter actions, with no Waterbend action or ruled payment prompt. Tapping it synchronized on both clients. |

**Limits and separate findings:** A `move gy Foggy Swamp Vinebender` dev command during Lesson's parked payment was rejected by the engine, so a live zone-change stale-selection test was not completed. Rejection left the existing selection and prompt usable; nested-mana invalidation was exercised above, and generation/zone rejection remains covered by automated tests. Freeform's normal **Play** action sent a creature to a hidden stack: `PlayerGraphicsItem::rearrangeZones` hides the stack graphics while `TabGame::ensureStackWindow` excludes freeform. Both conditions are already present in `HEAD` and untouched by this payment work. Direct table movement/tapping passed; a clean general freeform stack acceptance is not claimed. No human acceptance was performed.

The nested-mana fix filters the fork-owned menu using engine-published mana metadata, retaining ability indices, color-option indices, and enabled states. The existing shared payment suspension owns the transaction. It supports both Convoke and Waterbend; no new protocol, relay, rules, identity, or hidden-information contract was introduced by this correction.

Session logs are retained locally under `build/verification-logs/waterbend-desktop-20260828` and `build/verification-logs/waterbend-freeform-desktop-20260828`. Screenshots and interaction observations are in the task's computer-use record.

### Reproduction steps

Launch `./scripts/launch-ruled-game.ps1 -Dev -Seed 146`, complete opening choices, and stop on the first player's main phase. Use that seat's dev console:

```text
put bf Foggy Swamp Vinebender
put bf Goldvein Pick
put bf Ornithopter
mana 5
```

1. Open Vinebender's ability, select Goldvein Pick, then cancel. Its highlight should disappear immediately; neither client should see a committed tap or spent mana. Repeat with two objects and three mana. The final contribution should submit once, tap precisely those objects on both seats, and put one ability on the stack. After passing priority, Vinebender gains one +1/+1 counter.
2. Repeat using all mana, then five eligible objects with no mana. Newly controlled creatures and artifact creatures are eligible once each; tapped permanents, plain lands, and the opponent's objects are not. On the opponent's turn, Vinebender's ability is unavailable.
3. Begin another payment with a ready Llanowar Elves (`put bf Llanowar Elves ready`). Use its context-menu mana ability during payment. The original prompt and Cancel control must return, with the new mana available and any stale object selection removed. Cancel clears staging while preserving committed mana-ability state.
4. Conjure `put hand Waterbending Lesson`, `put bf Island ready`, and fresh artifacts/creatures; add `mana UUUU`. Cast Lesson. It draws three before offering the branch. Choose Waterbend: existing mana is not silently spent. Select an object and use Island's mana ability to finish. It should resolve once without discarding or drawing again. Only the payer sees payment staging; both seats see committed taps.
5. Repeat Lesson, choose Waterbend, stage a contribution, and decline. The mandatory branch returns with the same hand. Choose Discard, select a card privately, and confirm that exact physical card enters the graveyard on both clients. The engine rejects dev zone moves during this parked choice; use the automated generation/zone regressions for that failure case, and the nested-mana flow above for desktop stale-contributor recovery.
6. Recheck Convoke with Unexpected Assistance: colored/generic choices, payment-time mana abilities, cancel, and private discard should behave as before. A freeform game should have no ruled payment prompt or Waterbend selection handling.

Stop the test clients before rebuilding with `./scripts/launch-ruled-game.ps1 -Stop`.

## Automated verification (2026-08-28)

All required automated commands exited 0. Logs are retained under `build/verification-logs`:

- Full `cargo test --quiet`: 154 card unit tests, 199 core unit tests, all-card conformance, and 1,194 scenarios, plus the remaining workspace tests. Label: `Waterbend full Rust conformance corrected`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`. Labels: `Waterbend clippy`, `Waterbend Rust formatting check`.
- Full `scripts/build-ninja.ps1`. Label: `Waterbend final client completion build`.
- Full CTest with `RULED_E2E_REQUIRE=1`: 18/18 targets passed; the real relay target executed without skipping. Label: `Waterbend final CTest rerun`.
- Focused client/prompt tests, including zero-cost completion and rejection recovery. Label: `Waterbend zero cost and client green`.
- Card checklist regeneration and `--check`: 1,522 full and nine partial cards. Labels: `Waterbend regenerate card checklist`, `Waterbend card checklist check`.
- Shared payment C++ files pass `clang-format --dry-run --Werror`; `git diff --check` passes.

After the desktop-discovered nested-mana correction:

- The new `RuledPendingCastTest.NestedPaymentMenuOffersOnlyEnginePublishedManaAbilities` regression failed red through the Windows CTest harness (expected three mana options, received five including unrelated actions), then passed after the fix. Labels: `Waterbend nested menu red through Windows harness`, `Waterbend nested menu client green`.
- The final full Windows Ninja build exited 0. Label: `Waterbend desktop fix final Windows build`.
- Full CTest with `RULED_E2E_REQUIRE=1` exited 0, 18/18 targets passed. Label: `Waterbend desktop fix full CTest`.
- Changed menu code and shared payment files passed the scoped C++ formatting checks; `git diff --check` exited 0. Rust/card data were unchanged by this correction, so the full Rust gates above remain the affected-side evidence.

The conformance driver now answers parked mana payments with `PayMana` before handling branch selection. This lets the corpus sweep finish Lesson's first branch rather than mistakenly treating its second prompt as another branch menu.

**MTG applicability:** Waterbend (CR 701.67), costs (CR 118), activation (CR 602), resolution-time mana abilities (CR 608.2g), and new-object identity (CR 400.7), checked against the [official Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt). No intended rules deviation for the two delivered cards; Watery Grasp and authored Waterbent observers remain outside this scope.
