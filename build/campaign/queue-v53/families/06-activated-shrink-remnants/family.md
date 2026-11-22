# 06-activated-shrink-remnants

Shape: Existing-cost activated abilities that give one target creature a fixed negative power/toughness until end of turn. Costs include tap, mana, discard, self-sacrifice and sacrificing a Food or creature; the shared typed effect is `PumpTarget` with a chosen creature.

## Source and identity

| Card | Oracle ID | Cost | Mana | Type | Oracle text |
|---|---|---|---|---|---|
| Aquus Steed | `3517abe3-eb18-49af-9f1c-b372d2039d99` | {2}{U}, {T} | {3}{U} | Creature — Beast | Target creature gets -2/-0 until end of turn. |
| Grandmother Sengir | `4f92804f-a231-4406-9a16-f4e102517b06` | {1}{B}, {T} | {4}{B} | Legendary Creature — Human Wizard | Target creature gets -1/-1 until end of turn. |
| Plague Witch | `532ab335-4873-405b-84b3-c447207172b5` | {B}, {T}, Discard a card | {1}{B} | Creature — Elf Spellshaper | Target creature gets -1/-1 until end of turn. |
| Phyrexian Debaser | `554de904-086c-428c-9f32-b0b2498a6fd5` | {T}, Sacrifice this creature | {3}{B} | Creature — Phyrexian Carrier | Target creature gets -2/-2 until end of turn. |
| Nim Replica | `68c3aa66-85f2-494c-8fb2-ad0348814506` | {2}{B}, Sacrifice this creature | {3} | Artifact Creature — Zombie | Target creature gets -1/-1 until end of turn. |
| Bog Naughty | `74303ddb-d3b6-433d-a299-e0c4a7267461` | {2}{B}, Sacrifice a Food | {3}{B}{B} | Creature — Faerie | Target creature gets -3/-3 until end of turn. |
| Plagued Rusalka | `d6912f52-e478-4cc6-a34c-67e372abc4fa` | {B}, Sacrifice a creature | {B} | Creature — Spirit | Target creature gets -1/-1 until end of turn. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`blister_beetle`/`hornet_harasser` for negative `PumpTarget`; `plague_witch` uses the existing `Discard` cost, `sweettooth_witch` the Food sacrifice cost, and `acolyte_of_aclazotz` the creature sacrifice cost. Rules: CR 602.2, 118.3, 701.9, 701.21a and 611.2c/613.4c.

## Evidence

- Intended red: `build/campaign/queue-v53/red-b.log` (0/5 exit 101, `MissingCard`).
- Focused scenario green: `build/campaign/queue-v53/green-b6.log` (8/8 exit 0).
- Focused registry green: `build/campaign/queue-v53/registry-b5.log` (3/3 exit 0).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json`.
- Fixture-gap baseline row: `tricerules/tricerules-core/tests/conformance/baseline.tsv` (`bog_naughty` ability 0).

## Delivery

- Independent read-only review: approved after three frozen delta reviews; final bundle `build/campaign/queue-v53/freeze/batch-b-final3.json` (`patch_sha256 29516569…`).
- Batch gate: `build/verification-logs/verify-a2adabfc-b00b-4745-a802-323c68ac7e29/summary.json` passed exit 0 (`Side Rust`, `-CardData -CardsXml build/campaign/source/cards-validation.xml`) covering 06-activated-shrink-remnants..07-two-target-damage.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
