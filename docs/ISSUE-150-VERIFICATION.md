# Issue #150: Earthbend verification

Implemented and checked on Windows on 2026-08-28. Scope: Rebellious Captives,
Dai Li Indoctrination (both modes), Badgermole, animated-land P/T and battlefield
placement, and the delayed return after death or exile. This report records the
implementation checks; publication and completion are recorded on issue #150.

## Rules interaction checklist

1. **Rules authority.** Oracle text and rulings for all three cards were checked
   against Scryfall during implementation. Earthbend uses a shared `Amount`,
   indefinite continuous effects, and an independent delayed trigger. These
   support all three cards. `NonlandPermanent` also supports Auntie's Sentence;
   the generalized event-bound return continues to support Abnormal Endurance
   and Unholy Indenture. Adding cards that trigger whenever a player earthbends
   is deferred; this change does not add that separate event-authoring vocabulary.
2. **State and identity.** Tricerules owns targeting, characteristics, counters,
   delayed observers, and zone changes. The watched land is an ObjectId plus its
   battlefield generation, separate from the creating source's identity and
   controller. Only the immediate graveyard/exile generation can return. Other
   departures expire the watch. Reentry clears the old animation and counters
   through the existing zone-change and entry pipelines. Source departure and
   subsequent control/ability changes do not cancel the delayed trigger. Logged
   commands replay deterministically. No new attachment or face-choice behavior.
3. **Timing and interactions.** Shared targeting validates an own-land target at
   selection and resolution. Layers 4, 6, and 7b preserve other types and mana
   abilities, add haste, and set base P/T to 0/0; shared counter placement follows.
   No SBA occurs between these steps and observer creation. Departure observation
   uses committed, replacement-adjusted zone events and event-time creature status.
   Triggers use the existing stack and ordering pipeline. Tests cover repeated
   animation, replaced death, zero counters, cleanup, token disappearance, and
   source/target departure. New copy or combat algorithms: N/A; existing derived
   characteristics, haste, and Trample are consumed by those systems.
4. **Players and failure paths.** Ownership, original ability controller, current
   land controller, affected opponent, and discard chooser remain distinct.
   Implementation uses shared player filters, not two-player arithmetic. Tests
   reject nonland/opponent targets, stale targets, repeated Exhaust activation,
   ineligible discard cards, and the wrong chooser. A stale return resolves
   without moving a later incarnation; a departed token cannot return.
5. **Visibility.** No new wire field or hidden-state exposure. Battlefield P/T,
   types, counters, and public pile order use existing views. Dai Li intentionally
   reveals the opponent's hand through the existing public-reveal picker; only
   its controller chooses. Physical pile refreshes use recipient-filtered game
   states. Clients do not derive legality from Oracle images.
6. **Propagation.** Rust publishes existing characteristics and zone events.
   Servatrice clears former-creature P/T and preserves exact public-zone physical
   bindings across move passes, arranging piles in the engine's order. Qt's
   existing badge, row layout, targeting, modal actions, payment, and reveal
   picker render this state. Protobuf changes: N/A, existing contract is sufficient.
   Production C++ changes remain in fork-owned ruled files; freeform paths are
   unchanged. Relay/client and real two-client tests cover the contract.
7. **Verification and delivery.** Red/green regressions, both full affected-side
   suites, lint, format, generated checklist, diff checks, and the GUI checks
   below passed. `CARDS.md` was regenerated. Publication uses one focused commit
   on master, with the verification summary attached to issue #150.

## Automated evidence

All completion commands exited **0**. Logs are local files under
`build/verification-logs/`.

| Gate | Evidence |
| --- | --- |
| Full Rust tests (`cargo test --quiet`) | `20260828-094852-316-Earthbend-full-Rust-tests.log` |
| Clippy (`cargo clippy --all-targets -- -D warnings`) | `20260828-095059-808-Earthbend-Rust-clippy.log` |
| Rust formatting (`cargo fmt --check`) | Exit 0 |
| Generated card checklist check | `20260828-095100-173-Earthbend-checklist-check.log` |
| Full Windows Ninja build | `20260828-100044-066-Earthbend-final-identity-Windows-build.log` |
| Full CTest, `RULED_E2E_REQUIRE=1` | `20260828-100110-311-Earthbend-final-identity-full-CTest.log`; 18/18 passed |
| Whitespace/diff check (`git diff --check`) | Exit 0 |

Focused regressions cover 13 Earthbend engine scenarios including replay,
nonland-permanent filtering, badge/row transitions, client derived-state
replacement, duplicate-name public pile bindings, and actual Lightning Bolt and
Swords to Plowshares casting through two network clients.

Representative intended red failures were missing Earthbend/card definitions,
stale P/T after deanimation, the spell source's wrong generation, and a physical
Forest/Bolt identity swap. The last two have retained logs:
`20260828-094557-212-Earthbend-source-identity-red.log` and
`20260828-095744-647-Earthbend-real-removal-identity-red.log`.

## Computer-use manual checks actually performed

These were agent-operated Windows GUI checks using the computer-use skill,
separate from automated E2E and from human acceptance. The launcher set up two
local clients; card choices, targeting, payments, priority passes, and inspection
were performed in the GUI. Ruled setup used `scripts/launch-ruled-game.ps1 -Dev
-Seed 150`; dev-console commands only supplied test cards and mana.

| Flow | Observed result |
| --- | --- |
| Rebellious Captives: supply Forest and Captives; activate Exhaust with six mana; try a nonland target, then choose Forest; pass both players | Nonland click did not accept a target. Forest displayed 2/2, counters and haste in the creature row on both clients; Captives displayed 4/4. |
| Badgermole: supply Forest, then put Badgermole onto the battlefield; choose Forest for its ETB; pass both | Both clients displayed a 2/2 land creature with haste and Trample in the creature row. Badgermole remained 4/4 without Trample. |
| Death: cast Lightning Bolt from hand, target animated Forest, pay red, and pass both players | Forest left the battlefield and its delayed return appeared on the stack. Passing both again returned the correct Forest tapped to the land row, with no P/T badge, counters, haste or Trample. Bolt stayed in the graveyard on both clients. |
| Dai Li Earthbend mode: right-click its hand card, choose Earthbend, target the tapped returned Forest, pay black plus generic, and pass both | Forest stayed tapped, gained its 2/2 badge and abilities, and moved to the creature row on both clients. |
| Exile: cast Swords to Plowshares on that Forest, pay white, and pass both | Forest appeared in exile, its controller gained two life, and the Dai Li delayed trigger appeared. Passing both again returned Forest tapped to the land row without a badge; exile became empty and Swords remained in the graveyard on both clients. |
| Dai Li discard mode: choose its other cast action, target the opponent, pay, and pass both; click an Island, then Merfolk of the Pearl Trident and Confirm | Both clients saw the revealed hand. Island did not enable Confirm; Merfolk did. Exactly that creature was discarded, the hand count decreased, and the reveal window closed on both clients. |
| Freeform smoke: launch with `-Freeform`, draw seven, play/tap Mountain, drag Hill Giant from hand to the battlefield, increase its P/T through the context menu | Land/creature placement, manual tapping and mana, dragging, and 3/3 to 4/4 P/T editing worked. The second client showed matching placement and 4/4. No ruled targeting/payment prompts appeared. |

The first death check exposed a real relay bug: Bolt and Forest arrived in the
physical graveyard in a different order from the engine vector, so positional
rebinding returned the wrong physical card. The implementation now records exact
post-move bindings and reconciles pile order without changing known identities.
The actual spell flow was added as a failing E2E regression, fixed, and repeated
successfully in the GUI. All test clients and servers were stopped afterward.

Not manually repeated: stale-return generation races, changed controllers,
multiple watchers, token disappearance, and replay. These have automated engine
coverage. Human acceptance remains separate; the table gives reproducible steps
for repeating the visible flows.

## MTG applicability

The governed concepts are Earthbend (CR 701.66a), indefinite continuous effects
(611.2a), layers (613), state-based actions (704), delayed triggers (603.7), and
new-object identity across zone changes (400.7). Implementation follows those
concepts for the three scoped cards, including Exhaust, modal discard selection,
and Badgermole's counter-filtered Trample. CR 701.66b's separate “whenever you
earthbend” card-authoring surface is deferred as noted above.

Rules source checked during implementation:
[official comprehensive rules, 2026-08-19](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt)
and [Avatar release notes](https://magic.wizards.com/en/news/feature/avatar-the-last-airbender-release-notes).
