# Queue v50 — post-v49 expansion

Source: pinned Commander-legal `oracle-cards.jsonl.gz` (SHA-256
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`) and pinned
`build/campaign/source/rulings.jsonl.gz` (SHA-256
`3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`). Read-only join by
Oracle ID and printed name to the live registry, maps, `CARDS.md`, `queue-exclusions.txt` and
`blocked.tsv`; every implementation on `card-campaign` through `37a72a0a5` is excluded.
Queue-v49 is exhausted; this queue is the next expansion.

## Revalidation

Candidates were grouped by stable typed shape rather than only normalized Oracle text. Exact
Oracle IDs, costs, type lines, power/toughness, text, layout, Commander legality and presentation
shape were checked against the pinned bulk and stored in `revalidation.tsv` (34 admitted cards).
Rulings were copied from the pinned dataset for every card (empty list where none exist).
Canonical RON ids use `slugify` semantics (apostrophes stripped, printed hyphens kept in the id;
face ids normalize punctuation to underscores). The `blocked.tsv` IDs were excluded before
selection.

## Selected families

- `01-put-on-top` — 6 cards: put one matching permanent on top of its owner's library
  (flashback retained on Grasp of Phantoms).
- `02-graveyard-to-hand` — 4 cards: return one filtered card from your graveyard to hand.
- `03-tap-stun-spells` — 4 cards: tap one creature, put the printed stun counters on it, then
  draw, scry, or nothing.
- `04-etb-shrink` — 4 cards: enters trigger with one fixed negative P/T modification on an
  opponent's creature.
- `05-etb-pump-target` — 3 cards: enters trigger with one fixed pump on a creature you control
  (Rage-Scarred Berserker also grants indestructible).
- `06-etb-counter-another` — 3 cards: enters trigger with one +1/+1 counter on another typed
  creature you control.
- `07-basic-land-to-hand` — 4 cards: optional private search for a basic land or printed land
  subtype into hand.
- `08-treasure-on-death` — 3 cards: dies or enters-and-dies trigger creating exactly one Treasure.
- `09-destroy-artifact-enchant` — 3 cards: fixed artifact/enchantment (and one flying-creature)
  destroy target union.

## Ejected or deferred from selection

- `Revive` and `Reborn Hope` were deferred before freezing: `ZoneCardFilter` has no color or
  multicolored predicate, so "target green card" and "target multicolored card" would need a new
  typed graveyard-card filter. The family remains a coherent four-card cohort without them; no
  partial card was admitted. Recorded in `blocked.tsv`.
- Existing blockers #515–#523 were not retried. No other selected card required a new semantic
  contract or had an ambiguous presentation source.

## Delivery

Pending batch gates and independent review; see `report.md`.
