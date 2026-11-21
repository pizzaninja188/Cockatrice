# Campaign report: queue and eleven families

## Phase 1: queue

- Source: `oracle-cards.jsonl.gz`, bulk id `27bf3214-1271-490b-bdfe-c0be6c23d02e`,
  updated `2026-11-10T16:02:03.121-06:00`.
- SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`, equal to
  `oracle-cards.jsonl.gz.meta.json`.
- Normalized index: `build/campaign/oracle-index.tsv` (29,663 unimplemented Commander-legal
  cards; 2,980 implemented registry cards matched to Oracle in the pre-campaign snapshot).
- Ranked families: `build/campaign/families/index.tsv`; packets under
  `build/campaign/families/<rank>-<slug>/`.
- Top 10: see `build/campaign/report.md`.
- Selected family: `01-savai-triome` (tapped cycling lands), 16 unimplemented cards, four
  reviewed analogues with shared tests, 20-card family including the four shipped analogues.

## Phase 2: first family

Implemented 16 tapped cycling lands:

Canyon Slough, Coastal Peak, Festering Thicket, Glittering Massif, Indatha Triome,
Jetmir's Garden, Ketria Triome, Raffine's Tower, Rain-Slicked Copse, Raugrin Triome,
Savai Triome, Spara's Headquarters, Umbral Expanse, Xander's Lounge, Zagoth Triome,
Ziatora's Proving Ground.

- RON: `tricerules/tricerules-cards/data/*.ron` (16 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (16 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/deck_coverage_cycling_taplands.rs`.
- Conformance exceptions: `tricerules/tricerules-core/tests/conformance/baseline.tsv`.
- Family evidence: `build/campaign/families/01-savai-triome/family.md`.
- Exact source and rulings: `build/campaign/families/01-savai-triome/source/` and `rulings/`.
- Presentation metadata: refreshed once; fingerprints and `CARDS.md` updated.

## Evidence status

- Canary red/green: focused `scenario deck_coverage_cycling_taplands` logs under
  `build/verification-logs/`.
- Independent semantic review: approved for reviewed card semantics by the read-only reviewer.
  First verdict required exact draw-count evidence; the delta added exact active-player
  hand/library and opponent-library assertions and was approved in follow-up delta review.
- Frozen review bundles: `build/campaign/review/cycling-taplands` (v1) and
  `build/campaign/review/cycling-taplands-v2` (delta). v2 patch SHA-256
  `c259927e67c7a3e69eb8095c69f050d14f6d487d07ff4cf7ea3253108df62f80`.
- Delivery: commit `b34049f8b` on `card-campaign`, pushed to
  `origin/card-campaign`; remote SHA matches the local commit.
- Final gate: PASS, exit 0. Command: `scripts/verify.ps1 -Side Rust -CardData -CardsXml
  build/campaign/source/cards-validation.xml` (the `-CardsXml` override supplies the
  checklist built from `CARDS.md` because the local `cards.xml` is stale). Summary:
  `build/verification-logs/verify-da1f4263-e70a-4833-bf2f-10e156e3d9ae/summary.json`.
  The gate ran with `CARGO_PROFILE_TEST_OPT_LEVEL=2`, `CARGO_PROFILE_DEV_OPT_LEVEL=2`,
  `RUST_TEST_THREADS=4` and `TRICERULES_CONFORMANCE_WORKERS=4`; wall time was 7m59s.
  All 11 steps passed: verification environment, CardData Check, dependency boundary,
  Rust tests, Clippy, five package format checks, and `git diff --check`.


## Phase 3: second family

Implemented 19 exact-text damage-to-any-target spells:

Bee Sting, Cinder Storm, Cleansing Screech, Explosive Impact, Fire Ambush, Gin'tak Charge,
Gut Shot, Hornet Sting, Lightning Blast, Open Fire, Precision Bolt, Scorching Spear, Searing
Spear, Searing Wind, Tarfire, Unfriendly Fire, Unyaro Bee Sting, Volcanic Hammer, Zuko's
Offense.

- RON: `tricerules/tricerules-cards/data/*.ron` (19 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (19 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/damage_any_target_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/damage_any_target_batch.rs`.
- Family evidence: `build/campaign/families/02-hornet-sting/family.md`.
- Exact source and rulings: `build/campaign/families/02-hornet-sting/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 19 fingerprint rows plus 19 checklist entries.

Evidence status:

- Canaries: Hornet Sting (simplest) and Gut Shot (Phyrexian cost plus second presentation line)
  passed focused red/green before replication of the remaining 17 cards.
- Independent review: approved for reviewed semantics; no blocking defects. Frozen patch SHA-256
  `f1ecff196581dcd98cb056b22a7b2d7099c9e6f54e506b518f7d90b86695353a`.
  Post-review focused registry rerun passed exit 0.
- Delivery: commit `bab115a4c` on `card-campaign`, pushed to `origin/card-campaign`; remote SHA
  matches the local commit. No PR.
- Final gate: PASS, exit 0. Command: `./build/campaign/run-final-gate.sh`, which runs
  `scripts/verify.ps1 -Side Rust -CardData -CardsXml build/campaign/source/cards-validation.xml`
  with opt-level 2, four test threads/workers and four build jobs. Summary:
  `build/verification-logs/verify-5c2a0125-3991-40e6-95d4-8f9dbb12c26a/summary.json`.
  Wall time 11m45s; all 11 steps passed (environment, CardData, dependency boundary, full Rust
  tests, Clippy, five package format checks, `git diff --check`).
- MTG applicability: CR 115.1a/115.4 (target timing and any-target scope), CR 120.1/120.3a/c/e/h
  (damage results), CR 120.5-120.6 (lethal damage and state-based actions), CR 608.2b (stale
  target fizzle and spell destination), CR 107.4f/118.13a/119.4 (Phyrexian payment and life
  limits), CR 202.3g (Phyrexian mana value), and CR 308.2 (Kindred). No engine change.


## Phase 4: third family

Implemented 17 exact-text team-pump spells:

Banners Raised, Bar the Door, Burn Bright, Charge, Chorus of Woe, Desperate Charge, Ethereal
Guidance, Path of Anger's Flame, Righteous Charge, Scare Tactics, Shield Wall, Solidarity,
Steadfastness, Virtuous Charge, Vitalizing Wind, Warrior's Charge, Warrior's Honor.

- RON: `tricerules/tricerules-cards/data/*.ron` (17 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (17 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/pump_team_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/pump_team_batch.rs`.
- Family evidence: `build/campaign/families/06-ethereal-guidance/family.md`.
- Exact source and rulings: `build/campaign/families/06-ethereal-guidance/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 17 fingerprint rows plus 17 checklist entries.
- Selection: queue rank 6 over rank 3's 10-card family because 17 cards meets the 15–19 strong-reuse
  batch floor with a one-clause data-only shape and a shared actual-card fixture.

Evidence status:

- Canaries: Charge (simplest) and Vitalizing Wind (largest cost/delta) passed focused red/green
  before replication of the remaining 15 cards.
- Independent review: card semantics approved; one gate-blocking test-code finding (unused helper
  would fail Clippy). Frozen v1 patch SHA-256
  `81a36cc5e4f5aec2f3c2ea90fe7a1110ccd36b532a8cbcc4508ac3015551f150`. The fix changed only
  test helper/type-declaration code, with no RON, map, assertion or expected-value change. Frozen
  v2 patch SHA-256 `9ebaa3eb880b2fabab11c0cf3f4e07b0dfbcddcc3b1c601dcd69b61d27930f46`.
- Delivery: commit `1fbe28156` on `card-campaign`, pushed to `origin/card-campaign`; remote SHA
  matches. No PR.
- Final gate: first run failed on `clippy::type_complexity`; the test table was rewritten with an
  equivalent type alias and focused Clippy passed. The rerun passed exit 0. Command:
  `./build/campaign/run-final-gate.sh` (`scripts/verify.ps1 -Side Rust -CardData -CardsXml
  build/campaign/source/cards-validation.xml`). Summary:
  `build/verification-logs/verify-892ad38c-5a6a-4f80-9db2-4252836f6b33/summary.json`; all 11
  steps passed in 7m16s. Failed run preserved:
  `build/verification-logs/verify-b32f530c-58e3-48c4-8acd-eb8417e0967f/summary.json`.
- MTG applicability: CR 611.2c (affected set fixed at resolution), CR 613.4c (layer-7c P/T
  modification), CR 514.2 (until-end-of-turn expiry), with the saved rulings confirming that later
  entrants and later-controlled creatures are unaffected. No engine change.

## Phase 5: blocked candidate (attacking/blocking damage)

Selected `03-hamato-ninp` (queue rank 3): 10 exact-text Commander-legal cards that deal damage to
target attacking or blocking creature. The cards, review maps, actual-card scenario and registry
tests were implemented and frozen; the red canaries and green focused runs passed, and a metadata
refresh produced exactly ten fingerprint/checklist entries. The frozen patch is
`build/campaign/review/attacking-blocking-v1/patch.diff` (SHA-256
`3adb74be77c09a642ca6f9a6aa55da59590d00bce8e6e58c23f19bac7a5391eb`).

Independent review withheld semantic approval and identified two blocking engine gaps:

- `combat::remove_combat_participants` drops the attacker-keyed blocker group, so a surviving
  blocker loses its blocking role when its attacker leaves combat. The saved Arrows of Justice
  ruling and CR 506.4/509.1g require it to remain blocking through end of combat.
- `priority.rs` removes an attacker when the player it attacked leaves the game. CR 800.4e only
  suppresses combat damage to that player; it does not remove the attacker from combat.

The review also found that the red-hybrid test did not force the red branch, the fizzle loop did
not assert the fizzle event/destination, and an unused test helper would fail Clippy. Engine changes
are outside this campaign's scope, so the family was ejected without a gate, commit or push. All ten
Oracle IDs are recorded in `build/campaign/blocked.tsv` and tracked by issue #515; no cards were added
to `queue-exclusions.txt` or `CARDS.md`. The worktree was restored to `1fbe28156` on `card-campaign`. After the checkpoint the campaign
resumed with the prepared read-only family 05 rather than retrying this one.

## Phase 6: fourth family (fixed-bonus Equipment)

Selected `05-torch-gauntlet` (queue rank 5): 21 exact-text Commander-legal Equipment cards with
one attached P/T modifier and one equip activated ability.

- RON: `tricerules/tricerules-cards/data/*.ron` (21 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (21 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/equipment_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/equipment_batch.rs`.
- Family evidence: `build/campaign/families/05-torch-gauntlet/family.md`.
- Exact source and rulings: `build/campaign/families/05-torch-gauntlet/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 21 fingerprint rows plus 21 checklist entries
  (3133 → 3154 full).

Evidence status:

- Canaries: Bone Saw (simplest), Shuko (`{0}` equip), and Slagwurm Armor (reminder and host
  departure) passed focused red/green before replication of the remaining 18 cards.
- Independent review: approved for reviewed semantics; no blocking defects and no missing card
  evidence. One optional CR citation correction (704.5p → 704.5n) was applied as a clerical delta
  with no definition, map, assertion or expected-value change. Reviewed v1 patch SHA-256
  `8bf2f8d6bfc7d0dcb08129e6a8ebf117fae23805681c08a24cb1d8fcdb9e5384`; final v2 patch SHA-256
  `ea7cf4d0de02ec43b79be02ba5b302dc0c87dc654d0c494262ebc3df7bd8224e`.
- Delivery: commit `ddcc84d25` on `card-campaign`, pushed to `origin/card-campaign`; remote SHA
  matches the local commit. No PR.
- Final gate: PASS, exit 0. Command: `./build/campaign/run-final-gate.sh` (`scripts/verify.ps1
  -Side Rust -CardData -CardsXml build/campaign/source/cards-validation.xml`, opt-level 2, four
  test threads/workers, four build jobs). Summary:
  `build/verification-logs/verify-7d8462d2-195b-4c51-8e00-864111dd175d/summary.json`; all 11
  steps passed.
- MTG applicability: CR 301.5/301.5a-c (Equipment attachment), CR 702.6a (equip timing and
  target), CR 613.4c (layer-7c P/T modification), CR 704.5n (detachment), and CR 608.2b (stale
  target fizzle), plus Shuko's saved zero-cost re-equip ruling. No engine change.

## Phase 7: fifth family (forestwalk creatures)

Selected `08-zendikar-farguide` (queue rank 8): 16 exact-text Commander-legal creatures whose only
rules text is Forestwalk.

- RON: `tricerules/tricerules-cards/data/*.ron` (16 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (16 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/forestwalk_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/forestwalk_batch.rs`.
- Family evidence: `build/campaign/families/08-zendikar-farguide/family.md`.
- Presentation metadata: exactly 16 fingerprint rows and 16 checklist entries (3154 → 3170 full).

Evidence status:

- Canaries: Zodiac Rabbit (simplest) and Zendikar Farguide (hardest/namesake) passed focused
  red/green before replication.
- Independent review: approved for reviewed semantics; no blocking defects and no missing card
  evidence. Frozen patch SHA-256
  `6721d1e1a7bd375dc828e068540d342ea8cd357b70824c7041a0030ed1dcf354`.
- Delivery: commit `de01d955e` on `card-campaign`, pushed to `origin/card-campaign`; remote SHA
  matches. No PR.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-5c123502-7962-4787-b968-cb54f5fba9bb/summary.json`; all 11
  steps passed.
- MTG applicability: CR 702.14a-c (landwalk evasion and the defending-player land check) and
  CR 509.1b (blocking restrictions). No engine change.

## Phase 8: sixth family (regenerate creatures)

Selected `09-phyrexian-monitor` (queue rank 9): 13 of 15 exact-text/parameter regeneration
creatures. Rimebound Dead (`{S}` snow cost) and Drowned (ambiguous presentation source) are
recorded blockers.

- RON: `tricerules/tricerules-cards/data/*.ron` (13 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (13 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/regenerate_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/regenerate_batch.rs`.
- Family evidence: `build/campaign/families/09-phyrexian-monitor/family.md`.
- Presentation metadata: exactly 13 fingerprint rows and 13 checklist entries (3170 → 3183 full).

Evidence status:

- Canaries: Dutiful Thrull (simplest), Clay Statue (generic cost) and Ancient Silverback (hardest,
  reminder) passed focused red/green before replication.
- Independent review: v1 required the checklist to exclude Drowned and the CR citations to be
  corrected. After the corrective refresh and evidence rebuild, v2 was approved for reviewed
  semantics with no remaining blocking defect. Final v2 patch SHA-256
  `e5de7eca255ee6275014e78669648e08455e25dfe481ebd34ff5ac63669ec89d`.
- Delivery: commit `08c4c1134` on `card-campaign`, pushed to `origin/card-campaign`; remote SHA
  matches. No PR.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-b5f35db8-ab82-4a7b-91a5-e44e6d07adb3/summary.json`; all 11
  steps passed.
- MTG applicability: CR 701.19a-c, 614.8, 506.4 and 118.6/601.2h. No engine change. Rimebound Dead
  is blocked by snow-mana support (issue #516); Drowned by presentation-source ambiguity (issue #517).

## Phase 9: seventh family (shrink-target spells)

Selected `11-throttle` (queue rank 11): 16 exact-text Commander-legal spells that give a target
creature a negative P/T until end of turn.

- RON: `tricerules/tricerules-cards/data/*.ron` (16 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (16 new maps).
- Shared semantic test: `tricerules/tricerules-core/tests/scenario/shrink_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/shrink_batch.rs`.
- Family evidence: `build/campaign/families/11-throttle/family.md`.
- Presentation metadata: exactly 16 fingerprint rows and 16 checklist entries (3183 → 3199 full).

Evidence status:

- Canaries: Disfigure (simplest), Dismember (Phyrexian two-line presentation) and Disorient
  (power floor) passed focused red/green before replication.
- Independent review: approved for reviewed semantics; no blocking defects and no missing required
  evidence. Frozen patch SHA-256
  `18d09a1a99465895f27c01b08a24352a29cc7506d88e95c5d3925119f3f1dd0e`.
- Delivery: commit `cccc4e99b` on `card-campaign`, pushed to `origin/card-campaign`; remote SHA
  matches. No PR.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-909b7976-229a-4f44-afed-68687062acff/summary.json`; all 11
  steps passed.
- MTG applicability: CR 613.4c (layer-7c negative P/T), CR 514.2 (cleanup expiry), CR 608.2b
  (stale target fizzle), CR 107.4f/119.4 (Dismember's Phyrexian payment) and CR 510.1a (power
  floor). No engine change.

## Blocked cards

All ten attempted attacking/blocking damage cards are blocked by the combat-role persistence
contract: Arrows of Justice, Divine Arrow, Dragon's Presence, Gideon's Reproach, Hamato Ninpō,
Impeccable Timing, Righteous Blow, Sandblast, Slash of Talons and Wanderer's Intervention. See
`build/campaign/blocked.tsv` for exact Oracle IDs and the closest support. No other blocked cards
were carried from the three delivered families.

## MTG applicability summary

Per-family CR citations and rulings are recorded in each delivered family section above. All seven
delivered families are card-data-only and added no engine capability. No new GUI, networking,
privacy or physical-identity surface is introduced; manual GUI acceptance is N/A, while the
existing presentation path remains covered by CardData Check. The blocked fourth candidate did
expose a genuine combat-role contract gap (CR 506.4/509.1g, 800.4e) and was not admitted.

## Acceptance

Automated CardData and Rust gates passed for the seven delivered families. Exact summaries are
recorded in `status.tsv` and each family section. No new GUI interaction, networking, privacy or
physical-identity surface is introduced; manual GUI acceptance is N/A. The blocked candidate has
no gate, commit or push and is not claimed as delivered.


## Phase 10: eighth family (flying self-pump creatures)

Implemented 15 flying self-pump creatures:

Aven Flock, Balshan Collaborator, Blistering Dieflyn, Dragon Hatchling, Drifting Shade,
Dungeon Shade, Firefly, Furnace Whelp, Granite Gargoyle, Hellkite Punisher, Killer Bees,
Marble Gargoyle, Moltensteel Dragon, Moonwing Moth, Tower Drake.

- RON: `tricerules/tricerules-cards/data/*.ron` (15 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (15 new maps).
- Shared semantic scenario: `tricerules/tricerules-core/tests/scenario/pump_self_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/pump_self_batch.rs`.
- Family evidence: `build/campaign/families/12-chilling-shade/family.md`.
- Exact source and rulings: `build/campaign/families/12-chilling-shade/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 15 fingerprint rows plus 15 checklist entries.
- Chilling Shade was excluded for the existing snow-mana blocker (#516) and recorded in
  `blocked.tsv`; it has no RON, map or checklist change.

Evidence status:

- Canaries: Dragon Hatchling (simplest) and Moltensteel Dragon (hardest, two Phyrexian symbols and
  a three-line presentation) passed focused red/green before replication.
- Independent review: approved for reviewed semantics; no blocking defects. Frozen patch SHA-256
  `9c1e90ab5acd825d16c5dc2baca1649f5f5a40f929816b843332e1679e61fc03`. Three optional
  non-blocking suggestions were deferred.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-23893e8c-eb36-45ed-8402-efcd87414502/summary.json`; all 11
  steps passed.
- Delivery: commit `47d9e6af806f31000c6b1ffa8aa9c25ed74ab84d` on `card-campaign`, pushed to
  `origin/card-campaign`; remote SHA matches.
- MTG applicability: CR 702.9a/b (flying), CR 602.1a/602.2b and 601.2h (activation and payment),
  CR 611.2a (until-end-of-turn duration), CR 514.2 (cleanup), CR 107.4e (`{B/R}`), and CR
  107.4f/119.4 (`{R/P}`). No engine change.

## Phase 11: ninth family (self-pump creatures)

Implemented 28 self-pump creatures:

Adarkar Sentinel, Angelfire Crusader, Boa Constrictor, Capashen Templar, Carrion Ants,
Dragon Engine, Dread Shade, Fetid Horror, Firescreamer, Flame Spirit, Flamekin Brawler,
Frozen Shade, Hoar Shade, Honor Guard, Immolating Souleater, Jousting Dummy, Kjeldoran Outrider,
Llanowar Vanguard, Loch Korrigan, Looming Shade, Molten Ravager, Nantuko Shade, Narstad Scrapper,
Parapet Watchers, Primeval Shambler, Ridgeline Rager, Sea Spirit, Storm Shaman.

- RON: `tricerules/tricerules-cards/data/*.ron` (28 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (28 new maps).
- Shared semantic scenario: `tricerules/tricerules-core/tests/scenario/pump_self_cohort.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/pump_self_cohort.rs`.
- Family evidence: `build/campaign/families/10-sea-spirit/family.md`.
- Exact source and rulings: `build/campaign/families/10-sea-spirit/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 28 fingerprint rows plus 28 checklist entries.
- Rank 10 was selected after family 12: it is the remaining family in the 20–30 exact-text band
  and shares the just-reviewed self-pump activated-ability fixture.

Evidence status:

- Canaries: Flamekin Brawler (simplest) and Immolating Souleater (hardest, `{R/P}` and reminder)
  passed focused red/green before replication.
- Edge cases: Boa Constrictor/Llanowar Vanguard `{T}` summoning-sickness and tapped rejection;
  Loch Korrigan/Parapet Watchers both hybrid branches plus off-color rejection.
- Independent review: approved for reviewed semantics; no blocking defects. Frozen patch SHA-256
  `0c544dea4d022f6e7ab35fd7d1ae04009bb6246b34999fa19c2945b3f9415c06`. Two optional
  non-blocking suggestions were deferred.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-cc4eb831-71af-4de4-8082-18a35abbdef4/summary.json`; all 11
  steps passed.
- Delivery: commit `c7dfbc7bde626a64a0948d3f2466484864149491` on `card-campaign`, pushed to
  `origin/card-campaign`; remote SHA matches.
- MTG applicability: CR 602.1a/602.2b and 601.2h/118.6 (activation and payment), CR 302.6 (tap
  cost readiness), CR 107.4e (hybrid), CR 107.4f/119.4 (Phyrexian), CR 611.2a (duration) and
  CR 514.2 (cleanup). No engine change.

## Phase 12: tenth family (pump-and-trample spells)

Implemented 12 pump-and-trample spells:

Awaken the Bear, Brute Strength, Colossal Might, Confidence from Strength, Crash the Ramparts,
Fanatical Fever, Larger Than Life, Massive Might, Predator's Strike, Rush of Adrenaline,
Tread Upon, Unnatural Predation.

- RON: `tricerules/tricerules-cards/data/*.ron` (12 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (12 new maps).
- Shared semantic scenario: `tricerules/tricerules-core/tests/scenario/pump_trample_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/pump_trample_batch.rs`.
- Family evidence: `build/campaign/families/04-tread-upon/family.md`.
- Exact source and rulings: `build/campaign/families/04-tread-upon/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 12 fingerprint rows plus 12 checklist entries.
- Parameter family (10–20 band), selected after families 12 and 10; rank 7 is the remaining
  candidate.

Evidence status:

- Canaries: Tread Upon (simplest) and Confidence from Strength (hardest, Sorcery plus reminder)
  passed focused red/green before replication.
- Independent review: the initial frozen review required two test-evidence corrections (pre-resolution
  target state and owner-graveyard destination). The v2 delta added those assertions and was
  approved; frozen patch SHA-256
  `488a2e524b3b334cdb81cef2a0e2036b3cfe7ae59b309e4b9b48fa97346c8633`. Optional suggestions were
  deferred.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-8778dcfc-0825-47c2-a7ed-2555ed97428b/summary.json`; all 11
  steps passed.
- Delivery: commit `a77f675bc03583ce48f2d09f7f1c1799c0304aee` on `card-campaign`, pushed to
  `origin/card-campaign`; remote SHA matches.
- MTG applicability: CR 115.1/601.2c (single creature target), CR 608.2b (stale-target fizzle),
  CR 611.2a (duration), CR 613.4c/613.1f (layer-7c pump, layer-6 trample) and CR 514.2 (cleanup).
  No engine change.

## Phase 13: eleventh family (negative-pump Auras)

Implemented 10 negative-pump Auras:

Chant of the Skifsang, Clinging Darkness, Debilitating Injury, Enfeeblement, Mire's Grasp,
Pin to the Earth, Sensory Deprivation, Torment, Weakness, Weight of the Underworld.

- RON: `tricerules/tricerules-cards/data/*.ron` (10 new files).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` (10 new maps).
- Shared semantic scenario: `tricerules/tricerules-core/tests/scenario/pump_aura_batch.rs`.
- Registry/fingerprint test: `tricerules/tricerules-cards/tests/pump_aura_batch.rs`.
- Family evidence: `build/campaign/families/07-sensory-deprivation/family.md`.
- Exact source and rulings: `build/campaign/families/07-sensory-deprivation/source/` and `rulings/`.
- Presentation metadata: refreshed once; exactly 10 fingerprint rows plus 10 checklist entries.
- Parameter family (10–20 band), the last remaining ranked candidate.

Evidence status:

- Canaries: Sensory Deprivation (simplest) and Chant of the Skifsang (hardest, saved power-floor
  ruling) passed focused red/green before replication.
- Independent review: approved for reviewed semantics; no blocking defects. Frozen patch SHA-256
  `2ac9c6d0eead59dc2597556ed659bcb4d83bd069d3d39a8ee9cb92de28001ac4`. Optional documentation
  and hardening suggestions were deferred; the CR 303.4g/303.4i and 608.2b citation was corrected.
- Final gate: PASS, exit 0. Summary:
  `build/verification-logs/verify-80659b9d-f477-42a1-bbda-1d39fd3f2bb4/summary.json`; all 11
  steps passed.
- Delivery: commit `b1da368793745c9a75430d72e70295c82582fd15` on `card-campaign`, pushed to
  `origin/card-campaign`; remote SHA matches.
- MTG applicability: CR 303.4/601.2c (Aura attach), CR 613.4c (layer-7c attached modifier),
  CR 704.5m (unattached Aura cleanup) and CR 608.2b (stale-target fizzle), plus Chant's saved
  power-floor ruling. No engine change.

## Current run summary (families 12, 10, 04, 07)

Four families and 65 cards were delivered after the prior campaign checkpoint:

- Family 12 `12-chilling-shade`: 15 flying self-pump creatures; commit `47d9e6af8`; gate
  `verify-23893e8c-eb36-45ed-8402-efcd87414502`.
- Family 10 `10-sea-spirit`: 28 self-pump creatures; commit `c7dfbc7bd`; gate
  `verify-cc4eb831-71af-4de4-8082-18a35abbdef4`.
- Family 04 `04-tread-upon`: 12 pump-and-trample spells; commit `a77f675bc`; gate
  `verify-8778dcfc-0825-47c2-a7ed-2555ed97428b`.
- Family 07 `07-sensory-deprivation`: 10 negative-pump Auras; commit `b1da36879`; gate
  `verify-80659b9d-f477-42a1-bbda-1d39fd3f2bb4`.

All four families have independent semantic review approval, one metadata refresh, a passing
Rust/CardData final gate, and a verified `origin/card-campaign` remote SHA. Chilling Shade is
recorded as a blocked card under issue #516; no engine work was performed. No ranked ready family
remains in the campaign queue, so the run stops at four families rather than forcing a fifth count.

## Acceptance for the current run

- Automated: all four final gates exited 0; complete Rust tests, Clippy, formatting, CardData
  Check, dependency boundary and `git diff --check` passed on the reviewed patches.
- Independent semantic review: approved for families 12, 10 and 07; family 04 required two test
  evidence fixes and its v2 delta was approved.
- Manual GUI acceptance: N/A. The families add card data, activated/spell/Aura behavior through
  existing engine and presentation paths; no new networking, privacy, physical-identity or
  interaction surface was introduced.
- Deferred optional polish from reviews is non-blocking and recorded in each family's evidence.
- Blocked: Chilling Shade remains excluded under the existing snow-mana issue #516; the ten
  attacking/blocking cards remain under #515; Drowned remains under #517. No blocked card was
  partially admitted.


## Phase 14: expanded queue and six-family run

### Queue expansion (v2)

- Source unchanged: pinned `oracle-cards.jsonl.gz` id `27bf3214-1271-490b-bdfe-c0be6c23d02e`,
  updated `2026-11-10T16:02:03.121-06:00`, SHA-256
  `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`; rulings
  `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
- Built `build/campaign/queue-v2/` by joining the live registry, `CARDS.md`,
  `queue-exclusions.txt`, `blocked.tsv` and the pinned bulk by Oracle ID. Excluded implemented,
  blocked, token/emblem/art-series/front_card-only and non-Commander-legal records. The expansion
  found 29,480 unblocked, unimplemented playable cards and 81 exact-text cohorts with a shipped
  analogue; 20 were curated in `queue-v2/candidates.tsv`.
- Ejected four additional ambiguous-presentation names under #517 (Agents of S.H.I.E.L.D., Bounty
  Hunter, Grave Robbers, Heroes for Hire), whose names collide with different-text Mystery Booster
  `front_card` records. Drowned was already recorded under #517.
- Selected six top ready families (21 cards): Warped Landscape land-search (3), Arachnoid
  Adaptation pump-reach-untap (4), Hieromancer's Cage linked exile (3), Captain's Tutelage flash
  Aura (3), Viashino Racketeer optional discard (3), Agonizing Syphon damage-and-life (5).
  Band-deferred: the 41-card vanilla cohort (needs a coherent 20-30 split), Kozilek's Channeler (6),
  Orochi Leafcaller (8) and Sodden Verdure (6).

### Delivered families

| Family | Cards | Commit | Gate summary |
|---|---|---|---|
| `10-warped-landscape` | 3 | `606ece568` | `verify-c4c9f7b7-b7d8-4d9a-ac1d-53d0b560cb16` |
| `12-arachnoid-adaptation` | 4 | `99072390b` | `verify-47e83697-bffd-4ab6-93e8-f5e0b60a6679` |
| `13-hieromancer-s-cage` | 3 | `ebb59ef14` | `verify-12023579-41da-4756-b34e-07e6b59fe567` |
| `14-captain-s-tutelage` | 3 | `bde819959` | `verify-26f45e05-74e7-4ecc-875a-2c53ba32aaf9` |
| `03-viashino-racketeer` | 3 | `0d5b3b97f` | `verify-0b8e0e08-bf51-4234-b2d2-6693b470bf80` |
| `07-agonizing-syphon` | 5 | `1969bfee5` | `verify-b6e0093d-447c-4ea6-bdbb-204832a63141` |

All six are pushed to `origin/card-campaign`; the remote SHA is
`1969bfee5f0df8db089072efc9a3f3a6e88d4dc7`. Each family has an independent semantic review
verdict, one metadata refresh, a passing 11-step Rust/CardData gate (exit 0), and family evidence
under `build/campaign/families/`. Review cycles: Warped Landscape required map-evidence/CR
corrections (v2 delta approved); Arachnoid Adaptation required the saved Vines of the Recluse
untapped-target ruling and its assertion (v2 approved); Hieromancer's Cage was approved at v1 and
strengthened in a v2 delta; Captain's Tutelage required removing a Clippy-blocking dead helper and
strengthening tests (v3 approved); Viashino Racketeer required CR-citation corrections (v2
approved); Agonizing Syphon was approved at v1. Optional non-blocking polish is recorded in the
family evidence.

### Blocked inventory

Unchanged for #515 (ten attacking/blocking cards), #516 (Rimebound Dead, Chilling Shade) and
#517 (Drowned). Added to #517: Agents of S.H.I.E.L.D., Bounty Hunter, Grave Robbers and Heroes for
Hire. Exact Oracle IDs are in `build/campaign/blocked.tsv`. No engine changes were made.

### Acceptance for this run

- Automated: all six final gates exited 0; complete Rust tests, Clippy, formatting, CardData
  Check, dependency boundary and `git diff --check` passed on reviewed patches.
- Independent semantic review: approved for all six families after recorded deltas.
- Manual GUI acceptance: N/A. The families are card data plus actual-card tests; no new
  networking, privacy, physical-identity or interaction surface.
- Delivery: six family commits; no PR. Remote branch SHA verified.

## Phase 15: six ready queue-v2 alternatives

### Selection and revalidation

Revalidated all ten remaining `queue-v2` ready alternates against the live registry, `CARDS.md`,
`queue-exclusions.txt`, `blocked.tsv`, the pinned bulk and current `card-campaign`. Six families
(21 cards) were ready and selected in queue order; `queue-v3` expansion was unnecessary. Evidence:
`build/campaign/next/revalidation-6.md` and each family's `revalidation.txt`.

### Delivered families

| Family | Cards | Commit | Gate summary |
|---|---|---|---|
| `05-glint` | 3 | `c595e84ce` | `verify-06dbdf40-e672-4220-a92f-5e4a7104b4a3` |
| `06-high-rise-sawjack` | 4 | `8e727d388` | `verify-c51cacce-0dc4-4a36-9203-1e0ae039a292` |
| `08-puncturing-blow` | 4 | `ba22ab047` | `verify-9f8f11ae-ef88-4b77-a8fb-f10e2ad8a310` |
| `09-soothsayer-adept` | 4 | `5bc769e84` | `verify-01cffaff-2b68-4c6f-8716-c6f7aa747f41` |
| `16-darkness` | 3 | `586756462` | `verify-ddf06ed5-c3af-42b2-8917-4382ef45b742` |
| `17-izzet-chronarch` | 3 | `caae6d475` | `verify-bb215a90-2378-4ee1-b222-200355e66aaf` |

All six are pushed to `origin/card-campaign`; remote SHA is
`caae6d475772ff9d738c50f094d888b58bea9c0a`. Each family has independent semantic review approval,
one metadata refresh (family 05/06 also refreshed after gate-caught identity corrections), and a
passing 11-step Rust/CardData gate (exit 0). Family evidence is under `build/campaign/families/`.

### Review and gate corrections

- `05-glint`: corrected CR 611.2a/514.2/613.1f/613.4c citations and 608.2b attribution, removed
  priority-related and error-cause vacuity, added control-change fizzle evidence, and fixed the
  canonical `woodcutters_grit` slug after the first gate.
- `06-high-rise-sawjack`: corrected Ezuri's Archers to 1/2 and split two-word subtypes; the first
  gate then required the hyphenated card id `high-rise_sawjack` with face id `high_rise_sawjack`;
  a Clippy-only unused import was removed before the passing run.
- `08-puncturing-blow`: reviewer required explicit fizzle-log/no-exile evidence for the
  stale-target path; citations corrected to CR 120.2/120.3 and 117.1a/307.1.
- `09-soothsayer-adept`: added full type-vector and rejection-state assertions, corrected CR
  701.9b/602.2b/601.2h; the first gate caught a one-hex-digit Research Assistant Oracle ID
  transcription, corrected to `...-4ed0-...`.
- `16-darkness` and `17-izzet-chronarch`: approved at v1 with no findings.

### Blocked inventory

Unchanged: #515 (ten attacking/blocking damage cards), #516 (Rimebound Dead, Chilling Shade),
#517 (Drowned, Agents of S.H.I.E.L.D., Bounty Hunter, Grave Robbers, Heroes for Hire). No new engine
blocker and no engine change. Exact IDs remain in `build/campaign/blocked.tsv`.

### Acceptance for this run

- Automated: all six final gates exited 0; complete Rust tests, Clippy, formatting, CardData Check,
  dependency boundary and `git diff --check` passed on reviewed patches.
- Independent semantic review: approved for all six families after recorded deltas.
- Manual GUI acceptance: N/A. Card data plus actual-card tests; no new networking, privacy,
  physical-identity or interaction surface.
- Delivery: six family commits; no PR. Remote branch SHA verified.

## Phase 16: queue-v2 remainder plus queue-v3 expansion

### Selection and revalidation

Revalidated the four remaining `queue-v2` ready alternatives against the live registry,
`CARDS.md`, `queue-exclusions.txt`, `blocked.tsv`, the pinned bulk and current `card-campaign`.
Because fewer than six remained, ran a fresh read-only `queue-v3` expansion from the pinned bulk
(16 ranked candidates; packets under `build/campaign/queue-v3/`). Selected six ready families:
the four queue-v2 alternatives plus `03-quirion-explorer` and `08-mindstatic` from queue-v3.

### Delivered families

| Family | Cards | Commit | Gate summary |
|---|---|---|---|
| `18-urborg-volcano` | 3 | `b29ed9070` | `verify-52ad1881-57a8-4ddf-86f5-488d3b74815b` |
| `19-dueling-rapier` | 3 | `333b76478` | `verify-215fd5a9-8658-48d6-8391-84a30fc24880` |
| `21-recollect` | 3 | `48b5948bb` | `verify-fe837036-c9e8-4e28-b1ab-9024e54c2ba3` |
| `23-aggressive-urge` | 3 | `5ed09667d` | `verify-d084875c-d8c0-49b7-b75e-30360f369c7c` |

Remote SHA after these families: `5ed09667d1d07e64895eba72324d41de406fc763`.

- `18-urborg-volcano`: reviewed at v1 with no blocking findings; one optional fingerprint-field
  assertion improvement is recorded as non-blocking. First gate run stopped at rustfmt only;
  `cargo fmt --all` changed layout, and the second run passed all 11 steps.
- `19-dueling-rapier`: v1 required a full-pool (including colorless) snapshot for the rejected
  equip-target path; the v2 delta was approved. The first gate run stopped at Clippy on a
  needless struct update; the focused Clippy fix passed and the second run passed all 11 steps.
- `21-recollect`: approved at v1 with no blocking findings; optional map-targeting-path and
  registry-field tightening notes are non-blocking. The first full gate passed all 11 steps.
- `23-aggressive-urge`: v1 approved for semantics; the v2 evidence delta fixed the refresh-log
  citation and supplied an all-definitions-absent scenario red plus registry red before the
  post-restore greens. Gate run 1 stopped at environment preflight (9.8 GiB free); a documented
  `cargo clean --profile dev` reclaimed 33.8 GiB and run 2 passed all 11 steps.

## Phase 17: queue-v3 ready remainder plus queue-v4 expansion (nine families)

### Selection and revalidation

Revalidated the two staged queue-v3 families (`03-quirion-explorer`, `08-mindstatic`) and the six
queue-v3 ready alternates by live name and Oracle ID, not packet slug. All eight were absent from
`data/`, `review-maps/`, `blocked.tsv` and `queue-exclusions.txt`. The queue-v4 expansion added 14
ranked candidates under `build/campaign/queue-v4/`. Live name revalidation then ejected the tapped
U/B life cycle because `A.I.M. Labs` is already implemented as `a.i.m._labs` (and the B/R group
had `Hell's Kitchen`); the selected ninth family became the colorless two-mana cycle with the
reviewed Sol Ring analogue.

### Delivered families

| Family | Cards | Commit | Gate summary |
|---|---|---|---|
| `03-quirion-explorer` | 3 | `326a8e11f` | `verify-e5c14d36-827a-412d-82fa-def11ee5a78e` |
| `08-mindstatic` | 5 | `ea0216eb3` | `verify-ca9445c5-7b62-480c-93ed-17c476bdad59` |
| `09-colorless-two-mana` | 3 | `4af5d8e91` | `verify-9a82bb38-d45c-4bed-8191-43b581b1ea8a` |
| `10-timber-gorge` | 3 | `b7eeac4a6` | `verify-49cb37ac-dc6e-45e7-8026-e6670aa5b760` |
| `12-ronom-unicorn` | 3 | `405aabc05` | `verify-752a0d6f-1763-4fc1-9341-66e5e1ab6de2` |
| `13-brazen-freebooter` | 3 | `00b5089a7` | `verify-03b89ec7-e674-4ec2-951a-3e23915c9d1b` |
| `14-gaea-s-anthem` | 4 | `86e0b7f12` | `verify-109cbd38-8cba-4472-a1e1-c7d36ebf05de` |
| `15-sylvan-ranger` | 4 | `512b07834` | `verify-24641a12-1e6e-411e-a212-58970dd8fa1c` |
| `16-breath-of-life` | 4 | `1fe6636ba` | `verify-85badfb0-14d5-4916-8388-5a8d89941af2` |

Remote SHA after these nine families: `1fe6636bacabfb9c270052d3f4f02895fffba3d6`.
`CARDS.md` reports 3350 full + 8 partial / 35533 cards.

### Review corrections

- `03-quirion-explorer`: v1 required tapping the Interplanar Beacon itself, adding a
  controller-owned land to prove "opponent" scope, and removing an inapplicable Fellwar Stone
  summoning fixture; v2 approved. The staged `W/U/G` expectation was also corrected to `U/G`.
- `08-mindstatic`: approved at v1 with no material findings. The staged `PayMana` path was
  corrected to `submit_mana_resolution_decision` before review; gate run 1 needed the five
  reviewed conformance-baseline rows, run 2 passed.
- `09-colorless-two-mana`: approved at v1 with no material findings; first gate passed.
- `10-timber-gorge`: approved at v1 with no material findings; first gate passed.
- `12-ronom-unicorn`: v1 required corrected CR citations (602.2b/601.2h/701.21a/701.8a) and removal
  of an unused import; v2 approved, first gate passed.
- `13-brazen-freebooter`: v1 required single-line map spans, an entry-trigger assertion and a
  complete review mirror; v2 approved, first gate passed.
- `14-gaea-s-anthem`: v1 required an exact full anthem-filter assertion, removal of an unused
  import, frozen-content green reruns and CR 604.2/611.3b/613.4c/109.5 citations; v2 approved,
  first gate passed.
- `15-sylvan-ranger`: v1 required corrected nested program primitive paths, recorded intentional
  branch `Fallback` rows, and frozen-content green reruns; v2 required one more family.md wording
  correction; v3 approved, first gate passed.
- `16-breath-of-life`: v1 required corrected CR 110.2/110.2a/110.5b citations, exact full-filter
  registry assertions and frozen-content green reruns; v2 still had the old matcher; v3 approved,
  first gate passed.

### Blocked inventory

Unchanged: #515 (ten combat-role cards), #516 (Rimebound Dead, Chilling Shade; snow mana),
#517 (Drowned, Agents of S.H.I.E.L.D., Bounty Hunter, Grave Robbers, Heroes for Hire). No new
engine blocker and no engine change.

### Acceptance for this run

- Automated: all nine final gates exited 0; complete Rust tests, Clippy, formatting, CardData
  Check, dependency boundary and `git diff --check` passed on reviewed content.
- Independent semantic review: approved for all nine families after recorded deltas.
- Manual GUI acceptance: N/A. Card data plus actual-card tests; no new interaction, networking,
  privacy, physical-identity or presentation-contract surface.
- Delivery: nine family commits; no PR. Remote branch SHA verified.

## Queue-v9 run (29 cards, nine families)

- Delivered `01-exile-target-creature`, `02-etb-damage-any-target`,
  `03-combat-damage-draw`, `04-dies-shrink-opponent`, `05-aura-cant-attack-block`,
  `06-put-creature-on-top`, `07-etb-loot`, `08-flying-etb-loot`, `09-mana-any-color`.
- Commit `6a3de7cb0`; remote `origin/card-campaign` SHA
  `6a3de7cb0a54ac27a753922f1538d331c3013565`.
- Final gate `build/verification-logs/verify-29647aed-c626-4b9d-a813-5bcf83407514/summary.json`,
  exit 0, complete Rust tests, Clippy, formatting, CardData Check, dependency boundary and
  `git diff --check`.
- Independent semantic review approved all three batches; the batch-A stale-target evidence and
  the batch-B/C fingerprint logs were added before the gate.
- `CARDS.md`: 3519 full + 8 partial / 35533 cards. `blocked.tsv` unchanged (#515, #516, #517).
- Manual GUI acceptance: N/A. Card data plus actual-card tests; no new interaction, networking,
  privacy, physical-identity or presentation-contract surface.

## Queue-v10 run (45 cards, nine families)

- Delivered `01-filter-lands`, `02-pain-lands`, `03-cycling-lands`,
  `04-cycling-color-lands`, `05-vivid-lands`, `06-explore-etb`,
  `07-dies-damage-any-target`, `08-dies-gain-life`, `09-team-pump-activated`.
- Commits `d50f7be3d`, `1591e28df`, `657317c6f`, plus metadata ordering `676ab629f`;
  remote `origin/card-campaign` SHA `676ab629f0d3b35a6b9801bbdba424b40b3f1f48`.
- Final gates: batch A `verify-208915f3-cf40-4ccd-84c0-bb09d0e4d1fa`, batch B
  `verify-aa4dd9ba-b21b-4742-82bd-9cf54e5953e1`, batch C
  `verify-1234cca2-32d7-4757-95df-e46b0a645ae5`; all exit 0 with complete Rust tests,
  Clippy, formatting, CardData Check, dependency boundary and `git diff --check`.
- Independent semantic review approved all nine families after recorded deltas: corrected CR
  citations (106.4, 109.5/120.3a, 614.1d/614.12, 616.1, 118.3/602.2, 701.44c, 701.20a, 608.2d,
  115.1d/603.3d/115.4), strengthened Vivid all-five-options and empty-counter evidence, and
  corrected the Cycling copy-from statement.
- `CARDS.md`: 3564 full + 8 partial / 35533 cards. `blocked.tsv` unchanged (#515, #516, #517).
- Manual GUI acceptance: N/A. Card data plus actual-card tests; no new interaction, networking,
  privacy, physical-identity or presentation-contract surface.

## Queue-v16 run (49 cards, nine families)

- Delivered `01-vanilla-red` (9), `02-first-strike` (3), `03-trample` (4),
  `04-self-pump-black` (3), `05-self-pump-red` (3), `06-self-pump-green` (6),
  `07-convoke` (12), `08-prowess` (5) and `09-protection-red` (4).
- Commit `fb5050eb40261080dacde8a5776734842beabe0d`; remote
  `origin/card-campaign` SHA verified as the same commit.
- Review bundles `queue-v16-full-v1` (patch `b2ab5cf5…`), `queue-v16-delta-v1`
  (clerical evidence delta, patch `d9242aaf…`) and `queue-v16-delta-v3` (shared
  conformance fixture and evidence sync, patch `31f705c9…`); all approved for
  reviewed semantics.
- Final gate `verify-7d44f26e-5bf0-4684-b48b-591055bc39ab`, exit 0: complete
  Rust tests, Clippy, formatting, CardData Check, dependency boundary and
  `git diff --check`. The first gate exposed only two conformance-fixture gaps
  (Autochthon Wurm's fifteen-mana generic cast and Disciple of Law's hand-zone
  Cycling baseline row); both were corrected narrowly and delta-reviewed.
- `CARDS.md`: 3921 full + 8 partial / 35533 cards. `status.tsv`: 135 verified
  families / 840 cards, 1 blocked family. `queue-exclusions.txt` now lists all
  688 campaign-added card ids.
- `blocked.tsv` unchanged (#515, #516, #517). No engine, protobuf, relay or
  client change and no new interaction, networking, privacy, physical-identity
  or presentation-contract surface.
- Manual GUI acceptance: N/A. Card data plus actual-card tests.

## Phase 17: queue-v17 nine-family batch

Queue-v16 was fully delivered; queue-v17 (`build/campaign/queue-v17/selection.md`)
expanded the remaining Commander-legal normal-layout pool into nine reuse-only
cohorts. Implemented 31 cards:

- `01-etb-scry` (4), `02-flier-blocker` (3), `03-aura-menace` (4),
  `04-first-strike-scry` (4), `05-shrink-draw` (4), `06-flash-hexproof-aura` (3),
  `07-landfall-trample` (3), `08-etb-damage-creature` (3) and
  `09-team-pump-on-enter` (3).

- Canaries: Welkin Tern (`02-flier-blocker`, simplest) and Starlit Mantle
  (`06-flash-hexproof-aura`, hardest), each with an intended `MissingCard` red log
  and focused green scenario and registry logs.
- Review bundles `queue-v17-full-v1` (rejected only for two stale map fixture
  links), `queue-v17-full-v2` (corrected evidence and metadata; APPROVE with no
  material findings) and `queue-v17-full-v3` (compile-only needless-borrow
  cleanup; no semantic change).
- One metadata refresh with `build/campaign/source/cards-validation.xml` added
  exactly 31 checklist transitions and 31 fingerprint rows; no unrelated drift.
- Conformance baseline passed with no new rows.
- Commits `14c9ee461` (nine families) and `cc01c62de` (Clippy borrow cleanup);
  pushed to `origin/card-campaign`; remote SHA verified.
- Final gate `verify-a2cd69d8-19fe-49b9-9ec7-932b75e4fe8e`, exit 0: complete
  Rust tests, Clippy, formatting, CardData Check, dependency boundary and
  `git diff --check`.
- `CARDS.md`: 3952 full + 8 partial / 35533 cards. `status.tsv`: 145 verified
  families / 881 cards, 1 blocked family.
- `blocked.tsv` unchanged (#515, #516, #517). No engine, protobuf, relay or
  client change. Manual GUI acceptance: N/A (card data plus actual-card tests).


## Phase 18: queue-v18 nine-family batch

Queue-v17 was fully delivered; queue-v18 (`build/campaign/queue-v18/selection.md`)
expanded the remaining Commander-legal pool with an exact-normalized scan and a
clause-composition scan. Implemented 37 cards:

- `01-flying-self-pump` (5), `02-aura-flying-pump` (6), `03-flash-aura-pump` (5),
  `04-aura-etb-draw-pump` (4), `05-flash-aura-shrink` (4), `06-damage-flashback` (4),
  `07-mass-shrink` (3), `08-lifelink-pump` (3) and `09-affinity-flying` (3).

- Canaries: simplest and hardest of every family were covered by the intended
  `MissingCard` red log (`build/verification-logs/20261115-queue-v18-canary-red.log`),
  then green by the shared actual-card scenarios.
- Review bundles `queue-v18-full-v1` (all nine; two material evidence findings) and
  `queue-v18-delta-v1` (flashback-fizzle exile and later-control-change mass shrink;
  APPROVE). The material findings were fixed before the gate.
- One metadata refresh with `build/campaign/source/cards-validation.xml` added exactly
  37 checklist transitions and 37 fingerprint rows; no unrelated drift.
- Conformance baseline passed with no new rows
  (`build/verification-logs/20261115-queue-v18-conformance-baseline-w4.log`).
- Commits `c33f69fc7` and `82990b3ed` (batch A), `dbeb279ad` (batch B) and
  `4229b9bfb` (batch C); pushed to `origin/card-campaign`; remote SHA
  `4229b9bfb3815331b1c06fc81d92a271494047d3` verified.
- Final gate `verify-a1719e2a-43f8-423f-bff3-422a39330d0b`, exit 0: complete Rust
  tests, Clippy, formatting, CardData Check, dependency boundary and
  `git diff --check`. The gate ran on the unchanged reviewed tree covering A+B+C.
- `CARDS.md`: 3989 full + 8 partial / 35533 cards. `status.tsv`: 154 families /
  918 cards total (153 verified / 908 verified cards plus 1 blocked family / 10 cards).
- `blocked.tsv` unchanged (#515, #516, #517). No engine, protobuf, relay or client
  change. Manual GUI acceptance: N/A (card data plus actual-card tests).

## Phase 19: queue-v19 nine-family continuation

Queue-v18 was fully delivered; queue-v19 (`build/campaign/queue-v19/selection.md`)
expanded the remaining Commander-legal pool with an exact-normalized scan and a
clause-composition scan. Implemented 32 cards:

- `01-wall-regenerate` (4), `02-flier-regenerate` (4), `03-wall-pump` (5),
  `04-flash-etb-pump` (3), `07-equipment-vigilance` (3), `09-vanilla-vehicles` (4),
  `05-soft-counter-cycling` (3), `06-fog-cycling` (3), `08-artifact-sac-ping` (3).

- Canaries: Batch A proved a missing-`living_wall` registry red (exit 101) and green;
  Batch B proved a missing-`sleek_schooner` red (exit 101); Batch C proved a
  missing-`lull` red (exit 101). The shared actual-card scenarios execute every card.
- Review bundles `queue-v19-batch-a-v1` and `queue-v19-batch-a-delta-v1` approved the
  13 Batch A cards; `queue-v19-batch-bc-v1` and `queue-v19-batch-bc-delta-v1` approved
  the 19 Batch B+C cards. The Batch B reviewer withdrew a crew-scope finding after
  CR 702.122a ("other untapped creatures") and a cross-Vehicle crew scenario.
- One metadata refresh for Batch A and one for Batches B+C added exactly the expected
  checklist transitions and fingerprint rows; no unrelated drift.
- The conformance baseline gained the reviewed unsupported-fixture rows for the new
  cycling, target and crew cases.
- Commits `0d27e74ff` (Batch A) and `96663bee5` (Batches B+C); pushed to
  `origin/card-campaign`; remote SHA `96663bee57e6ab2d3621033a28a113fa4efd482f`
  verified.
- Final gates `verify-ac4f41f8-5cc5-4211-93e3-7899b305140f` (Batch A) and
  `verify-22dd7cee-ed95-48d4-959f-6c78918351f8` (Batches B+C), both exit 0: complete
  Rust tests, Clippy, formatting, CardData Check, dependency boundary and
  `git diff --check`.
- `CARDS.md`: 4021 full + 8 partial / 35533 cards. `status.tsv`: 163 families /
  950 card rows (162 verified / 940 verified rows plus 1 blocked family / 10 cards).
- `blocked.tsv` unchanged (#515, #516, #517). No engine, protobuf, relay or client
  change. Manual GUI acceptance: N/A (card data plus actual-card tests).

## Phase 20: queue-v21 nine-family three-batch delivery (queue-v20 is recorded in `build/campaign/queue-v20/final-report.md`)

Queue-v20 was fully delivered; queue-v21 (`build/campaign/queue-v21/selection.md`)
expanded the remaining Commander-legal pool with exact-normalized, composition and
parameter-shape scans. Implemented 54 cards in nine families:

- `01-tapped-dual` (10), `02-snow-tapped-dual` (10), `03-mass-shrink` (6),
  `04-cant-be-blocked` (4), `05-loot-activated` (4), `06-put-land` (4),
  `07-pay-life-any` (3), `08-any-color-mana` (3), `09-self-pump-minus` (10).

- Canaries: each family proved a missing-canary red (exit 101) and focused green; the
  shared actual-card scenarios execute every card.
- Review bundles `queue-v21-batch-a-v3`, `queue-v21-batch-b-v3` and
  `queue-v21-batch-c-v3`, with delta approval after fixes. Material findings: a missing
  metadata freeze, a CR 514.2/608.2c citation, canary-only five-color and loot coverage,
  a vacuous zero-toughness test, a missing no-tap assertion, false map fixtures, and two
  documentation identity typos. All were corrected and delta-reviewed.
- Commits `a754f13d9` (Batch A), `5bafed94a` (Batch B) and `ffb86f799` (Batch C);
  pushed to `origin/card-campaign`; remote SHA
  `ffb86f79995aa76465dc441246fe2d87efb97283` verified.
- Final gates `verify-4fe1b0dd-f2b3-45c0-890f-96c83b77fe50`,
  `verify-89a031a8-75bb-4d33-9d53-c2fc0de84531` and
  `verify-e98ff02d-8c53-4873-a2c9-6536b07b866d`, all exit 0.
- One metadata refresh per batch added exactly 26, 12 and 16 checklist transitions and
  fingerprint rows; no unrelated drift. `CARDS.md`: 4112 full + 8 partial / 35533 cards.
  `status.tsv`: 181 families / 1041 cards (180 verified / 1031 cards plus 1 blocked family /
  10 cards).
- `blocked.tsv` unchanged (#515, #516, #517). No engine, protobuf, relay or client change.
  Manual GUI acceptance: N/A (card data plus actual-card tests). No context compaction.

## Phase 21: queue-v36 nine-family three-batch delivery

Queue-v35 was fully delivered; queue-v36 (`build/campaign/queue-v36/selection.md`)
expanded the remaining Commander-legal pool with stable typed shapes covering pump/scry,
shrink/scry, damage/scry, enters-mill, team haste, self deathtouch, pump-fight,
untap-reach and self-unblockable. Implemented 38 cards in nine families:

- `01-pump-scry` (5), `02-shrink-scry` (4), `03-damage-scry` (5),
  `04-etb-mill` (6), `05-team-haste` (4), `06-self-deathtouch` (4),
  `07-pump-fight` (4), `08-untap-reach-pump` (3), `09-self-unblockable` (3).

- Canaries: each family proved a missing-canary red and focused green; the shared
  actual-card scenarios execute every card.
- Review bundles `queue-v36/batch-a`, `batch-b` and `batch-c`, with delta approval after
  fixes. Material findings: CR 115.1a/marked-damage wording, a vacuous short-library mill
  test, false "no pinned ruling" headers, `Opponent` instead of `NotYou` for "creature you
  don't control", and a foreign-activation test that did not exercise foreign control.
  All were corrected and delta-reviewed.
- Commits `831165003` (Batch A), `78a62989b` (Batch B) and `fae5b74ab` (Batch C);
  pushed to `origin/card-campaign`; remote SHA
  `fae5b74abdaa96a9cce04f54bde0ef17992ee60c` verified.
- Final gates `verify-09726331-f6b6-42e8-ac54-85dc8d1cb551`,
  `verify-10a1cf31-f5c5-46a0-8cf4-2a5b9c86e055` and
  `verify-5608425d-b33a-4455-8992-543e40943ee1`, all exit 0.
- One metadata refresh per batch added exactly 14, 14 and 10 checklist transitions and
  fingerprint rows; no unrelated drift. `CARDS.md`: 4727 full + 8 partial / 35533 cards.
  `status.tsv`: 316 rows / 315 verified families / 1646 cards plus 1 blocked family.
- `blocked.tsv` unchanged (#515–#519). No engine, protobuf, relay or client change.
  Manual GUI acceptance: N/A (card data plus actual-card tests). No context compaction.

## Phase 22: queue-v49 nine-family three-batch delivery

- Queue-v48 exhausted; expanded under `build/campaign/queue-v49/` from the pinned Commander-legal
  bulk. Revalidated 42 admitted cards by Oracle ID, printed name, cost, type line, body, layout,
  legality and presentation; stored per-card source and pinned rulings.
- Delivered nine families in three batches: `01-protection-white` (3), `02-aura-keyword` (5),
  `03-aura-pump-keyword` (5), `04-aura-flash-pump-keyword` (4), `05-landfall-simple` (4),
  `09-becomes-tapped` (5), `06-graveyard-hand-etb` (6), `07-sacrifice-mana` (6),
  `08-raid-simple` (4) — 42 cards.
- Each family has a reviewed RON definition, complete map, actual-card scenarios with independent
  expectations, intended red, focused green and independent frozen review. Batch gates:
  `verify-b7051213-7bd3-459d-8cd7-79481a6d6c2b`,
  `verify-361ca08d-2f68-46b8-a068-922eb902f35b`,
  `verify-9d7f3ffd-392a-4d54-b2eb-d0141989e0e0`, all exit 0 (`Side Rust`, `-CardData`).
- `CARDS.md`: 5233 full + 8 partial / 35533 cards. `status.tsv`: 432 verified families.
- Commits `af62a8d30`..`37b8557af`; evidence `0867590b2`, `589b58b1b`, `37a72a0a5`; remote
  `37a72a0a5935dd26ba45024aac0f16f896c0a5a5`.
- Three frozen bundles independently reviewed; delta reviews approved the evidence fixes.
  `blocked.tsv` unchanged (#515–#523 not retried). No engine, protobuf, relay or client change.
  Manual GUI acceptance: N/A (card data plus actual-card tests). No context compaction.

## Phase 23: queue-v51 eight-family three-batch delivery

- Queue-v50 exhausted; expanded under `build/campaign/queue-v51/` from the pinned Commander-legal
  bulk. Revalidated 51 candidates and admitted 47 across eight families; stored per-card source and
  pinned rulings.
- Delivered `01-token-spells` (11), `02-splicer-cycle` (5), `03-artifact-value-tutors` (4),
  `04-subtype-harbingers` (6), `05-aura-restrictions` (7), `06-etb-each-discard` (3),
  `08-attack-team-pumps` (6), `09-spirit-arcane-payoffs` (5) — 47 cards.
- `07-blocked-pumps` was ejected after review: "becomes blocked" needs a once-per-combat trigger
  and a resolution-time blocker count, the existing #518 gap; all four IDs are recorded in
  `blocked.tsv`. `06-blocking-pumps` was likewise rejected before freezing because `PumpAll` has no
  blocking combat-role predicate.
- Each family has a reviewed RON definition, complete map, actual-card scenarios with independent
  expectations, historical intended red, focused green and independent frozen review. Batch gates
  `verify-8bd9732e-bf56-4343-92ef-cec4c77dcc07`,
  `verify-4c42320b-2186-496e-823a-00b3109b480a`,
  `verify-2703d25f-3914-4e95-8f19-51d135e87681`, all exit 0 (`Side Rust`, `-CardData`).
- `CARDS.md`: 5314 full + 8 partial / 35533 cards. `status.tsv`: 449 verified families.
- Family commits `3b97c548f`..`35873aa1e`; shared evidence `2b7b2a1b0`; baseline `4ab4e4c99`;
  gated remote `4ab4e4c99`.
- Invisibility was corrected during review to exclude non-Walls (`excluded_subtypes`), with real
  Wall/non-Wall blocking tests. No engine, protobuf, relay or client change. Manual GUI acceptance:
  N/A (card data plus actual-card tests). No context compaction.

