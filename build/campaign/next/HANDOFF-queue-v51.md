# Handoff after queue-v51

## Delivered

Eight families, 47 cards, independently reviewed, gated, committed and pushed to
`origin/card-campaign`. Family commits `3b97c548f`..`35873aa1e`; shared evidence `2b7b2a1b0`;
baseline classification `4ab4e4c99`; gated remote SHA `4ab4e4c99`.

| Family | Cards | Gate | Commit |
|---|---:|---|---|
| `01-token-spells` | 11 | A | `3b97c548f` |
| `02-splicer-cycle` | 5 | A | `2819f4bc9` |
| `03-artifact-value-tutors` | 4 | A | `c21ed13e5` |
| `04-subtype-harbingers` | 6 | B | `5c22ada43` |
| `05-aura-restrictions` | 7 | B | `340172af7` |
| `06-etb-each-discard` | 3 | B | `f003abe7a` |
| `08-attack-team-pumps` | 6 | C | `27d0dfcbd` |
| `09-spirit-arcane-payoffs` | 5 | C | `35873aa1e` |

Batch gates `verify-8bd9732e-bf56-4343-92ef-cec4c77dcc07`,
`verify-4c42320b-2186-496e-823a-00b3109b480a`,
`verify-2703d25f-3914-4e95-8f19-51d135e87681`, all exit 0 (`Side Rust`, `-CardData`).
`CARDS.md` now reports 5314 full + 8 partial / 35533 cards. `status.tsv` has eight new verified
rows.

## Queue state

`build/campaign/queue-v51/` holds the selection, revalidation, 47 source/ruling packets, 47 maps,
eight family records, frozen bundles and report. Queue-v51 is exhausted. The next run should
expand under `queue-v52/`.

## Ejected / blocked

- `07-blocked-pumps` (`Elvish Berserker`, `Sparring Golem`, `Gang of Elk`, `Rabid Elephant`) was
  ejected after review under #518: the engine has no once-per-combat "becomes blocked" trigger with
  a resolution-time blocker count.
- `06-blocking-pumps` (`Piety`, `Rally`, `Hold the Line`) was rejected before freezing because
  `PumpAll`/`GrantKeywordsAll` use `CreatureScopeFilter`, which has no blocking combat-role scope.
- Existing blockers #515–#524 were not retried.
- `Invisibility` was corrected during review to `excluded_subtypes: ["Wall"]` with real
  Wall/non-Wall blocking tests.

## Environment notes

- Batch C's first gate stopped on the 10 GiB headroom check at 9.9 GiB; `cargo clean --profile dev`
  recovered to 33 GiB and the retry passed.
- Batch A's first gate stopped on the `vital_splicer` conformance fixture classification; the row is
  in `tricerules-core/tests/conformance/baseline.tsv`.
