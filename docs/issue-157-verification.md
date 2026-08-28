# Issue 157 implementation and verification

Verified on Windows, 2026-08-28. Scope: counter removal effects and costs, departure counter snapshots, Wither, untap prohibition, and five cards. Barbed Bloodletter remains deferred to the attachment work in issue 155.

## Delivered behavior

- Dockworker Drone recreates its departure counter bag on the selected friendly creature.
- Heirloom Auntie surveils before removing a -1/-1 counter; Reluctant Dounguard removes one after another friendly creature enters, with its intervening condition rechecked.
- Brambleback Brute pays an engine-published counter option together with mana, using the existing activation and payment flow.
- Blossombind taps its attached creature and prohibits untapping and counter placement, without prohibiting removal.
- Wither uses the damage-result pipeline, including prevention, placement prohibitions, lifelink, deathtouch, source last-known information, and combat damage.

The generated card checklist reports 1,520 full and 10 partial cards, an increase of five full cards.

## Automated verification

All final commands exited **0**. Complete local logs are in `build/verification-logs`.

| Gate | Final log |
| --- | --- |
| Full Rust tests (`cargo test --quiet`) | `20260828-112940-545-157-final-Rust-tests.log` |
| Clippy, all targets, warnings denied | `20260828-112901-495-157-clippy-final.log` |
| Rust format check | `20260828-112951-138-157-fmt-check.log` |
| Generated card checklist | `20260828-112654-591-157-card-checklist.log` |
| Full Windows Ninja build | `20260828-112910-604-157-final-Windows-rebuild.log` |
| Full CTest, `RULED_E2E_REQUIRE=1` | `20260828-113003-441-157-full-CTest-required-E2E.log` |

CTest passed **18/18**, including the real Servatrice/sidecar smoke test. `git diff --check` also passed. Focused regressions were run red and green for counter timestamps, typed primitives, trigger-object validation, counter payments, damage interactions, and the client contract.

Regression coverage includes stale generations and invalid counter options without partial payment; aggregate counter debits; removal before sacrifice and post-payment LKI; pre-SBA counter bags; multiple observers and source return; deterministic payment replay; first-strike and ordinary Wither combat; and decider-only legal-cost data.

## Computer-use two-client checks actually performed

Two freshly built Cockatrice clients connected to local Servatrice and tricerules-server with development commands enabled and seed 157. The agent operated visible Qt controls through computer use. Development console commands prepared cards and mana; casts, targets, cost choices, payments, and priority passes used the normal ruled UI. These are agent-operated desktop checks, not human acceptance.

| Check | Observed result |
| --- | --- |
| Brute with two -1/-1 counters and one stun counter | Picker displayed both engine-provided kinds and their available counts. |
| Escape from the counter picker | No activation submitted; six red mana and all counters remained unchanged. |
| Stage -1/-1 choice before mana payment | Opponent saw no staged counter choice or premature mana/counter changes. |
| Complete the first payment | Exactly one -1/-1 counter and two mana were consumed. Both clients showed the committed state and the target's can't-block effect after resolution (11:37:48). |
| Cast Blossombind on tapped, stunned Brute | Aura attached to the same physical creature on both clients; existing counters remained. |
| Resolve Seeker of Skybreak's untap ability on enchanted Brute | Brute remained tapped and retained its stun counter (11:43:33). |
| Pay Brute's ability with its stun counter while enchanted | Stun was removed; the -1/-1 counter remained, and both clients displayed the same result (11:44:31). |
| Pay with the sole remaining counter | No redundant counter menu appeared. Payment removed the last counter and its annotation on both clients, leaving Brute 4/5 (11:45:48). |
| Kill Drone with Lightning Bolt; target enchanted Brute with its death trigger | Drone entered the public graveyard; the trigger resolved without placing a prohibited counter (11:50:08). |
| Repeat lethal damage; target Seeker instead | Exactly the selected Seeker received one +1/+1 counter and became 3/2 on both clients; Brute remained unchanged (11:52:59). |

Reproduction: put Brute on the battlefield, use an opponent's Rowdy Snowballers entry trigger to tap it and add stun, and add six red mana. Exercise cancellation and one -1/-1 payment. Cast Blossombind on Brute and use a ready Seeker of Skybreak to attempt an untap. Pay Brute's ability first with stun, then with its last -1/-1 counter. Put Drone on the battlefield and kill it with Lightning Bolt, testing the two recipients above. Pass priority on both clients for each spell and trigger.

Setup caveat: `move gy Dockworker Drone` relocated the card but did not produce its death trigger. It was not counted as a passing death test; both recorded death checks instead used lethal Lightning Bolt damage. The development relocation path was not changed by this issue.

Heirloom Auntie, Dounguard, Wither interaction matrices, stale command rejection, and replay were covered by automated regressions rather than additional desktop scenarios.

## Rules interaction checklist

1. **Authority and behavior:** Oracle and rulings were fetched on 2026-08-28. Reusable examples: Drone/The Ozolith for departure bags; Auntie/Dounguard for removal effects; Brute/Walking Ballista for removal costs; Sickle Ripper/Barbed Bloodletter for Wither; Frozen in Ice/Spider-Woman for untap prohibition. Barbed Bloodletter card authoring is deferred.
2. **State and identity:** tricerules remains the sole writer. Counter cost selections bind engine ObjectId and zone-change generation, not card names, list positions, or Server_Card IDs. Departure bags survive source return without reading the new object. State-affecting choices remain logged commands.
3. **Timing and composition:** shared cost validation precedes mutation; counter costs precede sacrifice. Counter bags preserve pre-SBA LKI. Untap prohibition precedes stun replacement. Wither follows prevention and uses the shared counter-placement funnel. Existing trigger ordering, targeting, combat restrictions, and payment continuation remain in use.
4. **Players and failures:** controller and deciding-player identities remain explicit; no two-player arithmetic was added. Invalid, missing, stale, and overdrawn selections return established illegal-action errors. Cancellation submits nothing. Physical payment can make its source lethal; the subsequent SBA handles that normally.
5. **Visibility:** new legal counter choices and their nested option fields are per-player. The existing relay redaction path is covered by a recipient privacy regression. Public counter changes are published only after committed payment; the desktop opponent check confirmed no staged preview disclosure.
6. **Propagation:** Rust publishes and validates typed choices; protobuf carries source generation and opaque option IDs; the existing Servatrice relay transports and redacts them; Qt parses, selects, revalidates, and serializes them. New UI behavior remains in ruled files with short upstream call-site adapters. Freeform behavior is unchanged. No new zone-order contract was introduced.
7. **Verification and delivery:** focused red/green regressions, full affected-side gates, generated checklist, and the actual two-client checks above are complete. At the end of verification, no commit, push, or tracker mutation had been performed; publication requires separate user authorization.

## MTG applicability

Implements counter removal versus placement, departure counter recreation (CR 122.8), pre-SBA last-known information (704.8), counter timestamps (613.7c), full cost payment (118.3), intervening conditions (603.4), and Wither damage results and ordering (120.3, 120.4, 702.80). Rule numbers were checked against the [official 2026-08-19 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt). Barbed Bloodletter's attachment-dependent card implementation is deferred to issue 155.
