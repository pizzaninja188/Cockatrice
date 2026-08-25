use crate::helpers::*;
use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};
use tricerules_core::state::{
    ActiveEventObserver, DelayedTriggerPayload, EventObserverMatcher, EventObserverPayload,
    TriggerObjectRef,
};

#[test]
fn infernal_scarring_grants_the_creature_controller_a_dies_trigger() {
    let decks = Some(vec![
        deck_with("swamp", &["infernal_scarring"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6401, &[0, 1], 20, decks, true)
        .expect("Infernal Scarring card data must validate");
    advance_to_main1_from_game_start(&mut e);

    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_library_card(&mut e, 1, "forest");
    ensure_card_in_hand(&mut e, 0, "infernal_scarring");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let aura_slot = hand_index_for_card(&e, 0, "infernal_scarring");
    e.apply_command(0, &cast_spell(aura_slot, target_object(creature)))
        .expect("cast Infernal Scarring");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.effective_power(creature), Some(4));
    let hand_before = e.state.players[1].hand.len();
    e.state
        .objects
        .get_mut(&creature)
        .expect("enchanted creature")
        .damage = 2;
    e.apply_command(0, &pass()).expect("lethal damage SBA");

    assert_eq!(
        e.state.stack.len(),
        1,
        "the granted dies trigger must survive the creature and Aura leaving"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].hand.len(), hand_before + 1);
}

#[test]
fn multiple_infernal_scarrings_create_distinct_orderable_triggers() {
    let decks = Some(vec![
        deck_with("swamp", &["infernal_scarring", "infernal_scarring"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6408, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_library_card(&mut e, 1, "forest");
    inject_library_card(&mut e, 1, "forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            c: 2,
            ..Default::default()
        },
    );
    for _ in 0..2 {
        ensure_card_in_hand(&mut e, 0, "infernal_scarring");
        let aura = hand_index_for_card(&e, 0, "infernal_scarring");
        e.apply_command(0, &cast_spell(aura, target_object(creature)))
            .expect("cast Infernal Scarring");
        resolve_entire_stack_two_player(&mut e);
    }

    let hand_before = e.state.players[1].hand.len();
    e.state.objects.get_mut(&creature).expect("creature").damage = 2;
    e.apply_command(0, &pass()).expect("lethal damage SBA");
    let ordering = e
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the creature controller orders both granted triggers");
    assert_eq!(ordering.deciding_player, 1);
    assert_eq!(ordering.candidates.len(), 2);
    answer_trigger_order_in_engine_order(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].hand.len(), hand_before + 2);
}

#[test]
fn infernal_scarring_removed_before_death_no_longer_grants_the_trigger() {
    let decks = Some(vec![
        deck_with("swamp", &["infernal_scarring", "tranquility"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6409, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    for card in ["infernal_scarring", "tranquility"] {
        ensure_card_in_hand(&mut e, 0, card);
    }
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let aura = hand_index_for_card(&e, 0, "infernal_scarring");
    e.apply_command(0, &cast_spell(aura, target_object(creature)))
        .expect("cast Infernal Scarring");
    resolve_entire_stack_two_player(&mut e);
    let tranquility = hand_index_for_card(&e, 0, "tranquility");
    e.apply_command(0, &cast_spell(tranquility, vec![]))
        .expect("cast Tranquility");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(creature), Some(2));

    e.state.objects.get_mut(&creature).expect("creature").damage = 2;
    e.apply_command(0, &pass()).expect("lethal damage SBA");
    assert!(e.state.stack.is_empty());
    assert!(e.state.pending_trigger_order.is_none());
}

#[test]
fn abnormal_endurance_returns_the_exact_dead_creature_tapped_under_its_owner() {
    let decks = Some(vec![
        deck_with("swamp", &["abnormal_endurance"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6402, &[0, 1], 20, decks, true)
        .expect("Abnormal Endurance card data must validate");
    advance_to_main1_from_game_start(&mut e);

    let creature = inject_creature_under_foreign_control(&mut e, 1, 0, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, "abnormal_endurance");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(&e, 0, "abnormal_endurance");
    e.apply_command(0, &cast_spell(spell_slot, target_object(creature)))
        .expect("cast Abnormal Endurance");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(creature), Some(4));

    let generation_before = e
        .state
        .zone_change_generation
        .get(&creature)
        .copied()
        .unwrap_or(0);
    e.state
        .objects
        .get_mut(&creature)
        .expect("affected creature")
        .damage = 2;
    e.apply_command(0, &pass()).expect("lethal damage SBA");
    assert_eq!(e.state.stack.len(), 1, "the granted dies trigger fires");
    resolve_entire_stack_two_player(&mut e);

    let returned = e.state.objects.get(&creature).expect("returned creature");
    assert_eq!(returned.zone, tricerules_core::Zone::Battlefield);
    assert!(returned.tapped);
    assert_eq!(
        returned.controller, 1,
        "it returns under its owner's control"
    );
    assert_eq!(
        e.state
            .zone_change_generation
            .get(&creature)
            .copied()
            .unwrap_or(0),
        generation_before + 2,
        "death and return are distinct zone changes"
    );
}

#[test]
fn abnormal_endurance_does_not_return_a_card_that_left_the_graveyard() {
    let decks = Some(vec![
        deck_with("swamp", &["abnormal_endurance"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6405, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, "abnormal_endurance");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&e, 0, "abnormal_endurance");
    e.apply_command(0, &cast_spell(spell, target_object(creature)))
        .expect("cast Abnormal Endurance");
    resolve_entire_stack_two_player(&mut e);
    e.state.objects.get_mut(&creature).expect("creature").damage = 2;
    e.apply_command(0, &pass()).expect("lethal damage SBA");
    assert_eq!(e.state.stack.len(), 1);

    e.state.players[0].graveyard.retain(|&id| id != creature);
    e.state.players[0].exile.push(creature);
    e.state.objects.get_mut(&creature).expect("creature").zone = tricerules_core::Zone::Exile;
    *e.state.zone_change_generation.entry(creature).or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        tricerules_core::Zone::Exile
    );
}

#[test]
fn abnormal_endurance_granted_trigger_expires_at_cleanup() {
    let decks = Some(vec![
        deck_with("swamp", &["abnormal_endurance"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6407, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, "abnormal_endurance");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&e, 0, "abnormal_endurance");
    e.apply_command(0, &cast_spell(spell, target_object(creature)))
        .expect("cast Abnormal Endurance");
    resolve_entire_stack_two_player(&mut e);
    end_active_turn(&mut e, 0);

    e.state.objects.get_mut(&creature).expect("creature").damage = 2;
    e.apply_command(1, &pass())
        .expect("next-turn lethal damage SBA");
    assert_eq!(
        e.state.objects[&creature].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(
        e.state.stack.is_empty(),
        "the until-end-of-turn grant expired"
    );
}

#[test]
fn ray_of_command_control_loss_trigger_uses_the_stack_during_cleanup() {
    let decks = Some(vec![
        deck_with("island", &["ray_of_command"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6403, &[0, 1], 20, decks, true)
        .expect("Ray of Command card data must validate");
    advance_to_main1_from_game_start(&mut e);

    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&creature).expect("target").tapped = true;
    ensure_card_in_hand(&mut e, 0, "ray_of_command");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(&e, 0, "ray_of_command");
    e.apply_command(0, &cast_spell(spell_slot, target_object(creature)))
        .expect("cast Ray of Command");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature].controller, 0);
    assert!(!e.state.objects[&creature].tapped);
    assert!(e.effective_has_keyword(creature, tricerules_cards::Keyword::Haste));

    end_active_turn(&mut e, 0);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);
    assert_eq!(e.state.objects[&creature].controller, 1);
    assert!(!e.state.objects[&creature].tapped);
    assert_eq!(
        e.state.stack.len(),
        1,
        "control loss triggers during cleanup"
    );

    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&creature].tapped);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::Cleanup,
        "the game performs another cleanup only after the priority window"
    );
    pass_both_players(&mut e);
    assert_ne!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);
}

#[test]
fn ray_of_command_triggers_once_on_an_earlier_control_change() {
    let decks = Some(vec![
        deck_with("island", &["ray_of_command"]),
        deck_with("island", &["ray_of_command"]),
    ]);
    let mut e = GameEngine::new(6404, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    for player in [0, 1] {
        ensure_card_in_hand(&mut e, player, "ray_of_command");
        give_mana(
            &mut e,
            player as i32,
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        );
    }

    let first = hand_index_for_card(&e, 0, "ray_of_command");
    e.apply_command(0, &cast_spell(first, target_object(creature)))
        .expect("first Ray");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature].controller, 0);

    e.apply_command(0, &pass()).expect("P0 passes priority");
    let second = hand_index_for_card(&e, 1, "ray_of_command");
    e.apply_command(1, &cast_spell(second, target_object(creature)))
        .expect("second Ray");
    e.apply_command(1, &pass())
        .expect("second Ray controller passes");
    e.apply_command(0, &pass()).expect("second Ray resolves");

    assert_eq!(e.state.objects[&creature].controller, 1);
    assert_eq!(
        e.state.stack.len(),
        1,
        "the first Ray's control-loss trigger is waiting on the stack"
    );
    assert!(!e.state.objects[&creature].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&creature].tapped);
    assert_eq!(
        e.state.active_event_observers.len(),
        1,
        "the consumed first delayed trigger is gone; only the second Ray remains"
    );
}

#[test]
fn next_end_step_delayed_trigger_is_one_shot_and_keeps_object_identity() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(6406, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let generation = e
        .state
        .zone_change_generation
        .get(&creature)
        .copied()
        .unwrap_or(0);
    e.state.active_event_observers.push(ActiveEventObserver {
        watched: TriggerObjectRef {
            object_id: creature,
            zone_change_generation: generation,
            controller_at_event: 0,
        },
        matcher: EventObserverMatcher::AtBeginningOfNextEndStep,
        payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
            controller: 0,
            card_id: "grizzly_bears".into(),
            card_name: "Grizzly Bears".into(),
            source_face_index: 0,
            ability: TriggeredAbilityDef {
                trigger: TriggerCondition::AtBeginningOfNextEndStep,
                effect: vec![SpellEffectKind::Tap {
                    subject: EffectSubject::TriggerObject,
                }],
                modal: None,
                targeting: None,
                text: "At the beginning of the next end step, tap it.".into(),
                may: false,
                intervening_if: None,
                triggers_only_once: false,
            },
        })),
    });

    e.apply_command(0, &primitive_yield())
        .expect("main1 to combat");
    e.apply_command(0, &primitive_yield())
        .expect("begin combat");
    if e.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        e.apply_command(0, &primitive_yield())
            .expect("skip attackers");
    }
    e.apply_command(0, &primitive_yield())
        .expect("end combat to main2");
    e.apply_command(0, &primitive_yield())
        .expect("main2 to end step");

    assert_eq!(e.state.stack.len(), 1);
    assert!(e.state.active_event_observers.is_empty());
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&creature].tapped);
}
