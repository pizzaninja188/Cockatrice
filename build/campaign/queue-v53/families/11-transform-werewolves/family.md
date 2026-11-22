# 11-transform-werewolves

Shape: Transforming double-faced Werewolves with the classic upkeep transform trigger. The front face transforms when no spells were cast last turn; the back face transforms when a player cast two or more spells last turn.

## Source and identity

| Card | Oracle ID | Front | Back | Front/Back P/T |
|---|---|---|---|---|
| Convicted Killer // Branded Howler | `c1801b5c-1a87-46d0-83d3-e8dfd94211f4` | Convicted Killer | Branded Howler | 2/2 → 4/4 |
| Grizzled Outcasts // Krallenhorde Wantons | `abcfcc71-77c0-46e7-93c3-726967946b12` | Grizzled Outcasts | Krallenhorde Wantons | 4/4 → 7/7 |
| Solitary Hunter // One of the Pack | `c99834dc-755f-407c-ad0e-550f8ae12198` | Solitary Hunter | One of the Pack | 3/4 → 5/6 |
| Tormented Pariah // Rampaging Werewolf | `bbcc2d2d-b29c-410f-bee7-5ef1609a5b32` | Tormented Pariah | Rampaging Werewolf | 3/2 → 6/4 |
| Villagers of Estwald // Howlpack of Estwald | `dcf692f6-baec-4b60-a6e3-2a8b731d7816` | Villagers of Estwald | Howlpack of Estwald | 2/3 → 4/6 |

Exact records and pinned rulings: `build/campaign/queue-v53/families/11-transform-werewolves/`.

## Reuse

`reckless_waif_merciless_predator` for the exact two-face `ChangeSourceFace` triggers and `SpellsCastLastTurn` conditions. Rules: CR 701.27/712 and 603.6a.

## Evidence

- Intended red/green: pending batch D execution.
- Maps/tests: pending promotion.

## Delivery

- Blocked by #525: the back face needs a per-player previous-turn spell count. `SpellsCastLastTurn(players: All)` sums all players, so 1+1 incorrectly triggers. Engine changes are out of scope; no partial admission.
- Frozen batch D review requested this change; the five Oracle IDs are recorded in `build/campaign/blocked.tsv` under #525.
- Acceptance: not delivered.
