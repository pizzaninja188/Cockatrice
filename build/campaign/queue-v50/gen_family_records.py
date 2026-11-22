#!/usr/bin/env python3
"""Generate queue-v50 family records from the pinned revalidation data."""
from __future__ import annotations

from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
OUT = ROOT / "build" / "campaign" / "queue-v50"
RULINGS = OUT / "families"

SHAPES = {
    "01-put-on-top": "Single placement spell: put one matching permanent on top of its owner's library.",
    "02-graveyard-to-hand": "Single spell effect: return one filtered card from your graveyard to your hand.",
    "03-tap-stun-spells": "Single tap-and-stun spell: tap one creature, put the printed stun counters on it, then draw, scry, or nothing.",
    "04-etb-shrink": "Enters trigger with one fixed negative P/T modification on an opponent's creature.",
    "05-etb-pump-target": "Enters trigger with one fixed pump (and, for one card, indestructible) on a creature you control.",
    "06-etb-counter-another": "Enters trigger with one +1/+1 counter on another typed creature you control.",
    "07-basic-land-to-hand": "Enters trigger with an optional private search for a basic land or printed land subtype into hand.",
    "08-treasure-on-death": "Dies or enters-and-dies trigger creating exactly one Treasure token.",
    "09-destroy-artifact-enchant": "Single destroy spell with a fixed artifact/enchantment (and one flying-creature) target union.",
}

REUSE = {
    "01-put-on-top": "`time_ebb`, `griptide`, `excommunicate` (same `PutInOwnersLibrary(placement: Top)` program); `damage_flashback_batch` for the flashback alternative cost.",
    "02-graveyard-to-hand": "`sages_knowledge`, `recollect`, `disentomb` (same `MoveGraveyardCards(destination: Hand)` shape) plus `badlands_revival` for the permanent-card disjunction.",
    "03-tap-stun-spells": "`alchemax_slayer-bots` / `cryogen_relic` for tap plus stun counters, and `crippling_chill` / `aggressive_urge` for a targeted spell with a non-targeted follow-up.",
    "04-etb-shrink": "`blister_beetle`, `keening_banshee`, `brinebarrow_intruder`-family tests (same `WhenSelfEntersBattlefield` + negative `PumpTarget`).",
    "05-etb-pump-target": "`guardian_of_pilgrims` / `rubblebelt_boar` for entry pump, and `etb_target_pump_batch` for expiry and illegal-target coverage.",
    "06-etb-counter-another": "`kujar_seedsculptor` / `q43_etb_counter_target_batch` for entry counter placement; the printed subtype and `another` exclusion are the only differences.",
    "07-basic-land-to-hand": "`sylvan_ranger` / `borderland_ranger` for the optional private `Choose` + `SearchLibrary` program.",
    "08-treasure-on-death": "`piggy_bank` for the dies Treasure trigger and `brazen_freebooter_batch` for the canonical `treasure` token; `ichor_wellspring` for the combined entry/dies trigger.",
    "09-destroy-artifact-enchant": "`broken_wings` for the three-branch target union and `destroy_artifact_batch` for destruction, ownership and stale-target coverage.",
}

ASSERTIONS = {
    "01-put-on-top": [
        "q50_put_top_canary_simplest_repel_puts_the_target_on_top",
        "q50_put_top_canary_hardest_banishment_decree_accepts_three_card_types",
        "q50_put_top_cards_put_only_their_matching_permanents_on_top",
        "q50_put_top_cards_accept_each_disjunctive_branch",
        "q50_put_top_cards_fizzle_when_the_target_leaves",
        "q50_put_top_grasp_of_phantoms_flashes_back_and_exiles",
    ],
    "02-graveyard-to-hand": [
        "q50_graveyard_to_hand_canary_simplest_boggart_birth_rite_returns_a_goblin",
        "q50_graveyard_to_hand_canary_hardest_natures_spiral_returns_a_permanent_card",
        "q50_graveyard_to_hand_cards_return_only_their_filtered_cards",
        "q50_graveyard_to_hand_cards_reject_nonmatching_cards",
        "q50_graveyard_to_hand_boggart_returns_a_noncreature_goblin_card",
        "q50_graveyard_to_hand_cards_reject_opponent_graveyards_and_fizzle_stale_targets",
    ],
    "03-tap-stun-spells": [
        "q50_tap_stun_canary_simplest_cryogenic_stasis_taps_and_draws",
        "q50_tap_stun_canary_hardest_freeze_in_place_scries_two",
        "q50_tap_stun_cards_tap_and_place_their_printed_counters",
        "q50_tap_stun_cards_still_place_counters_on_an_already_tapped_creature",
        "q50_tap_stun_cards_replace_untaps_with_counter_removal",
        "q50_tap_stun_cards_scope_opponent_only_targets_and_reject_noncreatures",
        "q50_tap_stun_freeze_in_place_fizzles_without_scrying_when_the_target_leaves",
    ],
    "04-etb-shrink": [
        "q50_etb_shrink_canary_simplest_eyeblight_assassin_shrinks",
        "q50_etb_shrink_canary_hardest_black_dragon_flying_shrinks_three",
        "q50_etb_shrink_cards_target_only_opposing_creatures_and_expire",
        "q50_etb_triggered_cards_fizzle_stale_targets",
    ],
    "05-etb-pump-target": [
        "q50_etb_pump_canary_simplest_haazda_officer_pumps",
        "q50_etb_pump_canary_hardest_rage_scarred_berserker_grants_indestructible",
        "q50_etb_pump_cards_target_only_your_creatures_and_expire",
        "q50_etb_triggered_cards_fizzle_stale_targets",
    ],
    "06-etb-counter-another": [
        "q50_etb_counter_canary_simplest_jade_bearer_adds_a_counter",
        "q50_etb_counter_canary_hardest_stromkirk_mentor_excludes_source",
        "q50_etb_counter_cards_require_another_typed_creature_you_control",
        "q50_etb_triggered_cards_fizzle_stale_targets",
    ],
    "07-basic-land-to-hand": [
        "q50_basic_land_search_canary_simplest_farfinder_finds_a_basic_land",
        "q50_basic_land_search_canary_hardest_district_guide_finds_a_gate",
        "q50_basic_land_search_cards_offer_only_their_printed_filters",
        "q50_basic_land_search_cards_may_decline_the_search",
    ],
    "08-treasure-on-death": [
        "q50_treasure_canary_simplest_dire_fleet_hoarder_makes_a_treasure_on_death",
        "q50_treasure_canary_hardest_patron_of_the_arts_makes_one_on_entry_and_one_on_death",
        "q50_treasure_jewel_eyed_cobra_makes_a_treasure_on_death",
        "q50_treasure_cards_credit_the_creatures_controller_and_do_not_trigger_on_exile",
    ],
    "09-destroy-artifact-enchant": [
        "q50_destroy_canary_simplest_naturalize_destroys_an_artifact",
        "q50_destroy_canary_hardest_return_to_the_earth_destroys_a_flying_creature",
        "q50_destroy_cards_accept_only_their_printed_target_types",
        "q50_destroy_cards_fizzle_when_the_target_already_left",
    ],
}

REGISTRY = {
    "01-put-on-top": "q50_put_top_maps_definitions",
    "02-graveyard-to-hand": "q50_graveyard_to_hand_maps_definitions",
    "03-tap-stun-spells": "q50_tap_stun_maps_definitions",
    "04-etb-shrink": "q50_etb_shrink_maps_definitions",
    "05-etb-pump-target": "q50_etb_pump_maps_definitions",
    "06-etb-counter-another": "q50_etb_counter_maps_definitions",
    "07-basic-land-to-hand": "q50_basic_land_maps_definitions",
    "08-treasure-on-death": "q50_treasure_maps_definitions",
    "09-destroy-artifact-enchant": "q50_destroy_maps_definitions",
}

records = {}
for line in (OUT / "revalidation.tsv").read_text(encoding="utf-8").splitlines()[1:]:
    family, oid, name, mana, type_line, power, toughness, layout, commander, text, rulings = line.split("\t")
    records.setdefault(family, []).append((oid, name, mana, type_line, power, toughness, text, rulings))

for family, cards in records.items():
    family_dir = RULINGS / family
    lines = [f"# {family}", "", f"Shape: {SHAPES[family]}", "", "## Source and identity", "", "| Card | Oracle ID | Cost | Type | Body | Oracle text |", "|---|---|---|---|---|---|"]
    for oid, name, mana, type_line, power, toughness, text, rulings in cards:
        body = f"{power}/{toughness}" if power else "—"
        lines.append(f"| {name} | `{oid}` | {mana or '—'} | {type_line} | {body} | {text} |")
    lines += [
        "",
        "Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.",
        "",
        "## Reuse",
        "",
        REUSE[family],
        "",
        "## Evidence",
        "",
        f"- Intended red: `build/verification-logs/` `q50 intended red {family}` (two canary tests, exit 101 with the family definitions held out of the registry).",
        "- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.",
        "- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).",
        "- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).",
        "- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.",
        "- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.",
        "",
        "## Assertions",
        "",
    ]
    lines += [f"- `q50_cohorts_batch::{name}`" for name in ASSERTIONS[family]]
    lines += [
        f"- `mechanic_registry::q50_cohorts_batch::{REGISTRY[family]}`",
        "- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`",
        "",
        "## Delivery",
        "",
        "- Independent read-only review: pending.",
        "- Batch gate: pending.",
        "- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.",
        "",
    ]
    (family_dir / "family.md").write_text("\n".join(lines), encoding="utf-8")

print(f"wrote {len(records)} family records")
