use crate::helpers::*;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, DevCommand, DevMoveCard,
    DevZone, RuledCommand, SubmitResolutionChoice,
};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn setup_banishing_light() -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine =
        GameEngine::new(10901, &[0, 1], 20, decks, true).expect("Issue #109 cards must validate");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose Banishing Light target");
    (engine, target)
}

fn banishing_light_id(engine: &GameEngine) -> u32 {
    engine
        .state
        .objects
        .values()
        .find(|object| {
            object.card_id == "banishing_light" && object.zone == tricerules_core::Zone::Battlefield
        })
        .expect("Banishing Light on battlefield")
        .id
}

fn cast_broken_wings(engine: &mut GameEngine, source: u32) {
    ensure_card_in_hand(engine, 1, "broken_wings");
    give_mana(
        engine,
        1,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &pass())
        .expect("P0 passes priority");
    let slot = hand_index_for_card(engine, 1, "broken_wings");
    engine
        .apply_command(1, &cast_spell(slot, target_object(source)))
        .expect("cast Broken Wings");
}

fn dev_move(engine: &mut GameEngine, target_player: i32, card_name: &str, zone: DevZone) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: target_player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("dev move");
}

#[test]
fn banishing_light_exiles_the_exact_target_generation() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);
}

#[test]
fn source_leaving_returns_the_card_under_its_owners_control() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    let source = banishing_light_id(&engine);

    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&target].controller, 1);
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn source_leaving_before_the_etb_trigger_resolves_exiles_nothing() {
    let (mut engine, target) = setup_banishing_light();
    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn returning_aura_owner_chooses_a_legal_permanent_to_enchant() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(10902, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let chosen_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "capture_sphere");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Object(first_creature));
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(aura))
        .expect("target Aura");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Exile
    );

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura recipient choice");
    assert_eq!(pending.deciding_player, 1, "the Aura's owner chooses");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&chosen_creature));

    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![chosen_creature],
                    ..Default::default()
                })),
            },
        )
        .expect("choose Aura recipient");

    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            chosen_creature
        ))
    );
}

#[test]
fn exiled_card_that_moves_and_returns_is_not_the_linked_generation() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    let linked_generation = engine.state.zone_change_generation[&target];
    engine.enable_dev_commands();
    dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Hand);
    dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Battlefield);
    assert!(engine.state.zone_change_generation[&target] > linked_generation);

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn exiled_token_ceases_to_exist_and_never_returns() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(10903, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(token))
        .expect("target token");
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn ordinary_return_runs_through_battlefield_entry_replacements() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    let source = banishing_light_id(&engine);

    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&target].tapped);
}

#[test]
fn returning_aura_with_no_legal_recipient_stays_in_exile() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(10904, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "capture_sphere");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Object(first_creature));
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(aura))
        .expect("target Aura");
    resolve_entire_stack_two_player(&mut engine);

    engine.enable_dev_commands();
    dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Graveyard);
    dev_move(&mut engine, 1, "Hill Giant", DevZone::Graveyard);
    assert_eq!(
        engine.state.objects[&first_creature].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&second_creature].zone,
        tricerules_core::Zone::Graveyard
    );

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Exile
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn returning_player_aura_uses_the_typed_player_choice_surface() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(10905, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "curse_of_disturbance");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Player(0));
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(aura))
        .expect("target player Aura");
    resolve_entire_stack_two_player(&mut engine);

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("player Aura choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPlayer);
    assert!(pending.presentation.candidates.contains(&0));

    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![0],
                    ..Default::default()
                })),
            },
        )
        .expect("choose enchanted player");
    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Player(0))
    );
}

#[test]
fn two_sources_destroyed_together_return_both_linked_cards() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["banishing_light", "banishing_light", "tranquility"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(10906, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    for target in [first_target, second_target] {
        ensure_card_in_hand(&mut engine, 0, "banishing_light");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "banishing_light");
        engine
            .apply_command(0, &cast_spell(slot, Vec::new()))
            .expect("cast Banishing Light");
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &choose_trigger_target(target))
            .expect("choose linked target");
        resolve_entire_stack_two_player(&mut engine);
    }
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert_eq!(
        engine.state.objects[&first_target].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.objects[&second_target].zone,
        tricerules_core::Zone::Exile
    );

    ensure_card_in_hand(&mut engine, 0, "tranquility");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "tranquility");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Tranquility");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&first_target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&second_target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.active_event_observers.is_empty());
}
