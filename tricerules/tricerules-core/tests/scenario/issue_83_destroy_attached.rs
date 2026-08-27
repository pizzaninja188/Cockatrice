//! Issue #83: resolving effects over the Aura/Equipment cohort attached to one target.

use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn attach(engine: &mut GameEngine, attachment: u32, target: u32) {
    engine
        .state
        .objects
        .get_mut(&attachment)
        .expect("attachment")
        .attached_to = Some(AttachmentRecipient::Object(target));
}

fn cast_turn_to_slag(engine: &mut GameEngine, target: u32) {
    relocate_to_hand(engine, 0, "turn_to_slag");
    give_mana(
        engine,
        0,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(engine, 0, "turn_to_slag");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Turn to Slag");
}

#[test]
fn turn_to_slag_destroys_equipment_but_not_auras_attached_to_its_target() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["turn_to_slag", "holy_strength", "short_sword"],
        ),
        deck_with("forest", &["colossal_dreadmaw", "bonesplitter"]),
    ]);
    let mut engine = GameEngine::new(8301, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    let bonesplitter = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    let short_sword = relocate_to_battlefield(&mut engine, 0, "short_sword", false);
    let aura = relocate_to_battlefield(&mut engine, 0, "holy_strength", false);
    for attachment in [bonesplitter, short_sword, aura] {
        attach(&mut engine, attachment, target);
    }

    cast_turn_to_slag(&mut engine, target);
    let batch = {
        engine.apply_command(0, &pass()).expect("caster pass");
        engine.apply_command(1, &pass()).expect("opponent pass")
    };

    assert_eq!(engine.state.objects[&target].damage, 5);
    assert_eq!(engine.state.objects[&bonesplitter].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&bonesplitter));
    assert_eq!(engine.state.objects[&short_sword].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&short_sword));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(target))
    );
    let mut expected_move_order = vec![bonesplitter, short_sword];
    expected_move_order.sort_unstable();
    let actual_move_order = permanents_moved_in(&batch)
        .into_iter()
        .map(|event| event.object_id)
        .collect::<Vec<_>>();
    assert_eq!(actual_move_order, expected_move_order);
}

#[test]
fn turn_to_slag_allows_a_target_with_no_equipment() {
    let decks = Some(vec![
        deck_with("mountain", &["turn_to_slag"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(8302, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);

    cast_turn_to_slag(&mut engine, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].damage, 5);
}

#[test]
fn illegal_turn_to_slag_target_fizzles_without_destroying_equipment() {
    let decks = Some(vec![
        deck_with("mountain", &["turn_to_slag"]),
        deck_with("forest", &["colossal_dreadmaw", "bonesplitter"]),
    ]);
    let mut engine = GameEngine::new(8303, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    let equipment = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    attach(&mut engine, equipment, target);
    cast_turn_to_slag(&mut engine, target);

    engine.state.players[1]
        .battlefield
        .retain(|&oid| oid != target);
    engine.state.players[1].hand.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&equipment].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].damage, 0);
}

#[test]
fn attachment_cohort_is_recomputed_when_turn_to_slag_resolves() {
    let decks = Some(vec![
        deck_with("mountain", &["turn_to_slag", "short_sword"]),
        deck_with(
            "forest",
            &["colossal_dreadmaw", "grizzly_bears", "bonesplitter"],
        ),
    ]);
    let mut engine = GameEngine::new(8304, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    let other = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let moved_away = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    let newly_attached = relocate_to_battlefield(&mut engine, 0, "short_sword", false);
    attach(&mut engine, moved_away, target);
    attach(&mut engine, newly_attached, other);
    cast_turn_to_slag(&mut engine, target);

    attach(&mut engine, moved_away, other);
    attach(&mut engine, newly_attached, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&moved_away].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&newly_attached].zone, Zone::Graveyard);
}

#[test]
fn destroy_attached_honors_indestructible_and_regeneration() {
    let decks = Some(vec![
        deck_with("mountain", &["turn_to_slag", "short_sword"]),
        deck_with("forest", &["colossal_dreadmaw", "bonesplitter"]),
    ]);
    let mut engine = GameEngine::new(8305, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    let indestructible = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    let regenerating = relocate_to_battlefield(&mut engine, 0, "short_sword", false);
    attach(&mut engine, indestructible, target);
    attach(&mut engine, regenerating, target);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(indestructible),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Indestructible),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    engine
        .state
        .objects
        .get_mut(&regenerating)
        .expect("regenerating equipment")
        .regeneration_shields = 1;

    cast_turn_to_slag(&mut engine, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&indestructible].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&regenerating].zone, Zone::Battlefield);
    assert!(engine.state.objects[&regenerating].tapped);
    assert_eq!(engine.state.objects[&regenerating].regeneration_shields, 0);
}

#[test]
fn lethal_damage_does_not_detach_equipment_before_turn_to_slag_destroys_it() {
    let decks = Some(vec![
        deck_with("mountain", &["turn_to_slag"]),
        deck_with("forest", &["grizzly_bears", "bonesplitter"]),
    ]);
    let mut engine = GameEngine::new(8306, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let equipment = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    attach(&mut engine, equipment, target);

    cast_turn_to_slag(&mut engine, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Graveyard);
}
