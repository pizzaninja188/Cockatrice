# 09-spirit-arcane-payoffs

Shape: Whenever you cast a Spirit or Arcane spell, one targeted effect.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Briarknit Kami | `913dcf97-0804-4b21-8daa-f2b481dc0f2f` | {3}{G}{G} | Creature — Spirit | 3/3 | Whenever you cast a Spirit or Arcane spell, put a +1/+1 counter on target creature. |
| Horizon Seed | `3970e021-209d-41d3-9466-8b3ef9ab1a4c` | {4}{W} | Creature — Spirit | 2/1 | Whenever you cast a Spirit or Arcane spell, regenerate target creature. |
| Guardian of Solitude | `712ec339-ffa8-43bb-a826-a7b0f0120b1f` | {1}{U} | Creature — Spirit | 1/2 | Whenever you cast a Spirit or Arcane spell, target creature gains flying until end of turn. |
| Kami of Fire's Roar | `c247382f-821e-4f3c-9d34-450794ce74c3` | {3}{R} | Creature — Spirit | 2/3 | Whenever you cast a Spirit or Arcane spell, target creature can't block this turn. |
| Soul of Magma | `1566af29-bd7b-41bb-a589-499348fbd32c` | {3}{R}{R} | Creature — Spirit | 2/2 | Whenever you cast a Spirit or Arcane spell, this creature deals 1 damage to target creature. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`pyroceratops`/`ivory_cup` for `WheneverPlayerCastsSpell`, `flametongue_kavu` for targeted damage, `cudgel_troll` for regenerate, and `jump` for a keyword grant.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_spirit_arcane_canary_simplest_briarknit_kami_adds_a_counter`
- `q51_cohorts_batch::q51_spirit_arcane_canary_hardest_kami_of_fires_roar_forbids_blocking`
- `q51_cohorts_batch::q51_spirit_arcane_payoffs_fire_for_each_printed_effect`
- `q51_cohorts_batch::q51_spirit_arcane_payoffs_also_fire_for_an_arcane_spell`
- `mechanic_registry::q51_cohorts_batch::q51_spirit_arcane_payoffs_map_the_cast_trigger`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-c.json` after initial and delta reviews; no remaining material findings.
- Batch gate: `verify-2703d25f-3914-4e95-8f19-51d135e87681` passed exit 0 (`Side Rust`, `-CardData`) covering `08-attack-team-pumps..09-spirit-arcane-payoffs`.
- Family commit `35873aa1e`; remote `4ab4e4c99`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
