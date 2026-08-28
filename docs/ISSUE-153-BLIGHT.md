# Blight payments and acceptance

Bounded delivery for [issue #153](https://github.com/pizzaninja188/Cockatrice/issues/153): Cinder Strike, Wild Unraveling, Gristle Glutton, Dream Seizer, Blighted Blackthorn, Chaos Spewer, and supporting Tatterkite. At the user's delivery request, #153 is scoped for closure on the shared Blight implementation and these six calibration cards. The original eight-card cohort is not fully implemented: Spiral into Solitude remains explicitly tracked in [#155](https://github.com/pizzaninja188/Cockatrice/issues/155), and Burning Curiosity in [#159](https://github.com/pizzaninja188/Cockatrice/issues/159); both dependencies were verified open before publication.

## Rules interaction checklist

1. **Authority and rules.** Oracle text and rulings were checked for the seven cards. Rust performs the shared Blight operation: put all N counters on one currently controlled creature. This is not targeting. Optional payments require a creature that can receive counters; a mandatory instruction completes even when none exists. Cinder Strike and Wild Unraveling share cast options; Gristle Glutton and Dream Seizer share the operation across activated and resolving costs. Tatterkite and the counter-prohibition clause of Blossombind share source/attached-permanent counter restrictions; a partial Blossombind card is not added. Cinder Strike's conditional amount also supports kicker-style amounts such as Burst Lightning.
2. **State and identity.** `tricerules` is the only state writer. Candidate objects and committed receipts bind engine ObjectId plus zone-change generation, not a definition ID, card name, hand slot, or Server_Card.id. Receipt metadata survives parked resolution and spell copying; copying never pays again. Creature controller, not owner, governs eligibility. Committed choices use the existing logged commands; no new out-of-band gameplay mutation is introduced.
3. **Timing and interactions.** Mana and object payments are prevalidated before commitment. Counter placement precedes a selected sacrifice, including when the same creature pays tap, Blight, and sacrifice costs. Sacrifice snapshots include the counters. Lethal counters are legal; SBAs wait for the full cost or resolving effect list, including private discard choices. Counter annihilation and lethal decisions use the same pre-action state. Counter prohibition is shared by entry counters, ordinary placement, prevention rewards, and positive loyalty costs; removal is unaffected. Face-down state, copied abilities, attachment membership, and ability blanking feed the restriction. Existing APNAP discard, trigger ordering, and physical synchronization remain in control. No new combat-assignment or play-permission behavior is introduced.
4. **Players and failures.** Candidate generation uses current derived creature characteristics and controller. Tapping, summoning sickness, and targeting protections do not independently disqualify a creature from Blight. Wrong-seat, missing, excessive, stale-generation, prohibited, and unaffordable payments use existing rejection paths. Cancelling the mana stage of a required branch returns to that branch choice rather than skipping the instruction. A three-seat fixture checks controller/owner separation without introducing two-player arithmetic into the operation.
5. **Visibility.** Payment metadata and resolution candidates remain per-player. The shared CostObjects choice carries physical bindings only to its chooser; observers receive a wait. Hidden discard choices retain their existing redaction. Committed counters, taps, and zone moves use public state updates. Existing private payment-preview routing is unchanged; no preview is added to the gameplay log or broadcast.
6. **Propagation.** The protobuf adds Blight discriminants and a counter count, without a new command shape. Rust and C++ consume the same schema. Servatrice's existing private-choice routing and battlefield bindings handle the new operation. Qt parses the new kinds, displays the count, uses its existing battlefield picker/confirmation controls, and serializes exact generations. Shared prompts and wire-shape handling are extracted into ruled-owned helpers. Freeform paths are unchanged.
7. **Verification.** Red/green regressions cover prohibited placement, cast/ability payments, resolution ordering, mandatory Blight, required-branch cancellation, simultaneous SBAs, client dispatch, and amount-context validation. Additional coverage checks copying without repayment, counter-bearing sacrifice LKI, entry prohibition, stale rejection, Wild Unraveling's alternatives, and Blackthorn's ETB. The real-server E2E uses two protocol clients to check private choices, physical identity, tapped/sick eligibility, Tatterkite exclusion, and delayed lethal movement. These automated checks are distinct from desktop acceptance below. Full gates follow `AGENT-VERIFICATION.md`; generated card checklist and diff checks apply. Commit, push, and tracker changes require separate authorization.

## Two-client Qt acceptance

The complete checklist below remains recommended; the focused cancellation and payment checks performed with desktop automation on August 28 are recorded in the follow-up section. They are not a human acceptance pass. Start a local development game using `scripts/launch-ruled-game.ps1 -Dev -Seed 153`. Use its existing dev controls to arrange the named cards and mana.

- With a ready Gristle Glutton, a tapped/newly controlled Grizzly Bears, and Tatterkite, activate Glutton. The prompt must say “Blight 1”; Bears is selectable, Tatterkite is not. Cancel once: neither seat should see a committed tap or counter. Repeat and confirm: both seats see the source tap and Bears become 1/1. Only the controller chooses a discard; one card is drawn after an actual discard.
- Cast Cinder Strike, first skipping Blight, then paying it. Confirm 2 versus 4 damage. The Blight creature picker must not become a target-selection prompt or offer hand cards. Wild Unraveling must require exactly one of Blight 2 or the additional {1}.
- Resolve Dream Seizer with the 1/1 Bears available. Choose Blight, select Bears, and leave the opponent's discard prompt open. Both clients must still show the same Bears on the battlefield. Only the opponent sees discard candidates. Complete discard: that exact Bears moves to its owner's graveyard on both clients.
- Resolve both Blighted Blackthorn's entry and attack triggers. Paying Blight 2 draws one card and loses one life; declining does neither. A tapped attacker can be chosen to receive the counters.
- Resolve Chaos Spewer. Choosing Blight must require one eligible creature when available. Cancelling the {2} payment returns to the required choice. If the source has left before resolution and no eligible creature remains, choosing Blight completes without demanding mana or leaving a stuck prompt.
- Check a fresh freeform game: ordinary card movement and activation menus retain their existing behavior.

## Boundaries and MTG applicability

The completed-Blight event and exact selected-creature receipt are internal foundations. No “whenever you blight” observer card or “the blighted creature” follow-up consumer is authored here. Counter transfer/removal effects, Wither, the full Blossombind card, Spiral into Solitude, and Burning Curiosity remain outside this delivery.

**MTG applicability:** Blight, costs and optional payments, counter prohibition, simultaneous state-based actions, last-known characteristics, spell copying, and new-object identity. The implemented cards follow the checked [official Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt) and [Lorwyn Eclipsed release notes](https://magic.wizards.com/en/news/feature/lorwyn-eclipsed-release-notes), with the scope deferrals above.

## Delivery gates (2026-08-27)

All required commands exited 0:

- Full `cargo test --quiet`, including 149 card unit tests, 187 core unit tests, and 1,157 scenarios.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.
- Full `scripts/build-ninja.ps1` and full CTest: 18 of 18 targets passed with `RULED_E2E_REQUIRE=1`.
- The focused Blight E2E was also run verbosely: one real-server test executed and passed, without a skip.
- `scripts/gen-card-checklist.ps1` and `--check`: seven additional fully implemented cards, now 1,501 full plus 10 partial.
- New C++ helper header passes `clang-format --dry-run --Werror`; tracked edits pass `git diff --check`.

Complete logs are under `build/verification-logs`; these initial runs use labels `Blight final Rust rerun`, `Blight clippy green`, `Blight final rebuilt binaries`, `Blight final CTest rerun`, and `Blight card checklist check`. Desktop Qt acceptance was unperformed at this initial gate; see the subsequent focused checks below. No commit, push, or issue mutation was made.

## Follow-up: cancellation repaint and generic resolution payments

- Activated-ability cancellation now clears the staged objects before notifying prompt listeners and emits the existing card-repaint signal. Resolution cost selection teardown also repaints immediately, including cancellation and submission; neither requires a later battlefield update.
- Authored pure-generic resolution costs now share Ward's `PendingManaPayment::from_cost` conversion. Chaos Spewer's `{2}` and Mentor of the Meek's `{1}` publish their actual generic remainder and enter the existing staged pip picker. Choosing the branch does not spend already-floating mana; clicking pips or activating mana abilities completes the payment through the existing logged command.
- The automatic submit-at-zero hook predates Blight: `PlayerActions::resumePendingRuledPaymentAfterEngineCommand`, commit `0f25993f4` (resolution-time soft counters, August 13). Authored mana branches started publishing `generic_mana_cost: 0` with a structured cost in `ab6475563` (resolution-time modes and costs, August 18). Their combination caused the reported automatic payment. Ward already normalized pure-generic costs in `0e6d6c17d7` (August 23); this correction shares that implementation. Non-generic resolution costs retain their existing behavior and are outside this correction.
- Follow-up checklist: engine authority, existing logged payment commands, player-set-generic behavior, and generation-bound selection remain unchanged. New protobuf fields, relay changes, visibility changes, physical movement, and freeform behavior changes are N/A. No new MTG mechanic or timing rule is introduced; this restores the intended payment interaction and clears local display state.
- Red/green coverage: Chaos Spewer's published payment failed with zero instead of two; both selection cancellation and submission failed with zero repaint notifications. All three assertions pass after the correction. Existing Ward scenarios characterize the shared conversion.
- The desktop check also exposed a double subtraction in the paying client's mana display after a successful resolution payment. Submitted staging is now excluded from incoming authoritative pool snapshots while retaining its IDs for rejection recovery. Before the correction, the payer displayed zero and the opponent one; after rebuilding, both displayed one.

### Follow-up verification (2026-08-28)

All automated gates exited 0: full Rust tests (`Blight followup final Rust`), clippy (`Blight followup clippy green`), fmt (`Blight followup fmt`), full Ninja (`Blight payment display build`), focused client/prompt tests (`Blight payment display focused tests`), full CTest with `RULED_E2E_REQUIRE=1` (18/18; `Blight payment display full CTest`), and card checklist (`Blight followup card checklist`). Logs are in `build/verification-logs`.

Performed with two real Qt clients, local Servatrice, and the rebuilt sidecar using desktop automation:

1. Conjure ready Gristle Glutton and Grizzly Bears. Activate Glutton, select Bears, then Cancel. Bears' thick orange Blight outline disappears immediately, before another gameplay action; Bears stays 2/2 and Glutton stays untapped. The ordinary thin card-selection border is distinct from the cost highlight.
2. Float two red mana before resolving Chaos Spewer's ETB. Choose Pay {2}. The normal payment prompt remains open with two remaining and the pool untouched. Click one pip: both the displayed pool and remaining payment become one. Decline: the pool returns to two and the required Pay/Blight choice returns.
3. Choose Pay again, activate Mountain during payment, then click the final pool pip. Payment completes once, Chaos Spewer stays 5/4 without Blight counters, and Mountain is tapped. On the final rebuilt client both seats display the correct one remaining red mana. This check reproduced and then verified the display-accounting correction.

The remaining six-card desktop checklist and a human acceptance pass are still recommended. At this verification checkpoint, no commit, push, or tracker mutation had been made.

## Publication verification (2026-08-28)

After the user authorized committing on master, pushing, and completing the issue, all pre-commit gates were rerun and exited 0: full Rust tests, clippy, Rust formatting, full Windows Ninja build, full CTest with `RULED_E2E_REQUIRE=1` (18/18), generated card checklist, C++ helper formatting, and `git diff --check`. The retained logs use the `Blight precommit` labels. Live tracker review confirmed that the two deferred calibration cards are already explicitly owned by open issues #155 and #159 as described above.
