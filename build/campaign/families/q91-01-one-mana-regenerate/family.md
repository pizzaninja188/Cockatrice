# 01-one-mana-regenerate — four "{one colored mana}: Regenerate this creature" bodies

- Queue: `build/campaign/queue-v91`.
- Cards: 4.
- Band: multi-clause composition (4).
- Status: **verified**. Independent read-only review APPROVED (bundle rev 4, manifest
  `cb2fc8c79ae4d9adc97dc7d63df9f6e10fa97af77fd75d62a5080380b944b1ec`); batch gate
  `verify-647f2fb9-0ebe-4355-9459-6653327b653a` exit 0 covering all three queue-v91 families.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints one static keyword/evasion/protection line plus one
`{one colored mana}: Regenerate this creature.` line; Kathari Remnant additionally prints the
cascade spell ability on a third line. The activation is the
shipped `costs: [Mana("<printed symbol>")]` with `effect: [Regenerate(subject: Source)]` — the same
shape **Cudgel Troll**, **Ancient Silverback**, **Gorilla Chieftain**, **Asphodel Wanderer** and
**Lim-Dûl's High Guard** ship, and the shared `regenerate_activated_batch` /
`q42_regenerate_self_batch` scenarios already exercise end to end.

What this cohort adds is the *single colored symbol* activation cost across four different static
lines, including two non-keyword static shapes (protection and landwalk) that already ship on
non-regenerating bodies. **River Boa** (`{1}{G}` 2/1 Snake, Islandwalk plus `{G}: Regenerate`) is
Mire Boa's structural twin and the closest analogue for the evasion member; **Cudgel Troll** covers
the keyword member:

| Card | Oracle ID | Card cost | Activation | Static line | Body |
|---|---|---|---|---|---|
| Charging Troll | `bbc1b96d-5a63-4091-a2c7-08bd289e0bf2` | `{2}{G}{W}` | `{G}` | Vigilance | 3/3 |
| Spectral Lynx | `b5187725-9a39-433c-ae09-eccd6111574c` | `{1}{W}` | `{B}` | Protection from green | 2/1 |
| Kathari Remnant | `bbf07946-33d0-48fe-85af-022e9b0b834c` | `{2}{U}{B}` | `{B}` | Flying, cascade | 0/1 |
| Mire Boa | `e9a3a466-df78-49a0-add3-975b94c5cd09` | `{1}{G}` | `{G}` | Swampwalk | 2/1 |

Spectral Lynx and Mire Boa print no `keywords:` entry at all: their static lines are
`protections: [Color(Green)]` and `evasions: [Landwalk(land_subtype: "Swamp")]`, which are the
shipped Abbey Gargoyles and Anaconda shapes. Kathari Remnant's cascade is a **spell** keyword even
on a creature body (CR 702.85a), so it is `spell_keywords: [Cascade]` exactly as Shardless Agent and
Enlisted Wurm ship it.

Kathari Remnant's colour identity is `{U}{B}`, but its regenerate activation is `{B}`; mixing those
up is the copy risk this card tests, so the registry test asserts a per-card expected symbol rather
than one shared literal.

## Exact Oracle text (pinned bulk)

- Charging Troll — `Vigilance` / `{G}: Regenerate this creature.`
- Spectral Lynx — `Protection from green` / `{B}: Regenerate this creature.`
- Kathari Remnant — `Flying` / `{B}: Regenerate this creature.` / `Cascade`
- Mire Boa — `Swampwalk` / `{G}: Regenerate this creature.`

All four are Commander-legal, normal layout, single-face, uniquely named in both pins, present in
`build/campaign/source/cards-validation.xml`, absent from the live registry and review maps,
unchecked in `CARDS.md` at admission (the refresh then flips all four to `[x]`), and absent from
`blocked.tsv` and `queue-exclusions.txt`. See `build/campaign/queue-v91/revalidation.tsv`.

## Pinned rulings

- Spectral Lynx (2022-12-08, three rulings): the ability may be activated with nothing to save; the
  shield replaces destruction or lethal damage once that turn. The shield half is asserted
  directly; the "nothing to save" half is the same code path (the activation has no precondition),
  and the scenario asserts activation with zero damage and no combat.
- Kathari Remnant (2021-06-18, nine rulings): cascade exiles cards face up through the first cheaper
  nonland, `{X}` is 0 when cast without paying, the exiled cards go to the bottom in a random order,
  and a countered cascade spell still resolves its cascade. The scenario asserts the exile-through
  and the bottoming.
- Charging Troll and Mire Boa have no pinned ruling. Mire Boa's swampwalk is asserted both
  negatively (with the `evasion` discriminant) and positively (the same block legal without a
  Swamp).

## Copy-from and differences

Copy `cudgel_troll` / `gorilla_chieftain` for the regeneration ability, `abbey_gargoyles` for
`protections: [Color(..)]`, `anaconda` for `evasions: [Landwalk(..)]`, and `shardless_agent` for
`spell_keywords: [Cascade]`. Differences: the single-symbol activation cost (the analogues use
`{G}`, `{1}{G}` or `{2}{G}`), and four different static lines on four bodies.

## Rules evidence

CR 701.19a-b (regeneration shield and the replacement of lethal damage or destruction), CR 602.1/
602.2 (activation and printed cost), CR 702.14a-c (swampwalk), CR 702.16b-f (protection from a
colour), CR 702.20 (vigilance), CR 702.9a-b (flying), CR 702.85a-c (cascade), CR 514.2 (a
"this turn" shield expires at cleanup) and CR 608.2b (a stale object is not affected).

## Semantic evidence

- `scenario q91_batch_a::q91_one_mana_regenerate_canary_applies_the_printed_shield`
- `scenario q91_batch_a::q91_kathari_remnant_canary_keeps_flying_and_the_shield`
- `scenario q91_batch_a::q91_one_mana_regenerate_creatures_consume_the_shield_and_keep_their_keyword`
- `scenario q91_batch_a::q91_one_mana_regenerate_requires_the_printed_colored_symbol`
- `scenario q91_batch_a::q91_one_mana_regenerate_shield_expires_at_cleanup`
- `scenario q91_batch_a::q91_kathari_remnant_cascades_into_a_cheaper_spell`
- `scenario q91_batch_a::q91_spectral_lynx_has_protection_from_green`
- `scenario q91_batch_a::q91_mire_boa_has_swampwalk`
- `scenario q91_batch_a::q91_mire_boa_can_be_blocked_without_a_swamp`
- `mechanic_registry::q91_batch_a::q91_one_mana_regenerate_maps_definitions`
- `mechanic_registry::q91_batch_a::q91_one_mana_regenerate_cards_keep_their_printed_static_line`
- `mechanic_registry::q91_batch_a::q91_batch_fingerprints_match_the_pinned_oracle_text`

## Canaries

- Simplest: Mire Boa (`{G}` activation on a plain 2/1 with swampwalk), proved red then green. Its
  swampwalk is asserted both negatively (block refused, with an `evasion` discriminant) and
  positively (same block legal with no Swamp).
- Hardest: Kathari Remnant (`{B}` activation, a 0/1 body, flying **and** cascade, so a dropped
  spell keyword or an `{U}{B}`-shaped activation both fail).

## Focused evidence

- Intended canary red (RONs held out): `build/campaign/queue-v91/logs/red-q91-scenario.log`
  — `MissingCard("mire_boa")`, 0 passed / 1 failed.
- Focused canary green: `build/campaign/queue-v91/logs/green-q91-canary.log` — 1 passed.
- Focused scenario green (the whole `q91_batch_a` module, 18 tests; 9 of them are this family, plus the three registry checks below):
  `build/campaign/queue-v91/logs/green-q91-scenario.log`.
- Focused registry green (6 tests): `build/campaign/queue-v91/logs/green-q91-registry.log`.
- Non-vacuity probes, each restored and re-verified green:
  - `build/campaign/queue-v91/logs/probe-fingerprint.log` — mutating one pinned fingerprint to zeros
    fails the fingerprint test with "fingerprint drift for charging_troll".
  - `build/campaign/queue-v91/logs/probe-protection.log` — deleting `protections: [Color(Green)]`
    from Spectral Lynx makes the protection scenario fail (it previously passed on a priority
    rejection; the test now casts as the active player and pairs the refusal with a legal control).

## Delivery

- Batch gate `verify-647f2fb9-0ebe-4355-9459-6653327b653a` Pass, exit 0 (batch queue-v91 A, covering 01-one-mana-regenerate,
  02-two-mana-regenerate and 03-basic-land-pair-search, 10 cards).
- Product commit `6a6cebd59`; remote `6a6cebd59496d7307344f22f2a25297e96d86070` on
  `origin/card-campaign`, verified by `git fetch` + `git rev-parse`.
- No MTG rules surface area beyond the per-card CR citations above; no engine, protobuf, relay
  or client contract changed, so those surfaces are N/A.

