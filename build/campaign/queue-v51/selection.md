# Queue v51 — post-v50 expansion

Source: pinned Commander-legal `oracle-cards.jsonl.gz` (SHA-256
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`) and pinned
`build/campaign/source/rulings.jsonl.gz` (SHA-256
`3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`). Read-only join by
Oracle ID and printed name to the live registry, maps, `CARDS.md`, `queue-exclusions.txt` and
`blocked.tsv`; every implementation on `card-campaign` through `9a37f656b` is excluded.
Queue-v50 is exhausted; this queue is the next expansion.

## Revalidation

Candidates were grouped by stable typed shape rather than only normalized Oracle text. Exact
Oracle IDs, costs, type lines, power/toughness, text, layout, Commander legality and presentation
shape were checked against the pinned bulk and stored in `revalidation.tsv` (51 candidates; 47 admitted after the family-07 ejection below).
Rulings were copied from the pinned dataset for every card (empty list where none exist).
Canonical RON ids use `slugify` semantics (apostrophes stripped, printed hyphens kept in the id;
face ids normalize punctuation to underscores). The `blocked.tsv` IDs were excluded before
selection. Every card is in the validated `cards-validation.xml` corpus and an unchecked
`CARDS.md` row at selection time.

## Selected families

- `01-token-spells` — 11 cards: one-shot spell creating a fixed token or fixed token cohort.
- `02-splicer-cycle` — 5 cards: enters trigger creating one 3/3 Golem plus a Golem lord or regenerate.
- `03-artifact-value-tutors` — 4 cards: optional private artifact search bounded to a mana-value band.
- `04-subtype-harbingers` — 6 cards: optional private subtype search put on top of the library.
- `05-aura-restrictions` — 7 cards: attached combat/untap restriction profiles.
- `06-etb-each-discard` — 3 cards: enters trigger making each player discard one card.
- `08-attack-team-pumps` — 6 cards: attack-trigger mass pump or keyword grant.
- `09-spirit-arcane-payoffs` — 5 cards: Spirit or Arcane cast trigger with one targeted effect.

## Ejected or deferred from selection

- `07-blocked-pumps` (`Elvish Berserker`, `Sparring Golem`, `Gang of Elk`, `Rabid Elephant`) was
  ejected after review: their Oracle text says "Whenever this creature becomes blocked" (one
  occurrence per combat) with a resolution-time blocker count, while the engine only exposes
  `WheneverSelfBecomesBlockedByCreature`. That is the existing #518 capability gap, so all four
  IDs are recorded in `blocked.tsv` under #518 and no partial family was admitted.
- `06-blocking-pumps` (`Piety`, `Rally`, `Hold the Line`) was considered and ejected before freezing:
  `PumpAll`/`GrantKeywordsAll` use `CreatureScopeFilter`, which has no blocking combat-role
  predicate, so "Blocking creatures get +N/+N" is not expressible without an engine change. The
  family was replaced by `06-etb-each-discard`; no partial card was admitted.
- Existing blockers #515–#524 were not retried. No other selected card required a new semantic
  contract or had an ambiguous presentation source.

## Delivery

Pending batch gates and independent review; see `report.md`.
