# 05-aura-restrictions

Shape: Aura with one attached combat/untap restriction profile and, for two cards, an activation prohibition.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Cloak of Mists | `13358b1d-0a35-48ed-a7fb-70359fa3998f` | {1}{U} | Enchantment — Aura | — | Enchant creature Enchanted creature can't be blocked. |
| Aether Tunnel | `0a90d1fb-7a76-4b62-b001-4a539a8e5df1` | {1}{U} | Enchantment — Aura | — | Enchant creature Enchanted creature gets +1/+0 and can't be blocked. |
| Protective Bubble | `1fba543c-bd22-4f7b-9ff4-0631af8ee814` | {3}{U} | Enchantment — Aura | — | Enchant creature Enchanted creature can't be blocked and has shroud. (It can't be the target of spells or abilities.) |
| Demotion | `c333eb79-300e-4dc0-9126-8dca1bac6ef2` | {W} | Enchantment — Aura | — | Enchant creature Enchanted creature can't block, and its activated abilities can't be activated. |
| Arrest | `81728b98-8cf9-4734-a318-69184bb4d15c` | {2}{W} | Enchantment — Aura | — | Enchant creature Enchanted creature can't attack or block, and its activated abilities can't be activated. |
| Paralyzing Grasp | `c1459d75-2bc2-4dae-8c7e-4e759a4dffc9` | {2}{U} | Enchantment — Aura | — | Enchant creature Enchanted creature doesn't untap during its controller's untap step. |
| Invisibility | `de26b0c6-dfb7-45a8-9d7f-f8d45522d675` | {U}{U} | Enchantment — Aura | — | Enchant creature Enchanted creature can't be blocked except by Walls. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`pacifism` and `petrify` for `AttachedModifier` restrictions, `prohibit_activated` coverage from `petrify`, and `capture_sphere` for the untap-step restriction.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_auras_canary_simplest_cloak_of_mists_makes_the_creature_unblockable`
- `q51_cohorts_batch::q51_auras_canary_hardest_arrest_locks_attack_block_and_activation`
- `q51_cohorts_batch::q51_auras_apply_each_printed_restriction`
- `q51_cohorts_batch::q51_unblockable_auras_reject_an_ordinary_blocker`
- `q51_cohorts_batch::q51_arrest_and_demotion_reject_the_printed_combat_actions`
- `q51_cohorts_batch::q51_aura_follows_its_host_to_the_graveyard`
- `q51_cohorts_batch::q51_paralyzing_grasp_keeps_the_creature_tapped`
- `q51_cohorts_batch::q51_invisibility_is_blocked_only_by_walls`
- `mechanic_registry::q51_cohorts_batch::q51_aura_restrictions_map_static_profiles`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-b.json` after initial and delta reviews; no remaining material findings.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
