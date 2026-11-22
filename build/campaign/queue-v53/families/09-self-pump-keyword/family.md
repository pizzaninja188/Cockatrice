# 09-self-pump-keyword

Shape: A creature with one supported keyword or landwalk evasion and one mana-only activated ability that pumps itself until end of turn.

## Source and identity

| Card | Oracle ID | Cost | Type | P/T | Static | Oracle text |
|---|---|---|---|---|---|---|
| Whispering Shade | `0036d062-10dc-4267-98b4-1d2f6b190a61` | {3}{B} | Creature — Shade | 1/1 | Swampwalk | {B}: This creature gets +1/+1 until end of turn. |
| Dirtwater Wraith | `0126f620-1dbf-405c-b5cf-f48b7ef02d32` | {3}{B} | Creature — Wraith | 1/3 | Swampwalk | {B}: This creature gets +1/+0 until end of turn. |
| Colos Yearling | `50cf702b-5647-415e-b9a8-56fa0e50da6d` | {2}{R} | Creature — Goat Beast | 1/1 | Mountainwalk | {R}: This creature gets +1/+0 until end of turn. |
| Sandstone Warrior | `120a0bcd-eca2-44eb-ad8d-5e4077f7e21c` | {2}{R}{R} | Creature — Human Soldier Warrior | 1/3 | First strike | {R}: This creature gets +1/+0 until end of turn. |
| Loxodon Stalwart | `6b2eb774-08f3-46cb-aeca-2ab82fdd5f03` | {3}{W}{W} | Creature — Elephant Soldier | 3/3 | Vigilance | {W}: This creature gets +0/+1 until end of turn. |
| Steam Spitter | `76ed34d2-e09e-4b53-98bd-d96b367fd315` | {4}{G} | Creature — Spider | 1/5 | Reach | {R}: This creature gets +1/+0 until end of turn. |

Exact records and pinned rulings: `build/campaign/queue-v53/families/09-self-pump-keyword/`.

## Reuse

`wyluli_wolf` for a mana-only self-pump; `bog_wraith`/`anaconda` for landwalk; keyword semantics unchanged. Rules: CR 602.2, 611.2c/613.4c and the printed keyword's CR section.

## Evidence

- Intended red/green: pending batch C execution.
- Maps/tests: pending promotion.

## Delivery

- Independent read-only review: pending.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
