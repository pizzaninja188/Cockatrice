# Queue v51 report

## Result

Eight families, 47 cards, independently reviewed; batch gates, commits and push pending.

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

## Queue state

`build/campaign/queue-v51/` holds the selection, revalidation, per-family source/rulings and
records, maps, frozen bundles and this report.

## Ejected or deferred

- `06-blocking-pumps` was replaced by `06-etb-each-discard` because the mass pump vocabulary has
  no blocking combat-role predicate.
- `07-blocked-pumps` was ejected after review: "becomes blocked" has no once-per-combat trigger,
  so all four IDs are blocked under #518. The delivered queue therefore has eight families.
- Existing blockers #515–#524 were not retried.
