# Convoke payment contract and acceptance

Implements [issue #145](https://github.com/pizzaninja188/Cockatrice/issues/145) on the shared cost transaction from #144.

## Rules interaction checklist

1. **Authority and rules.** Rust computes the total cost before Convoke, including X, cost methods, additions, increases, reductions, hybrid equivalents, and announced Phyrexian life payments. Untapped creatures controlled by the caster contribute generic or a current derived color; they never produce mana or pay explicit colorless. Summoning sickness and ownership do not disqualify them. Oracle text and rulings were checked for Unexpected Assistance, Merrow Skyswimmer, Appeal to Eirdu, and Sun-Dappled Celebrant. The reusable keyword/payment vocabulary supports all four; the grouped pump fix also preserves Giant Growth.
2. **State and identity.** `tricerules` is the only state writer. Spell and creature selections carry `ObjectId` plus zone-change generation, distinct from definition ID, name, hand slot, face index, and `Server_Card.id`. The final cast revalidates the state revision and the entire transaction before any debit. Taps precede sacrifices when the same creature pays both costs. Existing physical mappings perform exact public taps and movements. A serialized cast replays identically whether or not preview queries preceded it.
3. **Timing and interactions.** The ordinary cast preparation checks priority, timing, source permissions, targets, modes, and other costs. Preview uses that same preparation and payment demand planner. Existing tap events, sacrifice events, triggers, APNAP ordering, SBAs, and effect continuations remain authoritative. Tapping an attacking or blocking creature leaves it in combat. New attachment, counter, delayed-trigger, replacement, and combat-assignment mechanics are N/A. Existing continuous characteristics supply creature types and colors. The cost-free Siege defeat offer rejects extraneous payment payloads instead of ignoring them.
4. **Players and failure paths.** All engine logic uses authenticated player IDs and current controller, without two-player arithmetic. Duplicate, excessive, invalid-color, stale, tapped, and unavailable-resource submissions fail atomically. Refresh retains valid creatures and as much selected ordinary/restricted mana as still fits; retired selections are explained. Invalid previews cannot submit. There is no complete-creature-combination enumeration.
5. **Visibility.** Preview results and payment selections are private. Servatrice sends a fresh standalone response only to the authenticated requester. Queries bypass canonical gameplay, replay logging, command-index advancement, physical synchronization, and broadcast. Preview delivery does not clear normal legal actions. Committed taps and stack/zone movements follow the ordinary public path. Assistance reuses the controller-only discard choice and observer wait behavior.
6. **Propagation.** The protobuf contract is shared by Rust and C++. `RulesRelay`/`RuledGameSession` carry a dedicated sidecar query; `RuledBroadcastRouter` delivers it privately. Qt's headless `RuledPayment` owns transaction revisions, staged selections, pending replies, and exactly-once submission. `RuledPaymentUi` owns menus, highlights, prompts, mana-ability suspension, and the bridge to existing casting. Upstream hooks remain short and ruled-gated. Session reset and blocking choices clear staging; obsolete replies cannot revive it. Freeform and ordinary non-Convoke casts retain their paths.
7. **Verification.** Focused red/green coverage includes mixed payments, invalid payment rollback, selection retention, grouped pump resolution, and rejection of a forged Siege payment. Rust scenarios cover all four cards, all-mana/all-creature/mixed payments, sickness, ownership, hybrid colors, token colors, private discard, combat membership, restricted pools, and serialized replay. Payment unit tests cover X/modifiers, Phyrexian life, explicit colorless, and tap-before-sacrifice. Client tests cover stale/duplicate replies, exact automatic submission, cancellation/reset, and ordinary-state retention. Real-server E2E covers requester-only read-only queries and exact physical taps. Final gates use `AGENT-VERIFICATION.md`; generator feature tests and the card checklist also apply.

## Two-client UI acceptance

These are direct desktop UI checks, separate from the automated CTest E2E suite. Reproduce with `scripts/launch-ruled-game.ps1 -Dev -Seed 145`.

- Cast Appeal to Eirdu targeting a newly controlled creature, stage that creature for Convoke, then cancel. Neither seat should see a committed tap. Repeat with mixed mana; the final contribution should cast once and both seats should see the exact tap, target, and resolved bonus.
- Cast Merrow Skyswimmer using both hybrid colors. Resolve its trigger and inspect the token on both seats. Begin another Convoke spell: the white/blue token should offer white, blue, or generic when those contributions fit. Cancel without tapping it.
- Begin Unexpected Assistance with a ready Llanowar Elves and blue creatures available. A normal click selects Convoke; the separate context-menu mana ability taps the Elves and retires its Convoke selection. Continue using the generated mana and blue creatures. The final payment should cast automatically once.
- Resolve Assistance. Only its controller should see and select the discard candidates; the other seat sees a wait. Confirm the chosen physical card enters the graveyard.
- After activating a creature mana ability during payment, verify Cancel remains available. Cancel must leave the ability's committed tap/mana intact while clearing the spell's staged contributions.

The desktop run on August 26-27 passed the first four flows and exposed the missing Cancel-button refresh after a creature mana ability. The bridge now republishes the pending-cast signal on resume. A rebuilt-client recheck passed: Cancel remained available, cleared spell staging, and preserved the committed Elves tap and one green mana on both seats. Sanitization explanations persist in the local payment prompt across queued previews, with a focused red/green client regression. These checks were performed through desktop computer use, not by a human tester.

## Card data and boundaries

The three effect-bearing spells are authored RON; the white/blue Merfolk token is a separate token definition. Sun-Dappled Celebrant is generated from the verified cached Scryfall bulk input using exact Convoke keyword recognition. Only its intended generated file is added; `CARDS.md` is regenerated. Unknown keyword text or unsupported extra clauses still fail closed.

Waterbend, Station, a general casting-session redesign, and unrelated UI cleanup are deferred.

## Delivery gates (2026-08-27)

All commands exited 0 using the Windows quiet-runner workflow:

- `cargo test --quiet`: full workspace, including 1,025 scenario tests.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.
- Generator feature tests: 20 passed; `scripts/gen-card-checklist.ps1 --check` passed.
- `scripts/build-ninja.ps1`: full Windows build.
- Full `ctest --test-dir build/windows-ninja-all --output-on-failure` with `RULED_E2E_REQUIRE=1`: all 18 targets passed, including the real Convoke E2E.
- New payment C++ files pass `clang-format --dry-run --Werror`; `git diff --check` passes.

Complete command logs are under `build/verification-logs`. Test clients and servers were stopped after the desktop acceptance checks.

**MTG applicability:** Convoke (CR 702.51), casting and cost payment (CR 601.2 and 118), and new-object identity (CR 400.7), checked against the [official Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt). No intentional deviation for the implemented cards.
