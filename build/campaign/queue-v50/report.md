# Queue v50 report

## Result

Nine families, 34 cards, independently reviewed, gated, committed and pushed to
`origin/card-campaign`. Family commits `8a1a0054c`..`882b764d6`; shared evidence commits
`99a3b0991`, `c664a8f83`, `628b74170`, `0c2a0ed5e`; remote SHA
`0c2a0ed5ea6fee54a2760f1fe3cfcd061586ed82`.

| Family | Cards | Commit | Bundle |
|---|---:|---|---|
| `01-put-on-top` | 6 | `8a1a0054c` | A |
| `02-graveyard-to-hand` | 4 | `db143f0a0` | A |
| `03-tap-stun-spells` | 4 | `d0ebf0420` | A |
| `04-etb-shrink` | 4 | `d8dca4c9b` | B |
| `05-etb-pump-target` | 3 | `933a9f526` | B |
| `06-etb-counter-another` | 3 | `0608ab3c0` | B |
| `07-basic-land-to-hand` | 4 | `5be57344f` | C |
| `08-treasure-on-death` | 3 | `a1d883392` | C |
| `09-destroy-artifact-enchant` | 3 | `882b764d6` | C |

The consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed
exit 0 (`Side Rust`, `-CardData`). The worktree was clean before the gate; no unrelated changes
were staged.

## Evidence

- Intended red: two canary tests per family held out of the registry, all exit 101 with
  `MissingCard` (`build/verification-logs/*q50-intended-red-*.log`).
- Focused green: 41/41 scenario and 10/10 registry tests, exit 0
  (`20261120-163633-114-q50-scenario-green-v4.log`,
  `20261120-163633-113-q50-registry-green-v4.log`).
- Review maps: 34 selected maps passed Preparation evidence and executable-reference checks
  (`card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3`, exit 0).
- Metadata: one refresh; 34 fingerprint rows and 34 checklist flips changed, no unrelated drift.
- Independent review: three hash-verified frozen bundles approved after material/delta findings
  (graveyard filter negatives, Boggart noncreature Goblin, disjunctive put-top branches, family-06
  scope and `another`, Cobra death, Return-to-the-Earth artifact branch, stale-target loop,
  nonbasic search negative, map citations, CR citations). Prior bundles are preserved under
  `freeze/v2`, `freeze/v3`, `freeze/v4`; current bundles are `freeze/batch-a.json` etc.
- Presentation: a mechanical exception row was added for the four optional search branches
  (`presentation-exceptions.tsv`), matching the existing `sylvan_ranger` convention.

## Queue state

`build/campaign/queue-v50/` holds the selection, revalidation, per-family source/rulings and
records, maps, frozen bundles and this report. Queue-v50 is exhausted.

## Ejected or deferred

- `Revive` (`51991d28-...`) and `Reborn Hope` (`683d8deb-...`) were deferred: `ZoneCardFilter`
  has no color or multicolored predicate. Filed issue #524 and recorded both rows in
  `blocked.tsv`; the remaining four-card graveyard family was admitted whole.
- Existing blockers #515–#523 were not retried.

## Environment notes

- `cargo clean --profile dev` freed 30.2 GiB after the first gate stop reported 10.0 GiB
  available against a 10.0 GiB requirement.
- The full gate's Clippy step required two test-only `vec!` -> array fixes; no production change.
- The conformance baseline needed the three unsupported graveyard-target fixture classifications;
  they are target-group fixture limitations, not card semantics.
