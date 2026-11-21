#!/usr/bin/env python3
"""Generate queue-v51 family records from the pinned revalidation data."""
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
OUT = ROOT / "build" / "campaign" / "queue-v51"
FAMILIES = OUT / "families"

SHAPES = {
    "01-token-spells": "One-shot spell creating a fixed token or fixed token cohort with no other effect.",
    "02-splicer-cycle": "Enters trigger creating one 3/3 Golem, plus a Golem-only static lord or activated regenerate.",
    "03-artifact-value-tutors": "Enters trigger with an optional private artifact-card search bounded to a printed mana-value band.",
    "04-subtype-harbingers": "Enters trigger with an optional private subtype search, revealed and put on top of the library.",
    "05-aura-restrictions": "Aura with one attached combat/untap restriction profile and, for two cards, an activation prohibition.",
    "06-etb-each-discard": "Enters trigger making each player discard one card; one card also discards to regenerate.",
    "08-attack-team-pumps": "Whenever this creature attacks, a mass pump or keyword grant to a printed scope.",
    "09-spirit-arcane-payoffs": "Whenever you cast a Spirit or Arcane spell, one targeted effect.",
}

REUSE = {
    "01-token-spells": "`raise_the_alarm` and the queue-v23/v43 token cohorts for `CreateTokens`; each printed token is a registered template mirroring `soldier_w_1_1`.",
    "02-splicer-cycle": "`goblin_chieftain` for `AnthemKeyword`, `glorious_anthem` for `AnthemPt`, `cudgel_troll` for activated `Regenerate`, and `raise_the_alarm` for `CreateTokens`.",
    "03-artifact-value-tutors": "`goblin_engineer` for the optional `SearchLibrary` trigger and `sylvan_ranger`/`borderland_ranger` for the private search flow.",
    "04-subtype-harbingers": "`sylvan_ranger` and `district_guide` for the optional private search, extended with `destination: TopOfLibrary` as on `vampiric_tutor`.",
    "05-aura-restrictions": "`pacifism` and `petrify` for `AttachedModifier` restrictions, `prohibit_activated` coverage from `petrify`, and `capture_sphere` for the untap-step restriction.",
    "06-etb-each-discard": "`fanatic_of_the_harrowing` for the player-set `Discard(who: EachPlayer)` continuation and `cudgel_troll` for activated regenerate.",
    "08-attack-team-pumps": "`goblin_chieftain` for subtype-scoped `PumpAll`/keyword grants, `stampede`/`rally_the_forces` for attacking scopes, and `ravenous_skirge` for the attack trigger.",
    "09-spirit-arcane-payoffs": "`pyroceratops`/`ivory_cup` for `WheneverPlayerCastsSpell`, `flametongue_kavu` for targeted damage, `cudgel_troll` for regenerate, and `jump` for a keyword grant.",
}

ASSERTIONS = {
    "01-token-spells": [
        "q51_token_spells_canary_simplest_call_of_the_conclave_creates_a_centaur",
        "q51_token_spells_canary_hardest_advent_of_the_wurm_keeps_trample",
        "q51_token_spells_create_the_exact_printed_tokens",
    ],
    "02-splicer-cycle": [
        "q51_splicers_canary_simplest_wing_splicer_grants_flying",
        "q51_splicers_canary_hardest_vital_splicer_regenerates_a_golem",
        "q51_splicers_create_one_golem_and_apply_the_printed_lord",
        "q51_splicers_do_not_modify_other_creatures",
        "q51_splicers_scope_their_lord_to_your_golems",
        "q51_vital_splicer_rejects_non_golem_and_opposing_golem_targets",
    ],
    "03-artifact-value-tutors": [
        "q51_artifact_tutors_canary_simplest_trinket_mage_finds_a_one_drop",
        "q51_artifact_tutors_canary_hardest_transit_mage_finds_four_or_five",
        "q51_artifact_tutors_offer_only_matching_artifacts",
        "q51_artifact_tutors_may_decline_without_shuffling",
        "q51_artifact_tutors_reveal_shuffle_and_shuffle_on_failure_to_find",
    ],
    "04-subtype-harbingers": [
        "q51_harbingers_canary_simplest_flamekin_finds_an_elemental",
        "q51_harbingers_canary_hardest_treefolk_accepts_both_branches",
        "q51_harbingers_put_only_their_subtype_on_top",
        "q51_harbingers_reveal_shuffle_and_may_decline",
        "q51_elvish_harbinger_taps_for_any_color",
    ],
    "05-aura-restrictions": [
        "q51_auras_canary_simplest_cloak_of_mists_makes_the_creature_unblockable",
        "q51_auras_canary_hardest_arrest_locks_attack_block_and_activation",
        "q51_auras_apply_each_printed_restriction",
        "q51_unblockable_auras_reject_an_ordinary_blocker",
        "q51_arrest_and_demotion_reject_the_printed_combat_actions",
        "q51_aura_follows_its_host_to_the_graveyard",
        "q51_paralyzing_grasp_keeps_the_creature_tapped",
        "q51_invisibility_is_blocked_only_by_walls",
    ],
    "06-etb-each-discard": [
        "q51_discard_canary_simplest_miasmic_mummy_makes_each_player_discard",
        "q51_discard_canary_hardest_earsplitting_rats_regenerates_after_discarding",
        "q51_discard_cards_use_only_their_printed_cost",
        "q51_discard_choices_are_private_exact_one_and_owner_graveyarded",
        "q51_earsplitting_rats_shield_prevents_destruction",
    ],
    "08-attack-team-pumps": [
        "q51_attack_pumps_canary_simplest_goblin_general_pumps_goblins",
        "q51_attack_pumps_canary_hardest_blood_chin_rager_grants_menace",
        "q51_attack_pumps_apply_each_printed_scope",
        "q51_attack_pumps_only_affect_attackers_for_keyword_grants",
    ],
    "09-spirit-arcane-payoffs": [
        "q51_spirit_arcane_canary_simplest_briarknit_kami_adds_a_counter",
        "q51_spirit_arcane_canary_hardest_kami_of_fires_roar_forbids_blocking",
        "q51_spirit_arcane_payoffs_fire_for_each_printed_effect",
        "q51_spirit_arcane_payoffs_also_fire_for_an_arcane_spell",
    ],
}

REGISTRY = {
    "01-token-spells": "q51_token_spells_map_definitions_and_tokens",
    "02-splicer-cycle": "q51_splicers_map_entry_and_lord_profiles",
    "03-artifact-value-tutors": "q51_artifact_value_tutors_map_bounded_searches",
    "04-subtype-harbingers": "q51_subtype_harbingers_map_top_of_library_searches",
    "05-aura-restrictions": "q51_aura_restrictions_map_static_profiles",
    "06-etb-each-discard": "q51_each_player_discard_maps_the_shared_discard",
    "08-attack-team-pumps": "q51_attack_team_pumps_map_attack_triggers",
    "09-spirit-arcane-payoffs": "q51_spirit_arcane_payoffs_map_the_cast_trigger",
}

BATCHES = {
    "01-token-spells": "batch-a",
    "02-splicer-cycle": "batch-a",
    "03-artifact-value-tutors": "batch-a",
    "04-subtype-harbingers": "batch-b",
    "05-aura-restrictions": "batch-b",
    "06-etb-each-discard": "batch-b",
    "08-attack-team-pumps": "batch-c",
    "09-spirit-arcane-payoffs": "batch-c",
}

records = {}
for line in (OUT / "revalidation.tsv").read_text(encoding="utf-8").splitlines()[1:]:
    family, oid, name, mana, type_line, power, toughness, layout, commander, text, rulings = line.split("\t")
    records.setdefault(family, []).append((oid, name, mana, type_line, power, toughness, text, rulings))

for family, cards in records.items():
    family_dir = FAMILIES / family
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
        f"- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).",
        "- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.",
        "- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).",
        "- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.",
        "- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.",
        "",
        "## Assertions",
        "",
    ]
    lines += [f"- `q51_cohorts_batch::{name}`" for name in ASSERTIONS[family]]
    lines += [
        f"- `mechanic_registry::q51_cohorts_batch::{REGISTRY[family]}`",
        "",
        "## Delivery",
        "",
        f"- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/{BATCHES[family]}.json` after initial and delta reviews; no remaining material findings.",
        "- Batch gate: pending.",
        "- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.",
        "",
    ]
    (family_dir / "family.md").write_text("\n".join(lines), encoding="utf-8")

print(f"wrote {len(records)} family records")
