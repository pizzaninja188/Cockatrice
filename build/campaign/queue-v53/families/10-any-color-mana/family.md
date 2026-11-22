# 10-any-color-mana

Shape: A permanent with one printed keyword or no extra text and an untapped mana ability producing one mana of any color.

## Source and identity

| Card | Oracle ID | Cost | Type | P/T | Static | Oracle text |
|---|---|---|---|---|---|---|
| Deathbloom Gardener | `0c7f9701-c39e-4f14-844a-15f2e3c41ca9` | {2}{G} | Creature — Elf Druid | 1/1 | Deathtouch | {T}: Add one mana of any color. |
| Sylvan Caryatid | `13d4c46b-c2d6-44cc-a252-4a991d471854` | {1}{G} | Creature — Plant | 0/3 | Defender, hexproof | {T}: Add one mana of any color. |
| Leyline Prowler | `8ca3852a-fa37-46ef-a74d-3c7487ae50fc` | {1}{B}{G} | Creature — Nightmare Beast | 2/3 | Deathtouch, lifelink | {T}: Add one mana of any color. |
| Darksteel Ingot | `a2529491-7389-4cfa-92d2-145eda779603` | {3} | Artifact | — | Indestructible | {T}: Add one mana of any color. |

Exact records and pinned rulings: `build/campaign/queue-v53/families/10-any-color-mana/`.

## Reuse

`sylvok_explorer`/`utopia_tree` for any-color mana; keyword semantics unchanged. Rules: CR 605.1a/605.3b, 106.1 and the printed keyword's CR section.

## Evidence

- Intended red/green: pending batch C execution.
- Maps/tests: pending promotion.

## Delivery

- Independent read-only review: pending.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
