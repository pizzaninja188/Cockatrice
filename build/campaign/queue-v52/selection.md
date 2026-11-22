# Queue v52 — post-v51 expansion

Source: pinned Commander-legal `oracle-cards.jsonl.gz` (SHA-256
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`) and pinned rulings
(SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`). Read-only join by
Oracle ID and printed name to the live registry, maps, `CARDS.md`, `queue-exclusions.txt` and
`blocked.tsv`; every implementation through `52a578954` is excluded.

## Revalidation

Exact Oracle IDs, costs, type lines, text, layout, Commander legality and presentation shape were
checked against the pinned bulk and stored in `revalidation.tsv` (5 admitted cards). None appears
in `blocked.tsv`, `queue-exclusions.txt` or the live registry. All are normal-layout, unique-name,
non-ambiguous, present in `cards-validation.xml` and unchecked in `CARDS.md` before selection.

## Selected family

- `01-destroy-artifact-land` — 5 cards: destroy exactly one artifact or land, with only supported
  riders: regeneration prevention (Pillage), controller loses 1 life (Victorious Destruction), and
  cycling {2} (Violent Impact).

## Delivery

Pending independent review, the batch gate, and push; see `report.md`.
