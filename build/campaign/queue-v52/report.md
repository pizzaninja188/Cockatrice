# Queue v52 report

## Result

One family, 5 cards, independently reviewed, gated, committed and pushed to
`origin/card-campaign`. Family commit `defdacc6b`; shared evidence `5c50e54fe`; baseline
`fde229a9b`; gated remote SHA `fde229a9b`.

Batch D (single remaining family) full gate `verify-06a92503-bedd-4c7a-bb41-b45c68878ea2`
passed exit 0 (`Side Rust`, `-CardData`).

## Evidence

- Historical red: `build/verification-logs/20261121-q52-intended-red.log` (6/6 scenario tests exit
  101 with `MissingCard` while the five definitions are held out).
- Focused green: `build/verification-logs/20261121-q52-scenario-green.log` (6/6) and
  `build/verification-logs/20261121-q52-registry-green.log` (2/2), both exit 0.
- Review maps: 5 selected maps passed Preparation evidence and executable-reference checks
  (`build/verification-logs/card-evidence-d112cdb7-eb59-4b4a-9f95-7a5a09753d08`, exit 0).
- Presentation audit: problems 0 (`build/verification-logs/20261120-231452-958-Offline-card-author.log`).
- Metadata: one refresh `card-data-9743fa9f-0d12-414d-b7c1-76ea359d8033`; five fingerprint rows
  and five checklist flips changed, no unrelated drift.
- Final conformance check confirmed the `violent_impact` hand-zone cycling fixture classification
  before the passing gate.
