# 01-token-spells

Shape: One-shot spell creating a fixed token or fixed token cohort with no other effect.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Call of the Conclave | `e1635acd-ed1e-4038-a11a-6518df285253` | {G}{W} | Sorcery | — | Create a 3/3 green Centaur creature token. |
| Elemental Summoning | `f9e668f9-21af-4fd8-9a37-6505ab2234e3` | {3}{U/R}{U/R} | Sorcery — Lesson | — | Create a 4/4 blue and red Elemental creature token. |
| Advent of the Wurm | `eb62aa4b-c11b-4195-ae85-cff8f78ce17b` | {1}{G}{G}{W} | Instant | — | Create a 5/5 green Wurm creature token with trample. |
| Spore Swarm | `8fb599a7-2c61-446b-a7a7-cd5367f9b796` | {3}{G} | Instant | — | Create three 1/1 green Saproling creature tokens. |
| Master's Call | `d6c9c1f0-050f-40e2-ab6c-6566d0af418c` | {2}{W} | Instant | — | Create two 1/1 colorless Myr artifact creature tokens. |
| Servo Exhibition | `76a6ddc2-f894-4331-89a9-87ca16efd0c0` | {1}{W} | Sorcery | — | Create two 1/1 colorless Servo artifact creature tokens. |
| Sworn Companions | `d1a9fd14-fecb-4d2a-983c-70143238275b` | {2}{W} | Sorcery | — | Create two 1/1 white Soldier creature tokens with lifelink. |
| Icatian Town | `aa8b60b6-cf55-4aaa-9caa-2b17942d8269` | {5}{W} | Sorcery | — | Create four 1/1 white Citizen creature tokens. |
| Knight Watch | `4965f5fe-a5fe-4976-822c-5cf40fea8c34` | {4}{W} | Sorcery | — | Create two 2/2 white Knight creature tokens with vigilance. |
| Flurry of Horns | `0c8ab7bf-7b6a-4a34-b988-9f3f4d84a9ea` | {4}{R} | Sorcery | — | Create two 2/3 red Minotaur creature tokens with haste. |
| Take Up Arms | `61e2d557-f08d-489b-94c8-aff87e5189f9` | {4}{W} | Instant | — | Create three 1/1 white Warrior creature tokens. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`raise_the_alarm` and the queue-v23/v43 token cohorts for `CreateTokens`; each printed token is a registered template mirroring `soldier_w_1_1`.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_token_spells_canary_simplest_call_of_the_conclave_creates_a_centaur`
- `q51_cohorts_batch::q51_token_spells_canary_hardest_advent_of_the_wurm_keeps_trample`
- `q51_cohorts_batch::q51_token_spells_create_the_exact_printed_tokens`
- `mechanic_registry::q51_cohorts_batch::q51_token_spells_map_definitions_and_tokens`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-a.json` after initial and delta reviews; no remaining material findings.
- Batch gate: `verify-8bd9732e-bf56-4343-92ef-cec4c77dcc07` passed exit 0 (`Side Rust`, `-CardData`) covering `01-token-spells..03-artifact-value-tutors`.
- Family commit `3b97c548f`; remote `4ab4e4c99`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
