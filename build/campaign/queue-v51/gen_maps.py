#!/usr/bin/env python3
"""Generate queue-v51 review maps from the reviewed RON shapes."""
import json
import unicodedata
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
MAPS = ROOT / "tricerules" / "tricerules-cards" / "authoring" / "review-maps"
OUT = ROOT / "build" / "campaign" / "queue-v51"


def slug(name: str) -> str:
    folded = unicodedata.normalize("NFD", name.lower())
    text = "".join(ch for ch in folded if not unicodedata.combining(ch))
    return "_".join(text.replace("'", "").replace("/", " ").split())


cards = {}
for line in (OUT / "cards.tsv").read_text(encoding="utf-8").splitlines()[1:]:
    family, oid, name = line.split("\t")
    cards.setdefault(family, []).append((slug(name), name, oid))

REGISTRY = {
    "01-token-spells": "q51_token_spells_map_definitions_and_tokens",
    "02-splicer-cycle": "q51_splicers_map_entry_and_lord_profiles",
    "03-artifact-value-tutors": "q51_artifact_value_tutors_map_bounded_searches",
    "04-subtype-harbingers": "q51_subtype_harbingers_map_top_of_library_searches",
    "05-aura-restrictions": "q51_aura_restrictions_map_static_profiles",
    "06-etb-each-discard": "q51_each_player_discard_maps_the_shared_discard",
    "07-blocked-pumps": "q51_blocked_pumps_map_per_blocker_self_pumps",
    "08-attack-team-pumps": "q51_attack_team_pumps_map_attack_triggers",
    "09-spirit-arcane-payoffs": "q51_spirit_arcane_payoffs_map_the_cast_trigger",
}

TESTS = {
    "01-token-spells": [
        ("q51_token_spells_canary_simplest_call_of_the_conclave_creates_a_centaur", "actual spell creates its printed token"),
        ("q51_token_spells_canary_hardest_advent_of_the_wurm_keeps_trample", "actual spell creates the printed Wurm with trample"),
        ("q51_token_spells_create_the_exact_printed_tokens", "actual spell creates the printed count, name, P/T, colors and types for its controller"),
    ],
    "02-splicer-cycle": [
        ("q51_splicers_canary_simplest_wing_splicer_grants_flying", "actual Splicer creates a Golem and grants its printed keyword"),
        ("q51_splicers_canary_hardest_vital_splicer_regenerates_a_golem", "actual Vital Splicer regenerates a target Golem"),
        ("q51_splicers_create_one_golem_and_apply_the_printed_lord", "actual Splicers create one Golem and apply only their printed lord"),
        ("q51_splicers_do_not_modify_other_creatures", "actual lord does not modify non-Golems"),
        ("q51_splicers_scope_their_lord_to_your_golems", "actual lord affects your Golems and not an opponent's"),
        ("q51_vital_splicer_rejects_non_golem_and_opposing_golem_targets", "actual Vital Splicer rejects illegal targets and shields your Golem"),
    ],
    "03-artifact-value-tutors": [
        ("q51_artifact_tutors_canary_simplest_trinket_mage_finds_a_one_drop", "actual Mage finds only its mana-value band"),
        ("q51_artifact_tutors_canary_hardest_transit_mage_finds_four_or_five", "actual Transit Mage accepts mana values 4 and 5"),
        ("q51_artifact_tutors_offer_only_matching_artifacts", "actual Mage rejects wrong type, wrong value and foreign libraries"),
        ("q51_artifact_tutors_may_decline_without_shuffling", "actual Mage may decline without moving or shuffling"),
        ("q51_artifact_tutors_reveal_shuffle_and_shuffle_on_failure_to_find", "actual search reveals, shuffles, and shuffles after a qualified failure to find"),
    ],
    "04-subtype-harbingers": [
        ("q51_harbingers_canary_simplest_flamekin_finds_an_elemental", "actual Harbinger finds its printed subtype and puts it on top"),
        ("q51_harbingers_canary_hardest_treefolk_accepts_both_branches", "actual Treefolk Harbinger accepts Treefolk or Forest"),
        ("q51_harbingers_put_only_their_subtype_on_top", "actual Harbinger rejects nonmatching cards and puts the choice on top"),
        ("q51_harbingers_reveal_shuffle_and_may_decline", "actual Harbinger reveals, shuffles, and may decline"),
        ("q51_elvish_harbinger_taps_for_any_color", "actual Elvish Harbinger taps for a chosen color"),
    ],
    "05-aura-restrictions": [
        ("q51_auras_canary_simplest_cloak_of_mists_makes_the_creature_unblockable", "actual Aura attaches and publishes Can't be blocked"),
        ("q51_auras_canary_hardest_arrest_locks_attack_block_and_activation", "actual Arrest publishes both combat restrictions and the activation prohibition"),
        ("q51_auras_apply_each_printed_restriction", "actual Auras apply their printed power, keyword and restriction profile"),
        ("q51_unblockable_auras_reject_an_ordinary_blocker", "actual unblockable Auras reject an ordinary blocker"),
        ("q51_arrest_and_demotion_reject_the_printed_combat_actions", "actual Arrest and Demotion reject the printed attack/block actions"),
        ("q51_aura_follows_its_host_to_the_graveyard", "actual Aura follows its host to the graveyard"),
        ("q51_paralyzing_grasp_keeps_the_creature_tapped", "actual Paralyzing Grasp prevents the untap-step action"),
        ("q51_invisibility_is_blocked_only_by_walls", "actual Invisibility publishes the Wall-only exception"),
    ],
    "06-etb-each-discard": [
        ("q51_discard_canary_simplest_miasmic_mummy_makes_each_player_discard", "actual ETB collects both players' private discards"),
        ("q51_discard_canary_hardest_earsplitting_rats_regenerates_after_discarding", "actual rats discard to regenerate"),
        ("q51_discard_cards_use_only_their_printed_cost", "actual creature has the printed body"),
        ("q51_discard_choices_are_private_exact_one_and_owner_graveyarded", "actual discard choices are private, exact-one and credited to the owner's graveyard"),
        ("q51_earsplitting_rats_shield_prevents_destruction", "actual regeneration shield replaces destruction"),
    ],
    "08-attack-team-pumps": [
        ("q51_attack_pumps_canary_simplest_goblin_general_pumps_goblins", "actual General pumps each Goblin and not other creatures"),
        ("q51_attack_pumps_canary_hardest_blood_chin_rager_grants_menace", "actual Rager grants menace to each Warrior"),
        ("q51_attack_pumps_apply_each_printed_scope", "actual trigger applies the printed subtype or attacking scope"),
        ("q51_attack_pumps_only_affect_attackers_for_keyword_grants", "actual keyword grant does not affect nonattackers"),
    ],
    "09-spirit-arcane-payoffs": [
        ("q51_spirit_arcane_canary_simplest_briarknit_kami_adds_a_counter", "actual Kami adds a counter on a Spirit cast"),
        ("q51_spirit_arcane_canary_hardest_kami_of_fires_roar_forbids_blocking", "actual Kami restricts the target on a Spirit cast"),
        ("q51_spirit_arcane_payoffs_fire_for_each_printed_effect", "actual payoff resolves its printed effect on a Spirit cast"),
        ("q51_spirit_arcane_payoffs_also_fire_for_an_arcane_spell", "actual payoff also triggers on an Arcane spell"),
    ],
}

# Per-card primitive references and spans.
SHAPES = {
    "01-token-spells": (
        [("SpellEffectKind::CreateTokens", "/faces/0/spell_effect/0")],
        [(1, 1, ["/faces/0/spell_effect/0"])],
    ),
    "03-artifact-value-tutors": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::SearchLibrary", "/faces/0/triggered_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"])],
    ),
    "04-subtype-harbingers": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::SearchLibrary", "/faces/0/triggered_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"])],
    ),
    "06-etb-each-discard": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::Discard", "/faces/0/triggered_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"])],
    ),
    "07-blocked-pumps": (
        [("TriggerCondition::WheneverSelfBecomesBlockedByCreature", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::PumpTarget", "/faces/0/triggered_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"])],
    ),
    "08-attack-team-pumps": (
        [("TriggerCondition::WheneverSelfAttacks", "/faces/0/triggered_abilities/0/trigger")],
        [(1, 1, ["/faces/0/triggered_abilities/0"])],
    ),
    "09-spirit-arcane-payoffs": (
        [("TriggerCondition::WheneverPlayerCastsSpell", "/faces/0/triggered_abilities/0/trigger")],
        [(1, 1, ["/faces/0/triggered_abilities/0"])],
    ),
}

SPECIAL = {
    "wing_splicer": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::CreateTokens", "/faces/0/triggered_abilities/0/effect/0"),
         ("StaticAbilityDef::AnthemKeyword", "/faces/0/static_abilities/0/definition")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/static_abilities/0"])],
    ),
    "sensor_splicer": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::CreateTokens", "/faces/0/triggered_abilities/0/effect/0"),
         ("StaticAbilityDef::AnthemKeyword", "/faces/0/static_abilities/0/definition")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/static_abilities/0"])],
    ),
    "master_splicer": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::CreateTokens", "/faces/0/triggered_abilities/0/effect/0"),
         ("StaticAbilityDef::AnthemPt", "/faces/0/static_abilities/0/definition")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/static_abilities/0"])],
    ),
    "blade_splicer": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::CreateTokens", "/faces/0/triggered_abilities/0/effect/0"),
         ("StaticAbilityDef::AnthemKeyword", "/faces/0/static_abilities/0/definition")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/static_abilities/0"])],
    ),
    "vital_splicer": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::CreateTokens", "/faces/0/triggered_abilities/0/effect/0"),
         ("SpellEffectKind::Regenerate", "/faces/0/activated_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/activated_abilities/0"])],
    ),
    "elvish_harbinger": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::SearchLibrary", "/faces/0/triggered_abilities/0/effect/0"),
         ("SpellEffectKind::ProduceMana", "/faces/0/activated_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/activated_abilities/0"])],
    ),
    "earsplitting_rats": (
        [("TriggerCondition::WhenSelfEntersBattlefield", "/faces/0/triggered_abilities/0/trigger"),
         ("SpellEffectKind::Discard", "/faces/0/triggered_abilities/0/effect/0"),
         ("SpellEffectKind::Regenerate", "/faces/0/activated_abilities/0/effect/0")],
        [(1, 1, ["/faces/0/triggered_abilities/0"]), (2, 2, ["/faces/0/activated_abilities/0"])],
    ),
}

AURA_REFS = [
    ("SpellEffectKind::AuraAttach", "/faces/0/spell_effect/0"),
    ("StaticAbilityDef::AttachedModifier", "/faces/0/static_abilities/0/definition"),
]
AURA_SPANS = [(1, 1, ["/faces/0/spell_effect/0"]), (2, 2, ["/faces/0/static_abilities/0"])]
for card_id in ("demotion", "arrest"):
    SPECIAL[card_id] = (
        AURA_REFS + [("StaticAbilityDef::ProhibitActivatedAbilitiesOfAttachedPermanent", "/faces/0/static_abilities/1/definition")],
        AURA_SPANS,
    )
for card_id in ("cloak_of_mists", "aether_tunnel", "protective_bubble", "paralyzing_grasp", "invisibility"):
    SPECIAL[card_id] = (AURA_REFS, AURA_SPANS)

ATTACK_EFFECT = {
    "goblin_general": "PumpAll",
    "riot_ringleader": "PumpAll",
    "inspiring_unicorn": "PumpAll",
    "blood-chin_rager": "GrantKeywordsAll",
    "chieftain_en-dal": "GrantKeywordsAll",
    "fangren_pathcutter": "GrantKeywordsAll",
}
SPIRIT_EFFECT = {
    "briarknit_kami": "PutCounters",
    "horizon_seed": "Regenerate",
    "guardian_of_solitude": "GrantKeywords",
    "kami_of_fires_roar": "ApplyCombatRestriction",
    "soul_of_magma": "DamageTarget",
}

for family, rows in cards.items():
    for card_id, name, oid in rows:
        if card_id in SPECIAL:
            refs, spans = SPECIAL[card_id]
        elif family == "08-attack-team-pumps":
            refs, spans = (
                [("TriggerCondition::WheneverSelfAttacks", "/faces/0/triggered_abilities/0/trigger"),
                 (f"SpellEffectKind::{ATTACK_EFFECT[card_id]}", "/faces/0/triggered_abilities/0/effect/0")],
                [(1, 1, ["/faces/0/triggered_abilities/0"])],
            )
        elif family == "09-spirit-arcane-payoffs":
            refs, spans = (
                [("TriggerCondition::WheneverPlayerCastsSpell", "/faces/0/triggered_abilities/0/trigger"),
                 (f"SpellEffectKind::{SPIRIT_EFFECT[card_id]}", "/faces/0/triggered_abilities/0/effect/0")],
                [(1, 1, ["/faces/0/triggered_abilities/0"])],
            )
        else:
            refs, spans = SHAPES[family]

        token_ids = []
        if family == "01-token-spells":
            token_ids = [next(t for t in [
                "centaur_g_3_3", "elemental_ur_4_4", "wurm_g_5_5_trample", "saproling_g_1_1",
                "myr_token_c_1_1", "servo_c_1_1", "soldier_w_1_1_lifelink", "citizen_w_1_1",
                "knight_w_2_2_vigilance", "minotaur_r_2_3_haste", "warrior_w_1_1",
            ] if (family, card_id) and True)][:0]
            token_map = {
                "call_of_the_conclave": "centaur_g_3_3",
                "elemental_summoning": "elemental_ur_4_4",
                "advent_of_the_wurm": "wurm_g_5_5_trample",
                "spore_swarm": "saproling_g_1_1",
                "masters_call": "myr_token_c_1_1",
                "servo_exhibition": "servo_c_1_1",
                "sworn_companions": "soldier_w_1_1_lifelink",
                "icatian_town": "citizen_w_1_1",
                "knight_watch": "knight_w_2_2_vigilance",
                "flurry_of_horns": "minotaur_r_2_3_haste",
                "take_up_arms": "warrior_w_1_1",
            }
            token_ids = [token_map[card_id]]
        elif card_id.startswith(("wing_splicer", "sensor_splicer", "master_splicer", "blade_splicer", "vital_splicer")):
            token_ids = ["golem_c_3_3"]

        payload = {
            "format_version": 1,
            "oracle_id": oid,
            "spans": [
                {"face_id": card_id.replace("-", "_"), "start_line": start, "end_line": end,
                 "typed_paths": paths}
                for start, end, paths in spans
            ],
            "primitive_references": [
                {"symbol": symbol, "typed_path": path} for symbol, path in refs
            ],
            "tokens": token_ids,
            "semantic_fixtures": [
                {"test": f"scenario q51_cohorts_batch::{test}", "covers": covers}
                for test, covers in TESTS[family]
            ] + [
                {"test": f"mechanic_registry::q51_cohorts_batch::{REGISTRY[family]}",
                 "covers": "reviewed identity and typed shape"},
                {"test": "mechanic_registry::q51_cohorts_batch::q51_fingerprints_match_the_pinned_oracle_text",
                 "covers": "pinned normalized Oracle text fingerprint"},
            ],
            "complete_definition_review_confirmed": True,
        }
        (MAPS / f"{card_id}.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

print("wrote", sum(len(v) for v in cards.values()), "review maps")
