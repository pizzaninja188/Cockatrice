# 08-attack-team-pumps

Shape: Whenever this creature attacks, a mass pump or keyword grant to a printed scope.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Goblin General | `5fab0d9c-ccf7-4c6a-94b4-959be139dad8` | {1}{R}{R} | Creature — Goblin Warrior | 1/1 | Whenever this creature attacks, Goblin creatures you control get +1/+1 until end of turn. |
| Riot Ringleader | `420d09b5-aced-4f71-914c-90d19f7476dc` | {2}{R} | Creature — Human Warrior | 2/2 | Whenever this creature attacks, Human creatures you control get +1/+0 until end of turn. |
| Blood-Chin Rager | `a7036d08-6ee4-49b1-bbbf-c96a937afa15` | {1}{B} | Creature — Human Warrior | 2/2 | Whenever this creature attacks, Warrior creatures you control gain menace until end of turn. (They can't be blocked except by two or more creatures.) |
| Inspiring Unicorn | `1f9797e2-7ebb-41f9-9199-e9ee575eefd3` | {2}{W}{W} | Creature — Unicorn | 2/2 | Whenever this creature attacks, creatures you control get +1/+1 until end of turn. |
| Chieftain en-Dal | `8aae3a28-dcf3-460e-89f9-4b0c96d6f55e` | {1}{W}{W} | Creature — Human Knight | 2/2 | Whenever this creature attacks, attacking creatures gain first strike until end of turn. |
| Fangren Pathcutter | `36d1a3f4-7903-41aa-9fb7-401d8789b00c` | {4}{G}{G} | Creature — Beast | 4/6 | Whenever this creature attacks, attacking creatures gain trample until end of turn. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`goblin_chieftain` for subtype-scoped `PumpAll`/keyword grants, `stampede`/`rally_the_forces` for attacking scopes, and `ravenous_skirge` for the attack trigger.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_attack_pumps_canary_simplest_goblin_general_pumps_goblins`
- `q51_cohorts_batch::q51_attack_pumps_canary_hardest_blood_chin_rager_grants_menace`
- `q51_cohorts_batch::q51_attack_pumps_apply_each_printed_scope`
- `q51_cohorts_batch::q51_attack_pumps_only_affect_attackers_for_keyword_grants`
- `mechanic_registry::q51_cohorts_batch::q51_attack_team_pumps_map_attack_triggers`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-c.json` after initial and delta reviews; no remaining material findings.
- Batch gate: `verify-2703d25f-3914-4e95-8f19-51d135e87681` passed exit 0 (`Side Rust`, `-CardData`) covering `08-attack-team-pumps..09-spirit-arcane-payoffs`.
- Family commit `27d0dfcbd`; remote `4ab4e4c99`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
