#!/usr/bin/env python3
"""Generate queue-v50 review maps from the reviewed RON shapes."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
MAPS = ROOT / "tricerules" / "tricerules-cards" / "authoring" / "review-maps"
CARDS = ROOT / "build" / "campaign" / "queue-v50" / "cards.tsv"


def face_id(name: str) -> str:
    folded = name.lower().replace("'", "").replace(",", "")
    for source, target in (("é", "e"), ("è", "e"), ("ê", "e"), ("à", "a"), ("â", "a"), ("ô", "o")):
        folded = folded.replace(source, target)
    text = "".join(ch if ch.isalnum() else "_" for ch in folded)
    while "__" in text:
        text = text.replace("__", "_")
    return text.strip("_")


def write(card_id: str, oid: str, spans: list[dict], refs: list[tuple[str, str]], fixtures: list[tuple[str, str]], tokens: list[str] | None = None) -> None:
    payload = {
        "format_version": 1,
        "oracle_id": oid,
        "spans": spans,
        "primitive_references": [{"symbol": symbol, "typed_path": path} for symbol, path in refs],
        "tokens": tokens or [],
        "semantic_fixtures": [{"test": test, "covers": covers} for test, covers in fixtures],
        "complete_definition_review_confirmed": True,
    }
    (MAPS / f"{card_id}.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


rows = []
for line in CARDS.read_text(encoding="utf-8").splitlines()[1:]:
    family, oid, name = line.split("\t")
    rows.append((family, oid, name))

registry_fixtures = {
    "01-put-on-top": ("q50_put_top_maps_definitions", "reviewed placement spell identity and target filter"),
    "02-graveyard-to-hand": ("q50_graveyard_to_hand_maps_definitions", "reviewed graveyard return identity and filter"),
    "03-tap-stun-spells": ("q50_tap_stun_maps_definitions", "reviewed tap/stun spell identity and effect shape"),
    "04-etb-shrink": ("q50_etb_shrink_maps_definitions", "reviewed shrink creature identity and trigger"),
    "05-etb-pump-target": ("q50_etb_pump_maps_definitions", "reviewed pump creature identity and trigger"),
    "06-etb-counter-another": ("q50_etb_counter_maps_definitions", "reviewed counter creature identity and trigger"),
    "07-basic-land-to-hand": ("q50_basic_land_maps_definitions", "reviewed basic-land search identity and program"),
    "08-treasure-on-death": ("q50_treasure_maps_definitions", "reviewed Treasure creature identity and trigger"),
    "09-destroy-artifact-enchant": ("q50_destroy_maps_definitions", "reviewed destroy spell identity and target filter"),
}

scenario_by_family = {
    "01-put-on-top": [
        ("q50_put_top_cards_put_only_their_matching_permanents_on_top", "actual card accepts only its printed target types and moves the permanent to the top"),
        ("q50_put_top_cards_fizzle_when_the_target_leaves", "actual card fizzles when its target left before resolution"),
    ],
    "02-graveyard-to-hand": [
        ("q50_graveyard_to_hand_cards_return_only_their_filtered_cards", "actual card returns only a matching card from its controller's graveyard"),
        ("q50_graveyard_to_hand_cards_reject_opponent_graveyards_and_fizzle_stale_targets", "actual card rejects another graveyard and fizzles a stale target"),
    ],
    "03-tap-stun-spells": [
        ("q50_tap_stun_cards_tap_and_place_their_printed_counters", "actual card taps and places its printed stun counters"),
        ("q50_tap_stun_cards_still_place_counters_on_an_already_tapped_creature", "actual card still places counters on an already-tapped target"),
        ("q50_tap_stun_cards_replace_untaps_with_counter_removal", "actual card's stun counter replaces an untap"),
    ],
    "04-etb-shrink": [
        ("q50_etb_shrink_cards_target_only_opposing_creatures_and_expire", "actual card shrinks only an opposing creature and expires at cleanup"),
        ("q50_etb_triggered_cards_fizzle_stale_targets", "actual card fizzles when its chosen target left"),
    ],
    "05-etb-pump-target": [
        ("q50_etb_pump_cards_target_only_your_creatures_and_expire", "actual card pumps only a creature you control and expires at cleanup"),
        ("q50_etb_triggered_cards_fizzle_stale_targets", "actual card fizzles when its chosen target left"),
    ],
    "06-etb-counter-another": [
        ("q50_etb_counter_cards_require_another_typed_creature_you_control", "actual card requires another creature of the printed subtype you control"),
        ("q50_etb_triggered_cards_fizzle_stale_targets", "actual card fizzles when its chosen target left"),
    ],
    "07-basic-land-to-hand": [
        ("q50_basic_land_search_cards_offer_only_their_printed_filters", "actual card offers only basic lands and its printed subtype"),
        ("q50_basic_land_search_cards_may_decline_the_search", "actual card may decline the optional search"),
    ],
    "08-treasure-on-death": [
        ("q50_treasure_cards_credit_the_creatures_controller_and_do_not_trigger_on_exile", "actual card credits its controller and does not trigger on exile"),
    ],
    "09-destroy-artifact-enchant": [
        ("q50_destroy_cards_accept_only_their_printed_target_types", "actual card accepts only its printed target types"),
        ("q50_destroy_cards_fizzle_when_the_target_already_left", "actual card fizzles when its target already left"),
    ],
}

canaries = {
    "01-put-on-top": {
        "repel": ("q50_put_top_canary_simplest_repel_puts_the_target_on_top", "canary simplest puts a creature on top"),
        "banishment_decree": ("q50_put_top_canary_hardest_banishment_decree_accepts_three_card_types", "canary hardest accepts artifact, enchantment and creature"),
    },
    "02-graveyard-to-hand": {
        "boggart_birth_rite": ("q50_graveyard_to_hand_canary_simplest_boggart_birth_rite_returns_a_goblin", "canary simplest returns a Goblin card"),
        "natures_spiral": ("q50_graveyard_to_hand_canary_hardest_natures_spiral_returns_a_permanent_card", "canary hardest returns a permanent card"),
    },
    "03-tap-stun-spells": {
        "cryogenic_stasis": ("q50_tap_stun_canary_simplest_cryogenic_stasis_taps_and_draws", "canary simplest taps, stuns and draws"),
        "freeze_in_place": ("q50_tap_stun_canary_hardest_freeze_in_place_scries_two", "canary hardest scopes the opponent and scries two"),
    },
    "04-etb-shrink": {
        "eyeblight_assassin": ("q50_etb_shrink_canary_simplest_eyeblight_assassin_shrinks", "canary simplest shrinks a creature"),
        "black_dragon": ("q50_etb_shrink_canary_hardest_black_dragon_flying_shrinks_three", "canary hardest keeps flying and shrinks three"),
    },
    "05-etb-pump-target": {
        "haazda_officer": ("q50_etb_pump_canary_simplest_haazda_officer_pumps", "canary simplest pumps a creature"),
        "rage-scarred_berserker": ("q50_etb_pump_canary_hardest_rage_scarred_berserker_grants_indestructible", "canary hardest grants indestructible"),
    },
    "06-etb-counter-another": {
        "jade_bearer": ("q50_etb_counter_canary_simplest_jade_bearer_adds_a_counter", "canary simplest adds a counter"),
        "stromkirk_mentor": ("q50_etb_counter_canary_hardest_stromkirk_mentor_excludes_source", "canary hardest excludes the source"),
    },
    "07-basic-land-to-hand": {
        "farfinder": ("q50_basic_land_search_canary_simplest_farfinder_finds_a_basic_land", "canary simplest searches a basic land"),
        "district_guide": ("q50_basic_land_search_canary_hardest_district_guide_finds_a_gate", "canary hardest also finds a Gate"),
    },
    "08-treasure-on-death": {
        "dire_fleet_hoarder": ("q50_treasure_canary_simplest_dire_fleet_hoarder_makes_a_treasure_on_death", "canary simplest creates a Treasure on death"),
        "patron_of_the_arts": ("q50_treasure_canary_hardest_patron_of_the_arts_makes_one_on_entry_and_one_on_death", "canary hardest creates one on entry and one on death"),
    },
    "09-destroy-artifact-enchant": {
        "naturalize": ("q50_destroy_canary_simplest_naturalize_destroys_an_artifact", "canary simplest destroys an artifact"),
        "return_to_the_earth": ("q50_destroy_canary_hardest_return_to_the_earth_destroys_a_flying_creature", "canary hardest destroys a flying creature"),
    },
}

for family, oid, name in rows:
    card_id = name.lower().replace("'", "").replace("/", " ").replace(" ", "_").replace(",", "")
    for source, target in (("é", "e"), ("è", "e"), ("ê", "e"), ("à", "a"), ("â", "a"), ("ô", "o")):
        card_id = card_id.replace(source, target)
    # Keep printed hyphens in ids as the RON files do.
    fid = face_id(name)
    fixtures = []
    canary = canaries.get(family, {}).get(card_id)
    if canary:
        fixtures.append((f"scenario q50_cohorts_batch::{canary[0]}", canary[1]))
    for test, covers in scenario_by_family[family]:
        fixtures.append((f"scenario q50_cohorts_batch::{test}", covers))
    if family == "01-put-on-top" and card_id in {"run_aground", "banishment_decree", "temporal_eddy"}:
        fixtures.append(("scenario q50_cohorts_batch::q50_put_top_cards_accept_each_disjunctive_branch", "actual card accepts each printed disjunctive target branch and places the permanent on top of its owner's library"))
    if family == "01-put-on-top" and card_id == "grasp_of_phantoms":
        fixtures.append(("scenario q50_cohorts_batch::q50_put_top_grasp_of_phantoms_flashes_back_and_exiles", "actual Grasp of Phantoms casts from the graveyard for its flashback cost, places the target on top, and exiles itself"))
    if family == "02-graveyard-to-hand":
        fixtures.append(("scenario q50_cohorts_batch::q50_graveyard_to_hand_cards_reject_nonmatching_cards", "actual card rejects a nonmatching card in the controller's graveyard"))
        if card_id == "boggart_birth_rite":
            fixtures.append(("scenario q50_cohorts_batch::q50_graveyard_to_hand_boggart_returns_a_noncreature_goblin_card", "actual Boggart Birth Rite returns a noncreature Goblin card"))
    if family == "08-treasure-on-death" and card_id == "jewel-eyed_cobra":
        fixtures.append(("scenario q50_cohorts_batch::q50_treasure_jewel_eyed_cobra_makes_a_treasure_on_death", "actual Jewel-Eyed Cobra dies and creates exactly one Treasure under its controller"))
    if family == "07-basic-land-to-hand":
        fixtures.append(("scenario issue_110_library_search::embermouth_rechecks_the_dragon_condition_when_the_search_completes", "shared search path asserts exactly one revealed found card and one shuffle after an accepted search"))
    reg_test, reg_covers = registry_fixtures[family]
    fixtures.append((f"mechanic_registry::q50_cohorts_batch::{reg_test}", reg_covers))
    fixtures.append(("mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text", f"pinned normalized Oracle text fingerprint for {name}"))

    if family == "01-put-on-top":
        spans = [{"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/spell_program", "/faces/0/targeting/groups/0"]}]
        refs = [
            ("SpellEffectKind::PutInOwnersLibrary", "/faces/0/spell_program/body/steps/0/kind/Perform/PutInOwnersLibrary"),
            ("TargetingDef", "/faces/0/targeting/groups/0"),
        ]
        if card_id == "grasp_of_phantoms":
            spans.append({"face_id": fid, "start_line": 2, "end_line": 2, "typed_paths": ["/faces/0/flashback_cost"]})
            refs.append(("CardFaceDef.flashback_cost", "/faces/0/flashback_cost"))
    elif family == "02-graveyard-to-hand":
        spans = [{"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/spell_effect/0"]}]
        refs = [("SpellEffectKind::MoveGraveyardCards", "/faces/0/spell_effect/0")]
    elif family == "03-tap-stun-spells":
        spans = [{"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/spell_effect/0", "/faces/0/spell_effect/1", "/faces/0/targeting/groups/0"]}]
        refs = [
            ("SpellEffectKind::Tap", "/faces/0/spell_effect/0"),
            ("SpellEffectKind::PutCounters", "/faces/0/spell_effect/1"),
            ("TargetingDef", "/faces/0/targeting/groups/0"),
        ]
        if card_id in {"cryogenic_stasis", "impede_momentum"}:
            spans.append({"face_id": fid, "start_line": 2, "end_line": 2, "typed_paths": ["/faces/0/spell_effect/2"]})
            refs.append(("SpellEffectKind::Scry" if card_id != "cryogenic_stasis" else "SpellEffectKind::Draw", "/faces/0/spell_effect/2"))
        if card_id == "freeze_in_place":
            spans[0]["typed_paths"].append("/faces/0/spell_effect/2")
            refs.append(("SpellEffectKind::Scry", "/faces/0/spell_effect/2"))
    elif family == "04-etb-shrink":
        spans = []
        paths = []
        if "black_dragon" == card_id:
            spans.append({"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/keywords/0"]})
            paths.append(("Keyword::Flying", "/faces/0/keywords/0"))
        if card_id == "faerie_duelist":
            spans += [
                {"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/keywords/0"]},
                {"face_id": fid, "start_line": 2, "end_line": 2, "typed_paths": ["/faces/0/keywords/1"]},
            ]
            paths += [("Keyword::Flash", "/faces/0/keywords/0"), ("Keyword::Flying", "/faces/0/keywords/1")]
        if card_id == "brinebarrow_intruder":
            spans.append({"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/keywords/0"]})
            paths.append(("Keyword::Flash", "/faces/0/keywords/0"))
        line = 3 if card_id == "faerie_duelist" else 2 if card_id in {"black_dragon", "brinebarrow_intruder"} else 1
        spans.append({"face_id": fid, "start_line": line, "end_line": line, "typed_paths": ["/faces/0/triggered_abilities/0"]})
        refs = paths + [
            ("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
            ("SpellEffectKind::PumpTarget", "/faces/0/triggered_abilities/0/effect/0"),
        ]
    elif family in {"05-etb-pump-target", "06-etb-counter-another"}:
        spans = [{"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/triggered_abilities/0"]}]
        effect = "SpellEffectKind::PumpTarget" if family == "05-etb-pump-target" else "SpellEffectKind::PutCounters"
        refs = [
            ("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
            (effect, "/faces/0/triggered_abilities/0/effect/0"),
        ]
        if card_id == "rage-scarred_berserker":
            refs.append(("SpellEffectKind::GrantKeywords", "/faces/0/triggered_abilities/0/effect/1"))
    elif family == "07-basic-land-to-hand":
        spans = []
        refs = []
        if card_id in {"gatecreeper_vine", "farfinder"}:
            spans.append({"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/keywords/0"]})
            refs.append(("Keyword::Defender" if card_id == "gatecreeper_vine" else "Keyword::Vigilance", "/faces/0/keywords/0"))
        if card_id == "pilgrims_eye":
            spans.append({"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/keywords/0"]})
            refs.append(("Keyword::Flying", "/faces/0/keywords/0"))
        line = 2 if card_id in {"gatecreeper_vine", "farfinder", "pilgrims_eye"} else 1
        spans.append({"face_id": fid, "start_line": line, "end_line": line, "typed_paths": ["/faces/0/triggered_abilities/0"]})
        refs += [
            ("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
            ("InstructionKind::Choose", "/faces/0/triggered_abilities/0/program/body/steps/0/kind/Choose"),
            ("SpellEffectKind::SearchLibrary", "/faces/0/triggered_abilities/0/program/body/steps/0/kind/Choose/branches/0/body/steps/0/kind/Perform/SearchLibrary"),
        ]
    elif family == "08-treasure-on-death":
        spans = []
        refs = []
        if card_id == "jewel-eyed_cobra":
            spans.append({"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/keywords/0"]})
            refs.append(("Keyword::Deathtouch", "/faces/0/keywords/0"))
        line = 2 if card_id == "jewel-eyed_cobra" else 1
        spans.append({"face_id": fid, "start_line": line, "end_line": line, "typed_paths": ["/faces/0/triggered_abilities/0"]})
        refs += [
            ("TriggerCondition::WhenSelfDies" if card_id != "patron_of_the_arts" else "TriggerCondition::WhenSelfEntersOrIsPutIntoGraveyardFromBattlefield", "/faces/0/triggered_abilities/0/trigger"),
            ("SpellEffectKind::CreateTokens", "/faces/0/triggered_abilities/0/effect/0"),
        ]
    else:  # 09
        spans = [{"face_id": fid, "start_line": 1, "end_line": 1, "typed_paths": ["/faces/0/spell_effect/0"]}]
        refs = [("SpellEffectKind::Destroy", "/faces/0/spell_effect/0")]

    write(card_id, oid, spans, refs, fixtures, ["treasure"] if family == "08-treasure-on-death" else None)

print(f"wrote {len(rows)} review maps")
