# Queue v51 report

## Result

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

Batch gates: A `verify-8bd9732e-bf56-4343-92ef-cec4c77dcc07`, B
`verify-4c42320b-2186-496e-823a-00b3109b480a`, C
`verify-2703d25f-3914-4e95-8f19-51d135e87681`, all exit 0 (`Side Rust`, `-CardData`).

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card,
  34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the
  definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and
  `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: 47 selected maps passed Preparation evidence and executable-reference checks
  (`build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4`, exit 0).
- Presentation audit: `presentation-audit --check` problems 0
  (`build/verification-logs/20261120-190111-742-Offline-card-author.log`).
- Metadata: refresh `card-data-e756693d-c2c1-47a2-83ba-6aedef321489` changed 47 fingerprint rows
  and 47 checklist flips with no unrelated drift; the final no-op validation
  `card-data-a93e3106-4a92-47a2-95c5-dc39cb7ac991` matched the current `CARDS.md`.

## Environment

- Batch C's first gate stopped on the 10 GiB headroom check at 9.9 GiB; the documented
  `cargo clean --profile dev` recovery freed space and the retry passed.
- Batch A's first gate stopped on the `vital_splicer` conformance fixture classification; the row
  was added to `conformance/baseline.tsv` (target-group fixture limitation, not card semantics) and
  the retry passed.

## Queue state

`build/campaign/queue-v51/` holds the selection, revalidation, per-family source/rulings and
records, maps, frozen bundles and this report.

## Ejected or deferred

- `06-blocking-pumps` was replaced by `06-etb-each-discard` because the mass pump vocabulary has
  no blocking combat-role predicate.
- `07-blocked-pumps` was ejected after review: "becomes blocked" has no once-per-combat trigger,
  so all four IDs are blocked under #518. The delivered queue therefore has eight families.
- Existing blockers #515–#524 were not retried.
