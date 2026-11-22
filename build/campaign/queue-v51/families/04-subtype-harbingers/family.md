# 04-subtype-harbingers

Shape: Enters trigger with an optional private subtype search, revealed and put on top of the library.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Boggart Harbinger | `56508ae7-6013-4149-8935-96b8d5091baa` | {2}{B} | Creature — Goblin Shaman | 2/1 | When this creature enters, you may search your library for a Goblin card, reveal it, then shuffle and put that card on top. |
| Kithkin Harbinger | `a3d68848-e8cc-497c-951b-e7111f6d5142` | {2}{W} | Creature — Kithkin Wizard | 1/3 | When this creature enters, you may search your library for a Kithkin card, reveal it, then shuffle and put that card on top. |
| Giant Harbinger | `2445e58b-87ed-4ab2-8209-a5e1f566fba7` | {4}{R} | Creature — Giant Shaman | 3/4 | When this creature enters, you may search your library for a Giant card, reveal it, then shuffle and put that card on top. |
| Flamekin Harbinger | `d6585e30-4ca0-4701-b274-b24f3508dd97` | {R} | Creature — Elemental Shaman | 1/1 | When this creature enters, you may search your library for an Elemental card, reveal it, then shuffle and put that card on top. |
| Elvish Harbinger | `28ddc23c-5b0b-4747-9a33-6845acf27bec` | {2}{G} | Creature — Elf Druid | 1/2 | When this creature enters, you may search your library for an Elf card, reveal it, then shuffle and put that card on top. {T}: Add one mana of any color. |
| Treefolk Harbinger | `98c387bd-c9ba-464a-8380-839a5604a37f` | {G} | Creature — Treefolk Druid | 0/3 | When this creature enters, you may search your library for a Treefolk or Forest card, reveal it, then shuffle and put that card on top. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`sylvan_ranger` and `district_guide` for the optional private search, extended with `destination: TopOfLibrary` as on `vampiric_tutor`.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_harbingers_canary_simplest_flamekin_finds_an_elemental`
- `q51_cohorts_batch::q51_harbingers_canary_hardest_treefolk_accepts_both_branches`
- `q51_cohorts_batch::q51_harbingers_put_only_their_subtype_on_top`
- `q51_cohorts_batch::q51_harbingers_reveal_shuffle_and_may_decline`
- `q51_cohorts_batch::q51_elvish_harbinger_taps_for_any_color`
- `mechanic_registry::q51_cohorts_batch::q51_subtype_harbingers_map_top_of_library_searches`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-b.json` after initial and delta reviews; no remaining material findings.
- Batch gate: `verify-4c42320b-2186-496e-823a-00b3109b480a` passed exit 0 (`Side Rust`, `-CardData`) covering `04-subtype-harbingers..06-etb-each-discard`.
- Family commit `5c22ada43`; remote `4ab4e4c99`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
