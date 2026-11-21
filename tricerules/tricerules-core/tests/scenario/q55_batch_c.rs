//! Actual-card scenarios for the queue-v55 batch C cohorts.
//!
//! Expectations are printed Oracle values. Relevant rules: CR 701.14a-b (fight),
//! CR 115.1/115.3 (two separate fight targets), CR 602.1/602.2 (activation and costs),
//! CR 701.9 (discard as a cost), CR 121.1 (draw), CR 110.2/110.5b (reanimation control and
//! untapped entry), CR 702.34a-d (flashback) and CR 608.2b (stale targets fizzle).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CastMethod, CastSpell, TargetRef};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn fight_targets(first: u32, second: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: first,
            group_index: 0,
            ..Default::default()
        },
        TargetRef {
            object_id: second,
            group_index: 1,
            ..Default::default()
        },
    ]
}

fn graveyard_target(object: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: object,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

fn move_battlefield_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility};
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

// ---------------------------------------------------------------------------
// 07-fight-spells

#[test]
fn q55_c_canary_simplest_pounce_makes_two_creatures_fight() {
    let mut e = engine(55_300_001);
    let ours = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "pounce");
    let slot = hand_index_for_card(&e, 0, "pounce");
    e.apply_scenario_command(0, &cast_spell(slot, fight_targets(ours, theirs)))
        .expect("cast Pounce");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&ours].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&theirs].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q55_c_fight_spells_apply_their_riders() {
    // Prizefight: fight then create a Treasure.
    let mut prize = engine(55_301_001);
    let ours = inject_creature_with_stats(&mut prize, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut prize, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut prize,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut prize, 0, "prizefight");
    let slot = hand_index_for_card(&prize, 0, "prizefight");
    prize
        .apply_scenario_command(0, &cast_spell(slot, fight_targets(ours, theirs)))
        .expect("cast Prizefight");
    resolve_entire_stack_two_player(&mut prize);
    assert_eq!(prize.state.objects[&ours].zone, Zone::Graveyard);
    assert_eq!(prize.state.objects[&theirs].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&prize, 0, "treasure").len(), 1);

    // Go for Blood: fight then cycle for {1}.
    let mut blood = engine(55_301_002);
    let ours = inject_creature_with_stats(&mut blood, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut blood, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut blood,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut blood, 0, "go_for_blood");
    let slot = hand_index_for_card(&blood, 0, "go_for_blood");
    blood
        .apply_scenario_command(0, &cast_spell(slot, fight_targets(ours, theirs)))
        .expect("cast Go for Blood");
    resolve_entire_stack_two_player(&mut blood);
    assert_eq!(blood.state.objects[&ours].zone, Zone::Graveyard);
    assert_eq!(blood.state.objects[&theirs].zone, Zone::Graveyard);

    let cycle_card = inject_card_into_hand(&mut blood, 0, "go_for_blood");
    let drawn = inject_library_card(&mut blood, 0, "forest");
    blood.state.players[0].library.retain(|oid| *oid != drawn);
    blood.state.players[0].library.push_front(drawn);
    give_mana(
        &mut blood,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    blood
        .apply_scenario_command(0, &hand_ability(&blood, cycle_card, 0))
        .expect("cycle Go for Blood");
    resolve_entire_stack_two_player(&mut blood);
    assert_eq!(blood.state.objects[&cycle_card].zone, Zone::Graveyard);
    assert_eq!(blood.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn q55_c_fight_spells_do_no_damage_when_either_target_is_illegal() {
    let mut e = engine(55_302_001);
    let ours = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "pounce");
    let slot = hand_index_for_card(&e, 0, "pounce");
    e.apply_scenario_command(0, &cast_spell(slot, fight_targets(ours, theirs)))
        .expect("cast Pounce");
    move_battlefield_to_graveyard(&mut e, 1, theirs);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&ours].damage, 0);
    assert_eq!(e.state.objects[&ours].zone, Zone::Battlefield);

    // The other target leaves: the remaining fighter still deals no damage.
    let mut ours_gone = engine(55_302_002);
    let ours = inject_creature_with_stats(&mut ours_gone, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut ours_gone, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut ours_gone,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut ours_gone, 0, "go_for_blood");
    let slot = hand_index_for_card(&ours_gone, 0, "go_for_blood");
    ours_gone
        .apply_scenario_command(0, &cast_spell(slot, fight_targets(ours, theirs)))
        .expect("cast Go for Blood");
    move_battlefield_to_graveyard(&mut ours_gone, 0, ours);
    resolve_entire_stack_two_player(&mut ours_gone);
    assert_eq!(ours_gone.state.objects[&theirs].damage, 0);
    assert_eq!(ours_gone.state.objects[&theirs].zone, Zone::Battlefield);
}

// ---------------------------------------------------------------------------
// 08-discard-draw

#[test]
fn q55_c_canary_simplest_goblin_picker_taps_discards_and_draws() {
    let mut e = engine(55_310_001);
    let picker = inject_creature_on_battlefield(&mut e, 0, "goblin_picker");
    let fodder = inject_card_into_hand(&mut e, 0, "forest");
    let drawn = inject_library_card(&mut e, 0, "forest");
    e.state.players[0].library.retain(|oid| *oid != drawn);
    e.state.players[0].library.push_front(drawn);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let fodder_index = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == fodder)
        .expect("discard fodder");
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(
            picker,
            0,
            vec![],
            vec![hand_cost_selection(2, fodder_index as u32)],
        ),
    )
    .expect("activate Goblin Picker");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&picker].tapped);
    assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact {{R}} cost");
}

#[test]
fn q55_c_discard_draw_enchantments_use_their_printed_costs() {
    for (index, (id, mana)) in [
        (
            "mental_discipline",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "oread_of_mountains_blaze",
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
        let mut e = engine(55_311_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, id);
        let fodder = inject_card_into_hand(&mut e, 0, "forest");
        let drawn = inject_library_card(&mut e, 0, "forest");
        e.state.players[0].library.retain(|oid| *oid != drawn);
        e.state.players[0].library.push_front(drawn);
        give_mana(&mut e, 0, mana);
        let fodder_index = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == fodder)
            .expect("discard fodder");
        e.apply_scenario_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![hand_cost_selection(1, fodder_index as u32)],
            ),
        )
        .unwrap_or_else(|error| panic!("{id}: activate: {error:?}"));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{id}");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact cost");
    }
}

#[test]
fn q55_c_discard_draw_rejects_activation_without_a_card_to_discard() {
    let mut e = engine(55_312_001);
    assert!(
        tricerules_cards::registry::global()
            .get("mental_discipline")
            .is_some(),
        "Mental Discipline must be implemented"
    );
    let source = inject_permanent_on_battlefield(&mut e, 0, "mental_discipline");
    e.state.players[0].hand.clear();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_with_costs(source, 0, vec![], vec![]),)
            .is_err(),
        "the discard cost cannot be paid from an empty hand"
    );
    assert_eq!(
        pool(&e, 0),
        (0, 1, 0, 0, 0, 1),
        "rejected activation keeps mana"
    );
    assert!(!e.state.objects[&source].tapped);
}

// ---------------------------------------------------------------------------
// 09-reanimate-battlefield

fn flashback_cast(spell: u32, generation: u64, target: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: graveyard_target(target),
            ..Default::default()
        })),
    }
}

#[test]
fn q55_c_canary_simplest_argivian_restoration_returns_an_artifact() {
    let mut e = engine(55_320_001);
    let artifact = inject_graveyard_card(&mut e, 0, "+2_mace");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "argivian_restoration");
    let slot = hand_index_for_card(&e, 0, "argivian_restoration");
    e.apply_scenario_command(0, &cast_spell(slot, graveyard_target(artifact)))
        .expect("cast Argivian Restoration");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&artifact].controller, 0);
    assert!(!e.state.objects[&artifact].tapped);
}

#[test]
fn q55_c_reanimation_uses_the_printed_graveyard_and_controller() {
    // Refurbish returns your artifact.
    let mut refurbish = engine(55_321_001);
    let artifact = inject_graveyard_card(&mut refurbish, 0, "+2_mace");
    give_mana(
        &mut refurbish,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut refurbish, 0, "refurbish");
    let slot = hand_index_for_card(&refurbish, 0, "refurbish");
    refurbish
        .apply_scenario_command(0, &cast_spell(slot, graveyard_target(artifact)))
        .expect("cast Refurbish");
    resolve_entire_stack_two_player(&mut refurbish);
    assert_eq!(refurbish.state.objects[&artifact].zone, Zone::Battlefield);

    // Hymn of Rebirth may take a creature from any graveyard, under the caster's control.
    let mut hymn = engine(55_321_002);
    let creature = inject_graveyard_card(&mut hymn, 1, "grizzly_bears");
    give_mana(
        &mut hymn,
        0,
        ManaGift {
            g: 1,
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut hymn, 0, "hymn_of_rebirth");
    let slot = hand_index_for_card(&hymn, 0, "hymn_of_rebirth");
    hymn.apply_scenario_command(0, &cast_spell(slot, graveyard_target(creature)))
        .expect("cast Hymn of Rebirth");
    resolve_entire_stack_two_player(&mut hymn);
    assert_eq!(hymn.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(hymn.state.objects[&creature].controller, 0);
    assert_eq!(hymn.state.objects[&creature].owner, 1);
}

#[test]
fn q55_c_reanimation_rejects_wrong_card_types_and_stale_targets() {
    let mut e = engine(55_322_001);
    let spell = inject_graveyard_card(&mut e, 0, "divination");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "argivian_restoration");
    let slot = hand_index_for_card(&e, 0, "argivian_restoration");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, graveyard_target(spell)))
            .is_err(),
        "Argivian Restoration only targets artifact cards"
    );
    assert_eq!(pool(&e, 0), (0, 2, 0, 0, 0, 2), "rejected cast keeps mana");

    let mut stale = engine(55_322_002);
    let artifact = inject_graveyard_card(&mut stale, 0, "+2_mace");
    give_mana(
        &mut stale,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut stale, 0, "refurbish");
    let slot = hand_index_for_card(&stale, 0, "refurbish");
    stale
        .apply_scenario_command(0, &cast_spell(slot, graveyard_target(artifact)))
        .expect("cast Refurbish");
    // The graveyard card leaves before resolution.
    stale.state.players[0]
        .graveyard
        .retain(|oid| *oid != artifact);
    stale.state.players[0].hand.push(artifact);
    stale
        .state
        .objects
        .get_mut(&artifact)
        .expect("artifact")
        .zone = Zone::Hand;
    *stale
        .state
        .zone_change_generation
        .entry(artifact)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&artifact].zone, Zone::Hand);
    assert!(!stale.state.players[0].battlefield.contains(&artifact));
}

#[test]
fn q55_c_unburial_rites_flashes_back_then_exiles() {
    let mut e = engine(55_323_001);
    let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 4,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "unburial_rites");
    let slot = hand_index_for_card(&e, 0, "unburial_rites");
    e.apply_scenario_command(0, &cast_spell(slot, graveyard_target(creature)))
        .expect("cast Unburial Rites");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    let rites = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == "unburial_rites")
        .expect("Unburial Rites in graveyard");
    let generation = e
        .state
        .zone_change_generation
        .get(&rites)
        .copied()
        .unwrap_or(0);
    let second = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &flashback_cast(rites, generation, second))
        .expect("flash back Unburial Rites");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&second].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&rites].zone, Zone::Exile);
}
