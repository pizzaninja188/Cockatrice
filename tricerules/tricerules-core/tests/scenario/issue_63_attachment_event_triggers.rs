//! Issue #63: triggers from the object or player carrying an attachment.
//!
//! Oracle/rulings verified 2026-08-14 for Heart-Piercer Bow, Unholy Indenture,
//! Curse of Opulence, and Curse of Disturbance. Governing rules: CR 113.7a,
//! 113.8, 400.7, 508.1m, 508.3, 603.3, 603.6, 603.10, 608.2b, and 704.5.

use crate::helpers::*;
use tricerules_cards::CounterKind;

fn choose_trigger_target(target: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(target),
        })),
    }
}

#[test]
fn issue_63_heart_piercer_bow_targets_only_the_defending_players_creature() {
    let decks = Some(vec![
        deck_with("mountain", &["heart-piercer_bow"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(6301, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut engine);

    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "grizzly_bears")
        .expect("eligible attacker");
    let bow = relocate_to_battlefield(&mut engine, 0, "heart-piercer_bow", false);
    let defending_creature = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.objects.get_mut(&bow).expect("bow").attached_to =
        Some(AttachmentRecipient::Object(attacker));

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare equipped attacker");
    assert_eq!(engine.state.pending_triggers.len(), 1);

    let error = engine
        .apply_command(0, &choose_trigger_target(attacker))
        .expect_err("the attacking player's creature is not controlled by the defender");
    assert!(error.to_string().contains("target"), "unexpected: {error}");
    assert_eq!(engine.state.pending_triggers.len(), 1);

    engine
        .apply_command(0, &choose_trigger_target(defending_creature))
        .expect("choose defending player's creature");
    engine.state.objects.get_mut(&bow).expect("bow").attached_to = None;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&defending_creature].damage, 1,
        "the independent Bow trigger resolves after detachment"
    );
}

#[test]
fn issue_63_bow_requires_the_event_attachment_and_revalidates_defender_control() {
    let decks = || {
        Some(vec![
            deck_with("mountain", &["heart-piercer_bow"]),
            deck_with("forest", &["grizzly_bears"]),
        ])
    };

    let mut unattached_engine =
        GameEngine::new(6308, &[0, 1], 20, decks(), true).expect("unattached engine");
    advance_to_declare_attackers(&mut unattached_engine);
    let unattached_attacker = unattached_engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| unattached_engine.state.objects[oid].card_id == "grizzly_bears")
        .expect("attacker");
    relocate_to_battlefield(&mut unattached_engine, 0, "heart-piercer_bow", false);
    unattached_engine
        .apply_command(0, &declare_attackers(vec![unattached_attacker]))
        .expect("declare attacker without the Bow");
    assert!(
        unattached_engine.state.pending_triggers.is_empty(),
        "an unattached Bow does not observe the attack"
    );

    let mut stale_engine =
        GameEngine::new(6309, &[0, 1], 20, decks(), true).expect("stale-target engine");
    advance_to_declare_attackers(&mut stale_engine);
    let attacker = stale_engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| stale_engine.state.objects[oid].card_id == "grizzly_bears")
        .expect("attacker");
    let bow = relocate_to_battlefield(&mut stale_engine, 0, "heart-piercer_bow", false);
    let target = relocate_to_battlefield(&mut stale_engine, 1, "grizzly_bears", false);
    stale_engine
        .state
        .objects
        .get_mut(&bow)
        .unwrap()
        .attached_to = Some(AttachmentRecipient::Object(attacker));
    stale_engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare equipped attacker");
    stale_engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("target defender's creature");

    stale_engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    stale_engine.state.players[0].battlefield.push(target);
    let target_object = stale_engine.state.objects.get_mut(&target).unwrap();
    target_object.base_controller = 0;
    target_object.controller = 0;
    resolve_entire_stack_two_player(&mut stale_engine);
    assert_eq!(
        stale_engine.state.objects[&target].damage, 0,
        "a target no longer controlled by the event-time defender is illegal at resolution"
    );
}

#[test]
fn issue_63_unholy_indenture_returns_the_exact_card_with_an_entry_counter() {
    let decks = Some(vec![
        deck_with("swamp", &["unholy_indenture"]),
        deck_with("forest", &["squad_captain"]),
    ]);
    let mut engine = GameEngine::new(6302, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let indenture = relocate_to_battlefield(&mut engine, 0, "unholy_indenture", false);
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let creature = relocate_to_battlefield(&mut engine, 1, "squad_captain", false);
    engine
        .state
        .objects
        .get_mut(&indenture)
        .expect("Indenture")
        .attached_to = Some(AttachmentRecipient::Object(creature));
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .damage = 20;

    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("priority pass applies lethal-damage SBAs");
    resolve_entire_stack_two_player(&mut engine);

    let returned = engine
        .state
        .objects
        .get(&creature)
        .expect("returned creature");
    assert_eq!(returned.zone, tricerules_core::Zone::Battlefield);
    assert_eq!(
        returned.controller, 0,
        "the Aura controller gets the creature"
    );
    assert_eq!(
        returned.counters.get(&CounterKind::PlusOnePlusOne),
        Some(&2),
        "Indenture's counter composes with Squad Captain's entry replacement"
    );
    assert_eq!(
        engine.state.objects[&indenture].zone,
        tricerules_core::Zone::Graveyard,
        "Unholy Indenture does not return with the creature"
    );
}

#[test]
fn issue_63_multiple_attachment_triggers_keep_apnap_and_attachment_identity() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["heart-piercer_bow", "heart-piercer_bow", "unholy_indenture"],
        ),
        deck_with("forest", &["unholy_indenture", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(6304, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut engine);

    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "grizzly_bears")
        .expect("attacker");
    let first_bow = relocate_to_battlefield(&mut engine, 0, "heart-piercer_bow", false);
    let second_bow = relocate_to_battlefield(&mut engine, 0, "heart-piercer_bow", false);
    for bow in [first_bow, second_bow] {
        engine.state.objects.get_mut(&bow).expect("bow").attached_to =
            Some(AttachmentRecipient::Object(attacker));
    }
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker carrying two Bows");
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("two Bow triggers need ordering")
            .candidates
            .len(),
        2
    );

    let mut death_engine = GameEngine::new(
        6305,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["unholy_indenture"]),
            deck_with("forest", &["unholy_indenture", "grizzly_bears"]),
        ]),
        true,
    )
    .expect("death engine");
    advance_to_main1_from_game_start(&mut death_engine);
    let active_aura = relocate_to_battlefield(&mut death_engine, 0, "unholy_indenture", false);
    let nonactive_aura = relocate_to_battlefield(&mut death_engine, 1, "unholy_indenture", false);
    let creature = relocate_to_battlefield(&mut death_engine, 1, "grizzly_bears", false);
    for aura in [active_aura, nonactive_aura] {
        death_engine
            .state
            .objects
            .get_mut(&aura)
            .expect("Indenture")
            .attached_to = Some(AttachmentRecipient::Object(creature));
    }
    death_engine
        .state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .damage = 2;
    let priority = death_engine.state.priority_player_id();
    death_engine
        .apply_command(priority, &pass())
        .expect("apply death SBAs");
    assert_eq!(death_engine.state.stack.len(), 2);

    resolve_entire_stack_two_player(&mut death_engine);
    assert_eq!(
        death_engine.state.objects[&creature].controller, 1,
        "the nonactive player's trigger is on top and returns the creature first"
    );
    assert_eq!(
        death_engine.state.objects[&creature]
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1),
        "the later trigger finds no card and adds no second counter"
    );
}

#[test]
fn issue_63_unholy_indenture_rejects_tokens_and_stale_graveyard_generations() {
    let make_engine = |seed| {
        GameEngine::new(
            seed,
            &[0, 1],
            20,
            Some(vec![
                deck_with("swamp", &["unholy_indenture"]),
                deck_with("forest", &["grizzly_bears"]),
            ]),
            true,
        )
        .expect("engine")
    };

    let mut token_engine = make_engine(6306);
    advance_to_main1_from_game_start(&mut token_engine);
    let aura = relocate_to_battlefield(&mut token_engine, 0, "unholy_indenture", false);
    let token = inject_creature_on_battlefield(&mut token_engine, 1, "zombie_b_2_2");
    token_engine
        .state
        .objects
        .get_mut(&aura)
        .unwrap()
        .attached_to = Some(AttachmentRecipient::Object(token));
    token_engine.state.objects.get_mut(&token).unwrap().damage = 2;
    let priority = token_engine.state.priority_player_id();
    token_engine
        .apply_command(priority, &pass())
        .expect("token dies");
    resolve_entire_stack_two_player(&mut token_engine);
    assert!(
        !token_engine.state.objects.contains_key(&token),
        "a token ceases to exist and cannot return"
    );

    let mut stale_engine = make_engine(6307);
    advance_to_main1_from_game_start(&mut stale_engine);
    let aura = relocate_to_battlefield(&mut stale_engine, 0, "unholy_indenture", false);
    let creature = relocate_to_battlefield(&mut stale_engine, 1, "grizzly_bears", false);
    stale_engine
        .state
        .objects
        .get_mut(&aura)
        .unwrap()
        .attached_to = Some(AttachmentRecipient::Object(creature));
    stale_engine
        .state
        .objects
        .get_mut(&creature)
        .unwrap()
        .damage = 2;
    let priority = stale_engine.state.priority_player_id();
    stale_engine
        .apply_command(priority, &pass())
        .expect("creature dies");

    stale_engine.state.players[1]
        .graveyard
        .retain(|object_id| *object_id != creature);
    stale_engine.state.players[1].hand.push(creature);
    stale_engine.state.objects.get_mut(&creature).unwrap().zone = tricerules_core::Zone::Hand;
    *stale_engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 1;
    stale_engine.state.players[1]
        .hand
        .retain(|object_id| *object_id != creature);
    stale_engine.state.players[1].graveyard.push(creature);
    stale_engine.state.objects.get_mut(&creature).unwrap().zone = tricerules_core::Zone::Graveyard;
    *stale_engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 1;

    resolve_entire_stack_two_player(&mut stale_engine);
    assert_eq!(
        stale_engine.state.objects[&creature].zone,
        tricerules_core::Zone::Graveyard,
        "the leave-and-return graveyard object is not the card observed by the trigger"
    );
}
