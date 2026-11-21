# Handoff after queue-v51 (draft)

## Delivered

Pending independent review, batch gates, commits and push.

| Family | Cards | Gate |
|---|---:|---|
| `01-token-spells` | 11 | pending |
| `02-splicer-cycle` | 5 | pending |
| `03-artifact-value-tutors` | 4 | pending |
| `04-subtype-harbingers` | 6 | pending |
| `05-aura-restrictions` | 7 | pending |
| `06-etb-each-discard` | 3 | pending |
| `07-blocked-pumps` | 4 | pending |
| `08-attack-team-pumps` | 6 | pending |
| `09-spirit-arcane-payoffs` | 5 | pending |

## Queue state

`build/campaign/queue-v51/` holds the selection, revalidation, 51 source/rulings packets, 51 review
maps, nine family records, frozen bundles and the report. Queue-v51 will be exhausted after this
batch.

## Notes

- `06-blocking-pumps` was ejected before freezing: `CreatureScopeFilter` has no blocking combat-role
  predicate. `06-etb-each-discard` replaced it.
- Existing blockers #515–#524 were not retried.
