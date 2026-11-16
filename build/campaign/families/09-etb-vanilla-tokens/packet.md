# Packet: 09-etb-vanilla-tokens (12 cards)

Band: Parameter family, 12 cards.

Stable typed shape: Creature with one entry trigger creating one or more vanilla tokens.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 8b022754-6d16-470e-b754-4df6e4f4709e | Goblin Instigator | {1}{R} | Creature — Goblin Rogue | 1/1 | When this creature enters, create a 1/1 red Goblin creature token. |
| 79d99305-b3dd-4687-af69-bf4b28704aae | Goblin Gang Leader | {2}{R}{R} | Creature — Goblin Warrior | 2/2 | When this creature enters, create two 1/1 red Goblin creature tokens. |
| f5c5f64c-6911-430c-a825-b32b96d39c7d | Beetleback Chief | {2}{R}{R} | Creature — Goblin Warrior | 2/2 | When this creature enters, create two 1/1 red Goblin creature tokens. |
| 4045c24e-098f-4582-8c50-576a94c7b705 | Ambassador Oak | {3}{G} | Creature — Treefolk Warrior | 3/3 | When this creature enters, create a 1/1 green Elf Warrior creature token. |
| 3795fc31-2369-47d0-b9c9-120f44b8f114 | Elderleaf Mentor | {3}{G} | Creature — Elf Warrior | 3/2 | When this creature enters, create a 1/1 green Elf Warrior creature token. |
| 42a1954f-774e-4d80-98b3-a736f23e2060 | Yavimaya Sapherd | {2}{G} | Creature — Fungus | 2/2 | When this creature enters, create a 1/1 green Saproling creature token. |
| 193f9b12-ee34-40c2-9315-6794416a4f62 | Clarion Cathars | {3}{W} | Creature — Human Knight | 3/3 | When this creature enters, create a 1/1 white Human creature token. |
| 8239c63d-097a-449a-953a-cd480e594aa8 | Watchful Giant | {5}{W} | Creature — Giant Soldier | 3/6 | When this creature enters, create a 1/1 white Human creature token. |
| 42de2d46-c98d-4ae5-a53d-97531592529b | Daysquad Marshal | {3}{W} | Creature — Human Soldier | 3/3 | When this creature enters, create a 1/1 white Human Soldier creature token. |
| cf6bb391-4fde-43c4-a0c5-990630d0ddcf | Protector of Gondor | {3}{W} | Creature — Human Soldier | 3/3 | When this creature enters, create a 1/1 white Human Soldier creature token. |
| ad11ff81-acaa-4529-ba15-718dadbea259 | Aviation Pioneer | {2}{U} | Creature — Human Artificer | 1/2 | When this creature enters, create a 1/1 colorless Thopter artifact creature token with flying. |
| 710dc9b1-7d32-4330-95a8-3e91a668202a | Ghirapur Gearcrafter | {2}{R} | Creature — Human Artificer | 2/1 | When this creature enters, create a 1/1 colorless Thopter artifact creature token with flying. (A creature with flying can't be blocked except by creatures with flying or reach.) |

## Copy-from and differences

`elder_auntie` and `underfoot_underdogs` for `CreateTokens` on `WhenSelfEntersBattlefield`; token definitions are the shipped `goblin_r_1_1`, `elf_warrior_g_1_1`, `saproling_g_1_1`, `human_w_1_1`, `human_soldier_w_1_1` and `thopter_c_1_1_flying` templates.

Per-card differences: Token identity, count and body differ by card: red Goblin (one or two), green Elf Warrior, green Saproling, white Human, white Human Soldier, and colorless Thopter with flying. Every trigger is mandatory and controller-scoped.

## Rules evidence

CR 603.6a (entry trigger), 111.1 (tokens), 608.2c (the controller follows the instruction) and 704.5 (token state-based actions). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Goblin Instigator ({1}{R}, one 1/1 Goblin).
- Hardest: Aviation Pioneer ({2}{U}, flying Thopter artifact token).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
