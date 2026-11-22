//! Actual-card scenarios for the queue-v47 reuse-only cohorts.
//!
//! Expectations are printed Oracle values. CR 603.6a governs landfall entry triggers;
//! CR 603.2/601.2a govern cast triggers; CR 121.2 governs draw ordinality; CR 604.1/611.3a/
//! 613.1f/702.19 govern static trample; CR 702.29a-b and 601.2h govern basic landcycling;
//! CR 113.7a/303.4/602.2a govern granted Aura abilities; CR 120.3a and 702.9a govern sweeper
//! damage; CR 701.8, 115.1/601.2c, 508/509 and 608.2b govern filtered destruction and its
//! stale-target recheck; CR 120.2b/608.2b govern power-based bite damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand, TargetRefKind,
};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
}

fn plus_one(e: &GameEngine, object: u32) -> u32 {
    e.state.objects[&object].counter_count(CounterKind::PlusOnePlusOne)
}

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

// ── 01 landfall counter ─────────────────────────────────────────────────────

#[test]
fn q47_landfall_counter_canary_simplest_adds_one_counter() {
    let mut e = main1_engine(47_010_001, &["vinelasher_kudzu"], &[]);
    let kudzu = move_ready_to_battlefield(&mut e, 0, "vinelasher_kudzu");
    ensure_card_in_hand(&mut e, 0, "forest");
    let forest = hand_index_for_card(&e, 0, "forest");
    e.apply_scenario_command(0, &play_land(forest))
        .expect("play a Forest");
    pass_both_players(&mut e);
    assert_eq!(plus_one(&e, kudzu), 1);
    assert_eq!(e.effective_power(kudzu), Some(2));
    assert_eq!(e.effective_toughness(kudzu), Some(2));
    // A land entering under an opponent's control is not this controller's landfall.
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    pass_both_players(&mut e);
    assert_eq!(plus_one(&e, kudzu), 1, "opponent's land does not trigger");
}

#[test]
fn q47_landfall_counter_canary_hardest_keeps_keyword_and_stacks() {
    let mut e = main1_engine(47_010_002, &["territorial_scythecat"], &[]);
    let cat = move_ready_to_battlefield(&mut e, 0, "territorial_scythecat");
    assert!(e.effective_has_keyword(cat, Keyword::Trample));
    ensure_card_in_hand(&mut e, 0, "forest");
    let forest = hand_index_for_card(&e, 0, "forest");
    e.apply_scenario_command(0, &play_land(forest))
        .expect("play a Forest");
    pass_both_players(&mut e);
    assert_eq!(plus_one(&e, cat), 1);
    assert_eq!(e.effective_power(cat), Some(3));
    assert_eq!(e.effective_toughness(cat), Some(2));
    assert!(e.effective_has_keyword(cat, Keyword::Trample));
}

#[test]
fn q47_landfall_counter_cards_share_the_printed_shape() {
    for (index, (id, body, keyword)) in [
        ("vinelasher_kudzu", (1, 1), None),
        ("xindi_surveyors", (3, 2), None),
        ("prowling_felidar", (2, 3), Some(Keyword::Vigilance)),
        ("territorial_scythecat", (2, 1), Some(Keyword::Trample)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(47_011_000 + index as u64, &[id], &[]);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        assert_eq!(e.effective_power(source), Some(body.0), "{id}");
        assert_eq!(e.effective_toughness(source), Some(body.1), "{id}");
        if let Some(keyword) = keyword {
            assert!(e.effective_has_keyword(source, keyword), "{id}");
        }
        ensure_card_in_hand(&mut e, 0, "forest");
        let forest = hand_index_for_card(&e, 0, "forest");
        e.apply_scenario_command(0, &play_land(forest))
            .expect("play a Forest");
        pass_both_players(&mut e);
        assert_eq!(plus_one(&e, source), 1, "{id}");
        assert_eq!(e.effective_power(source), Some(body.0 + 1), "{id}");
        assert_eq!(e.effective_toughness(source), Some(body.1 + 1), "{id}");
    }
}

// ── 02 noncreature spell counter ────────────────────────────────────────────

#[test]
fn q47_noncreature_counter_canary_simplest_triggers_before_the_spell() {
    let mut e = main1_engine(47_020_001, &["deeproot_champion"], &[]);
    let champion = move_ready_to_battlefield(&mut e, 0, "deeproot_champion");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "bonesplitter");
    let slot = hand_index_for_card(&e, 0, "bonesplitter");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast a noncreature spell");
    assert_eq!(e.state.stack.len(), 2, "trigger is above the spell");
    assert!(
        e.state
            .stack
            .last()
            .is_some_and(|item| item.ability_text.is_some()),
        "the top stack object is the counter trigger"
    );
    let spell_id = e
        .state
        .stack
        .iter()
        .find(|item| item.ability_text.is_none())
        .expect("artifact spell")
        .id;
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    inject_card_into_hand(&mut e, 1, "counterspell");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let counter = hand_index_for_card(&e, 1, "counterspell");
    e.apply_scenario_command(
        1,
        &cast_spell(
            counter,
            vec![TargetRef {
                object_id: spell_id,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
        ),
    )
    .expect("counter the artifact spell");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        plus_one(&e, champion),
        1,
        "the trigger resolves even if the spell is countered"
    );
    assert_eq!(e.effective_power(champion), Some(2));
}

#[test]
fn q47_noncreature_counter_canary_hardest_ignores_creature_spells() {
    let mut e = main1_engine(47_020_002, &["sprite_dragon"], &[]);
    let dragon = move_ready_to_battlefield(&mut e, 0, "sprite_dragon");
    assert!(e.effective_has_keyword(dragon, Keyword::Flying));
    assert!(e.effective_has_keyword(dragon, Keyword::Haste));
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "bonesplitter");
    let equipment = hand_index_for_card(&e, 0, "bonesplitter");
    e.apply_scenario_command(0, &cast_spell(equipment, vec![]))
        .expect("cast a noncreature spell");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, dragon), 1);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let bear = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(bear, vec![]))
        .expect("cast a creature spell");
    assert_eq!(e.state.stack.len(), 1, "creature spell does not trigger");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, dragon), 1);
}

#[test]
fn q47_noncreature_counter_cards_share_the_printed_shape() {
    for (index, (id, body, keywords)) in [
        ("deeproot_champion", (1, 1), vec![]),
        ("pyroceratops", (2, 3), vec![Keyword::Trample]),
        (
            "sprite_dragon",
            (1, 1),
            vec![Keyword::Flying, Keyword::Haste],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(47_021_000 + index as u64, &[id], &[]);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        assert_eq!(e.effective_power(source), Some(body.0), "{id}");
        assert_eq!(e.effective_toughness(source), Some(body.1), "{id}");
        for keyword in keywords {
            assert!(e.effective_has_keyword(source, keyword), "{id}");
        }
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "bonesplitter");
        let slot = hand_index_for_card(&e, 0, "bonesplitter");
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast a noncreature spell");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(plus_one(&e, source), 1, "{id}");
    }
}

// ── 03 second draw counter ──────────────────────────────────────────────────

#[test]
fn q47_second_draw_counter_canary_simplest_triggers_once() {
    let mut e = main1_engine(47_030_001, &["knights_of_dol_amroth"], &[]);
    let knight = move_ready_to_battlefield(&mut e, 0, "knights_of_dol_amroth");
    cast_named(
        &mut e,
        0,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, knight), 1);
    // The second card drawn in a turn can trigger the ability only once.
    cast_named(
        &mut e,
        0,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, knight), 1);
    // "You draw" is controller-scoped: an opponent's second draw does not trigger it.
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    cast_named(
        &mut e,
        1,
        "inspiration",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, knight), 1, "opponent's draw does not trigger");
}

#[test]
fn q47_second_draw_counter_canary_hardest_creates_a_thopter_on_death() {
    let mut e = main1_engine(47_030_002, &["thopter_mechanic"], &[]);
    let mechanic = move_ready_to_battlefield(&mut e, 0, "thopter_mechanic");
    cast_named(
        &mut e,
        0,
        "divination",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, mechanic), 1);
    cast_named(
        &mut e,
        0,
        "murder",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        target_object(mechanic),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&mechanic].zone, Zone::Graveyard);
    let tokens = battlefield_token_oids(&e, 0, "thopter_c_1_1_flying");
    assert_eq!(tokens.len(), 1, "one Thopter token");
    assert!(e.effective_has_keyword(tokens[0], Keyword::Flying));
}

#[test]
fn q47_second_draw_counter_cards_share_the_printed_shape() {
    for (index, (id, body, keywords)) in [
        (
            "faerie_vandal",
            (1, 2),
            vec![Keyword::Flying, Keyword::Flash],
        ),
        ("knights_of_dol_amroth", (3, 3), vec![]),
        ("thopter_mechanic", (2, 1), vec![]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(47_031_000 + index as u64, &[id], &[]);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        assert_eq!(e.effective_power(source), Some(body.0), "{id}");
        assert_eq!(e.effective_toughness(source), Some(body.1), "{id}");
        for keyword in keywords {
            assert!(e.effective_has_keyword(source, keyword), "{id}");
        }
        cast_named(
            &mut e,
            0,
            "divination",
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            vec![],
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(plus_one(&e, source), 1, "{id}");
    }
}

// ── 04 team trample ────────────────────────────────────────────────────────

fn assert_trample_anthem(id: &str, seed: u64) {
    let mut e = main1_engine(seed, &[id], &["grizzly_bears"]);
    let source = move_ready_to_battlefield(&mut e, 0, id);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert!(e.effective_has_keyword(source, Keyword::Trample), "{id}");
    assert!(
        e.effective_has_keyword(first, Keyword::Trample),
        "{id}: other creatures you control"
    );
    assert!(
        !e.effective_has_keyword(opposing, Keyword::Trample),
        "{id}: opponent's creature unaffected"
    );
    let late = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    assert!(
        e.effective_has_keyword(late, Keyword::Trample),
        "{id}: late entrant"
    );
    cast_named(
        &mut e,
        0,
        "go_for_the_throat",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(source),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert!(
        !e.effective_has_keyword(first, Keyword::Trample),
        "{id}: anthem ends when the source leaves"
    );
}

#[test]
fn q47_team_trample_canary_simplest_grants_other_creatures_trample() {
    assert_trample_anthem("khenra_charioteer", 47_040_001);
}

#[test]
fn q47_team_trample_canary_hardest_supports_an_ability_word() {
    assert_trample_anthem("bloodcrusher_of_khorne", 47_040_002);
}

#[test]
fn q47_team_trample_cards_share_the_printed_shape() {
    for (index, id) in ["khenra_charioteer", "nyleas_forerunner"]
        .into_iter()
        .enumerate()
    {
        assert_trample_anthem(id, 47_041_000 + index as u64);
    }
}

// ── 05 basic landcycling spells ────────────────────────────────────────────

#[test]
fn q47_basic_landcycling_canary_simplest_gains_eight_life() {
    let mut e = main1_engine(47_050_001, &["sylvan_bounty"], &[]);
    cast_named(
        &mut e,
        0,
        "sylvan_bounty",
        ManaGift {
            g: 1,
            c: 5,
            ..Default::default()
        },
        target_player(0),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 28);
}

#[test]
fn q47_basic_landcycling_canary_hardest_rechecks_a_stale_damage_target() {
    let mut e = main1_engine(47_050_002, &["fiery_fall"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "fiery_fall",
        ManaGift {
            r: 1,
            c: 5,
            ..Default::default()
        },
        target_object(bear),
    );
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    cast_named(
        &mut e,
        1,
        "murder",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&bear].damage, 0,
        "the damage instruction does not resolve after the target left"
    );
}

#[test]
fn q47_basic_landcycling_spells_apply_their_printed_effects() {
    for (index, id) in [
        "absorb_vis",
        "fiery_fall",
        "mental_journey",
        "sylvan_bounty",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(47_051_000 + index as u64, &[id], &["grizzly_bears"]);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let mana = match id {
            "absorb_vis" => ManaGift {
                b: 1,
                c: 6,
                ..Default::default()
            },
            "fiery_fall" => ManaGift {
                r: 1,
                c: 5,
                ..Default::default()
            },
            "mental_journey" => ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
            _ => ManaGift {
                g: 1,
                c: 5,
                ..Default::default()
            },
        };
        let targets = match id {
            "fiery_fall" => target_object(bear),
            "mental_journey" => vec![],
            _ => target_player(1),
        };
        cast_named(&mut e, 0, id, mana, targets);
        resolve_entire_stack_two_player(&mut e);
        match id {
            "absorb_vis" => {
                assert_eq!(e.state.players[1].life, 16, "target player loses 4");
                assert_eq!(e.state.players[0].life, 24, "you gain 4");
            }
            "fiery_fall" => assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard),
            "mental_journey" => {
                assert_eq!(e.state.players[0].hand.len(), 10, "draw three from seven")
            }
            "sylvan_bounty" => assert_eq!(e.state.players[1].life, 28),
            _ => unreachable!(),
        }
    }
}

#[test]
fn q47_basic_landcycling_cards_search_a_basic_land_from_hand() {
    for (index, (id, ability_index, mana)) in [
        (
            "absorb_vis",
            0,
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "fiery_fall",
            0,
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "mental_journey",
            0,
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "sylvan_bounty",
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let decks = Some(vec![
            vec!["grizzly_bears".to_string(); 20],
            vec!["forest".to_string(); 20],
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            47_052_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let source = inject_card_into_hand(&mut e, 0, id);
        let mountain = inject_library_card(&mut e, 0, "mountain");
        let forest = inject_library_card(&mut e, 0, "forest");
        e.apply_scenario_command(0, &hand_ability(&e, source, ability_index))
            .expect_err("the full cost is checked before any discard");
        assert_eq!(e.state.objects[&source].zone, Zone::Hand);
        give_mana(&mut e, 0, mana);
        e.apply_scenario_command(0, &hand_ability(&e, source, ability_index))
            .expect("activate basic landcycling");
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard, "{id}");
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        let search = e
            .apply_scenario_command(1, &pass())
            .expect("basic landcycling resolves to a private search");
        let choice = find_resolution_choice(&search).expect("private library search");
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, vec![mountain, forest], "{id}");
        let completed = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![mountain]))
            .expect("choose the basic land");
        assert_eq!(e.state.objects[&mountain].zone, Zone::Hand, "{id}");
        assert!(completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                if log.text == "P0 shuffles their library."
        )));
    }
}

// ── 06 pinger auras ────────────────────────────────────────────────────────

fn aura_ping(id: &str, seed: u64) -> (GameEngine, u32, u32) {
    let mut e = main1_engine(seed, &[id, "grizzly_bears"], &[]);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let mana = match id {
        "psionic_gift" => ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        _ => ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    };
    cast_named(&mut e, 0, id, mana, target_object(creature));
    resolve_entire_stack_two_player(&mut e);
    (e, creature, 0)
}

#[test]
fn q47_pinger_aura_canary_simplest_grants_a_tap_ping() {
    let (mut e, creature, _) = aura_ping("power_of_fire", 47_060_001);
    e.apply_scenario_command(0, &activate_ability(creature, 1, target_player(1)))
        .expect("activate granted ability");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q47_pinger_aura_canary_hardest_restricts_the_host_and_sacrifices_itself() {
    let mut e = main1_engine(
        47_060_002,
        &["fire_whip", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "fire_whip");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "fire_whip");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(opposing)))
            .is_err(),
        "Fire Whip can only enchant a creature you control"
    );
    let slot = hand_index_for_card(&e, 0, "fire_whip");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(own)))
        .expect("cast Fire Whip on own creature");
    resolve_entire_stack_two_player(&mut e);
    let whip = battlefield_object_for_card(&e, 0, "fire_whip");
    e.apply_scenario_command(0, &activate_ability_for(&e, whip, 0, target_player(1)))
        .expect("sacrifice Fire Whip for 1 damage");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&whip].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q47_pinger_aura_cards_share_the_printed_shape() {
    for (index, id) in ["power_of_fire", "psionic_gift", "fire_whip"]
        .into_iter()
        .enumerate()
    {
        let (mut e, creature, _) = aura_ping(id, 47_061_000 + index as u64);
        e.apply_scenario_command(0, &activate_ability(creature, 1, target_player(1)))
            .expect("activate granted ability");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, 19, "{id}");
    }
}

// ── 07 sweeper damage ──────────────────────────────────────────────────────

#[test]
fn q47_sweeper_canary_simplest_damages_every_creature() {
    let mut e = main1_engine(
        47_070_001,
        &["desert_sandstorm", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
    let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_named(
        &mut e,
        0,
        "desert_sandstorm",
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&opposing].damage, 1);
}

#[test]
fn q47_sweeper_canary_hardest_excludes_flyers() {
    let mut e = main1_engine(
        47_070_002,
        &["seismic_shudder", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
    let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
    let angel = inject_creature_with_stats(&mut e, 0, "serra_angel", 4, 4);
    cast_named(
        &mut e,
        0,
        "seismic_shudder",
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&angel].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&angel].damage, 0);
}

#[test]
fn q47_sweeper_cards_apply_the_printed_amount_and_filter() {
    for (index, (id, mana, amount, excludes_flying)) in [
        (
            "desert_sandstorm",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            1,
            false,
        ),
        (
            "sweltering_suns",
            ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
            3,
            false,
        ),
        (
            "seismic_shudder",
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            1,
            true,
        ),
        (
            "tremor",
            ManaGift {
                r: 1,
                ..Default::default()
            },
            1,
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(47_071_000 + index as u64, &[id, "grizzly_bears"], &[]);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
        let angel = inject_creature_with_stats(&mut e, 1, "serra_angel", 4, 4);
        cast_named(&mut e, 0, id, mana, vec![]);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.objects[&angel].zone, Zone::Battlefield, "{id}");
        if excludes_flying {
            assert_eq!(e.state.objects[&angel].damage, 0, "{id}: flyer excluded");
        } else {
            assert_eq!(
                e.state.objects[&angel].damage, amount,
                "{id}: flyer still damaged"
            );
        }
    }
}

#[test]
fn q47_sweeper_canary_hardest_cycles_sweltering_suns() {
    let mut e = main1_engine(47_070_003, &["sweltering_suns"], &[]);
    let source = inject_card_into_hand(&mut e, 0, "sweltering_suns");
    let drawn = seat_on_top(&mut e, 0, &["grizzly_bears"])[0];
    e.apply_scenario_command(0, &hand_ability(&e, source, 0))
        .expect_err("cycling needs mana");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, source, 0))
        .expect("cycle Sweltering Suns");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
}

// ── 08 filtered destruction ────────────────────────────────────────────────

fn fixture_permanent(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    match card {
        "grizzly_bears" => inject_creature_with_stats(e, player, card, 2, 2),
        "hill_giant" => inject_creature_with_stats(e, player, card, 3, 3),
        "juggernaut" => inject_creature_with_stats(e, player, card, 5, 3),
        _ => inject_permanent_on_battlefield(e, player, card),
    }
}

#[test]
fn q47_filtered_destroy_canary_simplest_checks_power_at_cast() {
    let mut e = main1_engine(47_080_001, &["defeat"], &["grizzly_bears", "hill_giant"]);
    let bear = fixture_permanent(&mut e, 1, "grizzly_bears");
    let giant = fixture_permanent(&mut e, 1, "hill_giant");
    assert_eq!(e.effective_power(giant), Some(3), "Hill Giant is 3/3");
    assert_eq!(e.effective_power(bear), Some(2), "Grizzly Bears is 2/2");
    inject_card_into_hand(&mut e, 0, "defeat");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "defeat");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(giant)))
            .is_err(),
        "power 3 is above Defeat's power-2 bound"
    );
    let slot = hand_index_for_card(&e, 0, "defeat");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("destroy the power-2 creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&giant].zone, Zone::Battlefield);
}

#[test]
fn q47_filtered_destroy_canary_hardest_requires_combat_role() {
    let mut e = main1_engine(
        47_080_002,
        &["neck_snap", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass()).expect("ap pass");
    e.apply_scenario_command(1, &pass()).expect("nap pass");
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    inject_card_into_hand(&mut e, 0, "neck_snap");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "neck_snap");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bystander)))
            .is_err(),
        "a noncombatant is not attacking or blocking"
    );
    let slot = hand_index_for_card(&e, 0, "neck_snap");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
        .expect("destroy the attacker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
}

#[test]
fn q47_filtered_destroy_cards_enforce_their_printed_filters() {
    for (index, (id, mana, legal, illegal)) in [
        (
            "defeat",
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            "grizzly_bears",
            "hill_giant",
        ),
        (
            "smite_the_monstrous",
            ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            "juggernaut",
            "grizzly_bears",
        ),
        (
            "daring_demolition",
            ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
            "esikas_chariot",
            "sol_ring",
        ),
        (
            "dreadbore",
            ManaGift {
                b: 1,
                r: 1,
                ..Default::default()
            },
            "jace_beleren",
            "sol_ring",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(47_081_000 + index as u64, &[id], &[legal, illegal]);
        let chosen = fixture_permanent(&mut e, 1, legal);
        let illegal_target = fixture_permanent(&mut e, 1, illegal);
        inject_card_into_hand(&mut e, 0, id);
        give_mana(&mut e, 0, mana);
        let slot = hand_index_for_card(&e, 0, id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(illegal_target)))
                .is_err(),
            "{id}: the illegal target is rejected without spending"
        );
        let slot = hand_index_for_card(&e, 0, id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(chosen)))
            .expect("cast the filtered destruction spell");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&chosen].zone, Zone::Graveyard, "{id}");
    }
}

#[test]
fn q47_filtered_destroy_rechecks_the_filter_on_resolution() {
    let mut e = main1_engine(47_080_003, &["defeat"], &["grizzly_bears"]);
    let bear = fixture_permanent(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "defeat");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "defeat");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Defeat at the 2/2");
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    cast_named(
        &mut e,
        1,
        "giant_growth",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(5));
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Battlefield,
        "the power filter is rechecked on resolution"
    );
}

// ── 09 bite spells ─────────────────────────────────────────────────────────

#[test]
fn q47_bite_canary_simplest_uses_the_sources_power() {
    let mut e = main1_engine(
        47_090_001,
        &["aggressive_instinct", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "aggressive_instinct",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        vec![
            TargetRef {
                object_id: own,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                object_id: opposing,
                group_index: 1,
                ..Default::default()
            },
        ],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own].damage, 0);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
}

#[test]
fn q47_bite_canary_hardest_fizzles_when_either_target_left() {
    let mut e = main1_engine(
        47_090_002,
        &["tail_slash", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "tail_slash");
    let slot = hand_index_for_card(&e, 0, "tail_slash");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![
                TargetRef {
                    object_id: own,
                    group_index: 0,
                    ..Default::default()
                },
                TargetRef {
                    object_id: opposing,
                    group_index: 1,
                    ..Default::default()
                },
            ],
        ),
    )
    .expect("cast Tail Slash");
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    cast_named(
        &mut e,
        1,
        "murder",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        target_object(own),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&opposing].damage, 0,
        "no damage with a stale source"
    );
}

#[test]
fn q47_bite_stale_recipient_also_prevents_damage() {
    let mut e = main1_engine(
        47_090_003,
        &["tail_slash", "grizzly_bears"],
        &["grizzly_bears"],
    );
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "tail_slash");
    let slot = hand_index_for_card(&e, 0, "tail_slash");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![
                TargetRef {
                    object_id: own,
                    group_index: 0,
                    ..Default::default()
                },
                TargetRef {
                    object_id: opposing,
                    group_index: 1,
                    ..Default::default()
                },
            ],
        ),
    )
    .expect("cast Tail Slash");
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    cast_named(
        &mut e,
        1,
        "murder",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        target_object(opposing),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&own].damage, 0,
        "the source deals no damage"
    );
}

#[test]
fn q47_bite_masters_rebuke_can_damage_a_planeswalker() {
    let mut e = main1_engine(
        47_090_004,
        &["masters_rebuke", "grizzly_bears"],
        &["jace_beleren"],
    );
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let jace = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    e.state
        .objects
        .get_mut(&jace)
        .expect("Jace")
        .set_counter(CounterKind::Loyalty, 3);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "masters_rebuke");
    let slot = hand_index_for_card(&e, 0, "masters_rebuke");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![
                TargetRef {
                    object_id: own,
                    group_index: 0,
                    ..Default::default()
                },
                TargetRef {
                    object_id: jace,
                    group_index: 1,
                    ..Default::default()
                },
            ],
        ),
    )
    .expect("Master's Rebuke targets a planeswalker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&jace].counter_count(CounterKind::Loyalty),
        1,
        "two damage removes two loyalty"
    );
}

#[test]
fn q47_bite_creature_only_cards_reject_a_planeswalker() {
    for (index, (id, mana)) in [
        (
            "aggressive_instinct",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "tail_slash",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(
            47_091_100 + index as u64,
            &[id, "grizzly_bears"],
            &["jace_beleren"],
        );
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let jace = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
        inject_card_into_hand(&mut e, 0, id);
        give_mana(&mut e, 0, mana);
        let slot = hand_index_for_card(&e, 0, id);
        assert!(
            e.apply_scenario_command(
                0,
                &cast_spell(
                    slot,
                    vec![
                        TargetRef {
                            object_id: own,
                            group_index: 0,
                            ..Default::default()
                        },
                        TargetRef {
                            object_id: jace,
                            group_index: 1,
                            ..Default::default()
                        },
                    ],
                ),
            )
            .is_err(),
            "{id}: a planeswalker is not a creature target"
        );
    }
}

#[test]
fn q47_bite_uses_the_sources_current_power() {
    let mut e = main1_engine(
        47_090_005,
        &["aggressive_instinct", "grizzly_bears"],
        &["hill_giant"],
    );
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let giant = inject_creature_with_stats(&mut e, 1, "hill_giant", 3, 3);
    e.state
        .objects
        .get_mut(&own)
        .expect("source")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "aggressive_instinct");
    let slot = hand_index_for_card(&e, 0, "aggressive_instinct");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![
                TargetRef {
                    object_id: own,
                    group_index: 0,
                    ..Default::default()
                },
                TargetRef {
                    object_id: giant,
                    group_index: 1,
                    ..Default::default()
                },
            ],
        ),
    )
    .expect("cast Aggressive Instinct");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&giant].zone,
        Zone::Graveyard,
        "three power kills a 3/3"
    );
}

#[test]
fn q47_bite_cards_share_the_printed_shape() {
    for (index, id) in ["aggressive_instinct", "masters_rebuke", "tail_slash"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(
            47_091_000 + index as u64,
            &[id, "grizzly_bears"],
            &["grizzly_bears"],
        );
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let mana = match id {
            "tail_slash" => ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            _ => ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        };
        give_mana(&mut e, 0, mana);
        inject_card_into_hand(&mut e, 0, id);
        let slot = hand_index_for_card(&e, 0, id);
        e.apply_scenario_command(
            0,
            &cast_spell(
                slot,
                vec![
                    TargetRef {
                        object_id: own,
                        group_index: 0,
                        ..Default::default()
                    },
                    TargetRef {
                        object_id: opposing,
                        group_index: 1,
                        ..Default::default()
                    },
                ],
            ),
        )
        .expect("cast the bite spell");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.objects[&own].damage, 0, "{id}");
    }
}
