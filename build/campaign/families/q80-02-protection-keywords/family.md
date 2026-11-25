# 02-protection-keywords — Multicolor creatures with printed color protection

- Queue: `build/campaign/queue-v80`.
- Cards: 5.
- Band: simple keyword cohort (3–10).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Each card is a complete static creature: printed power/toughness, zero or more
`keywords: [Keyword::...]`, and two or more `protections: [Color(...)]`. No triggered, activated or
static ability definition is needed. The type line and protection list are the whole mechanical card.

Closest shipped support: Duskrider Falcon, White Knight and the reviewed protection-cycle and
flying/protection cohorts. The protection quality vocabulary is `ProtectionQuality::Color`.
Iridescent Angel's one printed line is five separate color protections under the 2004-10-04 ruling.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Iridescent Angel | `28fa9211-96bb-4fef-81bf-8f58114a8df7` | `{5}{W}{U}` | Creature — Angel | Flying, protection from each color | `tricerules/tricerules-cards/data/iridescent_angel.ron` | `authoring/review-maps/iridescent_angel.json` |
| Oversoul of Dusk | `da790c72-63e7-4955-9435-d073767f9bb5` | `{G/W}{G/W}{G/W}{G/W}{G/W}` | Creature — Spirit Avatar | Protection from blue, from black, and from red | `.../oversoul_of_dusk.ron` | `.../oversoul_of_dusk.json` |
| Sabertooth Nishoba | `ce82aa79-bb50-47bc-84f5-eae30249c7a5` | `{4}{G}{W}` | Creature — Cat Beast Warrior | Trample, protection from blue and from red | `.../sabertooth_nishoba.ron` | `.../sabertooth_nishoba.json` |
| Sphinx of the Steel Wind | `1c96772b-463c-4655-836b-cf6e9dc6319f` | `{5}{W}{U}{B}` | Artifact Creature — Sphinx | Flying, first strike, vigilance, lifelink, protection from red and from green | `.../sphinx_of_the_steel_wind.ron` | `.../sphinx_of_the_steel_wind.json` |
| Akroma, Angel of Wrath | `107f204c-9fd3-490d-8887-be63d14fc6a4` | `{5}{W}{W}{W}` | Legendary Creature — Angel | Flying, first strike, vigilance, trample, haste, protection from black and from red | `.../akroma,_angel_of_wrath.ron` | `.../akroma,_angel_of_wrath.json` |

## Rulings

CR 702.9a-b (flying), 702.7 (first strike), 702.20 (vigilance), 702.15 (lifelink), 702.19
(trample), 702.10 (haste), and 702.16b-f (protection) govern. The pinned 2004-10-04 ruling confirms
Iridescent Angel has five separate color protections, so an effect that removes one color of
protection removes only that one. Akroma, Oversoul, Sabertooth and Sphinx have no pinned rulings.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q80_protection_keywords.rs` (registered in `tests/scenario.rs`).

- `q80_protection_canary_simplest_iridescent_angel_has_each_color_protection`
- `q80_protection_canary_hardest_akroma_keeps_keywords_and_selective_protection`
- `q80_protection_cards_all_cast_with_exact_printed_characteristics`
- `q80_protection_cards_reject_targeting_by_each_protected_color`
- `mechanic_registry::q80_protection_keywords::q80_protection_map_definitions`
- `mechanic_registry::q80_protection_keywords::q80_protection_fingerprints_match_the_pinned_oracle_text`

The exact-characteristics test casts every card with its printed mana and asserts its power,
toughness, keyword set and protection list. The targeting test rejects a white, blue, black, red and
green spell against each corresponding protection; Akroma's white `Swords to Plowshares` positive
control proves the protection is selective, not a blanket hexproof/shroud.

## Presentation

Each single printed line maps `/faces/0/keywords` and `/faces/0/protections`; primitive references
include each printed `Keyword::...` and `Protection::Color`. The metadata refresh added exactly five
fingerprint rows and five `CARDS.md` checkboxes.

## Focused evidence

- Intended red: `build/verification-logs/20261124-183504-817-Q80-protection-canary-intended-red.log` (MissingCard, exit 101).
- Simplest canary green: `build/verification-logs/20261124-183839-797-Q80-protection-simplest-canary-green.log`.
- Hardest canary green: `build/verification-logs/20261124-183919-552-Q80-protection-hardest-canary-green.log`.
- Family green: `build/verification-logs/20261124-195234-383-Q80-all-focused-scenarios-v3.log`.
- Registry green: `build/verification-logs/20261124-195314-510-Q80-all-registry-tests-v4.log`.
- Formatting: `cargo fmt --all -- --check` exit 0.
- Map preparation: `build/verification-logs/20261124-195330-705-Q80-card-evidence-preparation-v3.log` exit 0.

## Review and gate

Independent frozen review: APPROVED (reviewers read `build/campaign/review/q80-batch-a/product-v2.patch`,
SHA-256 `7261e70808c75db607212d218be6b212b6853c89a110aba108b7064ce957d07d`; sliver and kicker approved
on v1 and carried forward, protection delta-approved on v2). Batch queue-v80 batch A full gate `verify-7230649c-8d9f-4a9c-8007-2f6539e126dd` exit 0 on the final product bytes; the batch
covers 01-sliver-pump-grant..03-kicker-spells (12 cards). No new primitive, shared-test change or
typed contract.
