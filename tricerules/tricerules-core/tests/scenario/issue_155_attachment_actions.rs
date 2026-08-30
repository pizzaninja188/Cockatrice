use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    permanent_moved, ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChooseTriggerTarget,
    RuledCommand,
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

fn cast_and_resolve_aura(
    engine: &mut GameEngine,
    card_id: &str,
    target: u32,
    mana: ManaGift,
) -> u32 {
    ensure_card_in_hand(engine, 0, card_id);
    give_mana(engine, 0, mana);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast issue #155 Aura");
    resolve_entire_stack_two_player(engine);
    battlefield_object_for_card(engine, 0, card_id)
}

#[test]
fn issue_155_light_jammer_etb_attaches_and_grants_hexproof() {
    let decks = Some(vec![
        deck_with("island", &["illvoi_light_jammer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine =
        GameEngine::new(155_001, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "illvoi_light_jammer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "illvoi_light_jammer");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Illvoi Light Jammer");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(creature))
        .expect("choose the ETB attachment target");
    pass_both_players(&mut engine);

    let equipment = battlefield_object_for_card(&engine, 0, "illvoi_light_jammer");
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    assert!(engine.effective_has_keyword(creature, tricerules_cards::Keyword::Hexproof));
    assert_eq!(engine.effective_power(creature), Some(3));
    assert_eq!(engine.effective_toughness(creature), Some(4));
}

#[test]
fn issue_155_new_equipment_generation_does_not_receive_the_old_attach_effect() {
    let decks = Some(vec![
        deck_with("island", &["illvoi_light_jammer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine =
        GameEngine::new(155_008, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "illvoi_light_jammer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "illvoi_light_jammer");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Illvoi Light Jammer");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(creature))
        .expect("choose the ETB attachment target");
    let equipment = battlefield_object_for_card(&engine, 0, "illvoi_light_jammer");
    *engine
        .state
        .zone_change_generation
        .entry(equipment)
        .or_default() += 1;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert!(engine.effective_has_keyword(creature, tricerules_cards::Keyword::Hexproof));
    assert_eq!(engine.effective_power(creature), Some(2));
}

#[test]
fn issue_155_merchant_returns_its_exact_graveyard_source_to_hand() {
    let mut engine = anthem_engine(155_002, "swamp");
    let merchant = inject_graveyard_card(&mut engine, 0, "merchant_of_many_hats");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let generation = engine
        .state
        .zone_change_generation
        .get(&merchant)
        .copied()
        .unwrap_or(0);
    let command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: merchant,
            source_zone: AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: generation,
            ability_index: 0,
            ..Default::default()
        })),
    };

    engine
        .apply_command(0, &command)
        .expect("activate Merchant from the graveyard");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&merchant].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&merchant));
    engine
        .apply_command(0, &command)
        .expect_err("the stale graveyard generation cannot be replayed");
}

#[test]
fn issue_155_path_uses_pre_sacrifice_attachment_and_the_creatures_owner() {
    let decks = Some(vec![
        deck_with("plains", &["path_to_redemption"]),
        deck_with("forest", &[]),
    ]);
    let mut engine =
        GameEngine::new(155_003, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "path_to_redemption",
        creature,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Path");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    engine.apply_command(0, &pass()).expect("controller pass");
    let resolution = engine.apply_command(1, &pass()).expect("opponent pass");

    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
    assert!(engine.state.players[1].exile.contains(&creature));
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .any(|oid| { engine.state.objects[oid].card_id == "ally_w_1_1" }));
    assert!(permanents_moved_in(&resolution).iter().any(|moved| {
        moved.object_id == creature
            && moved.destination == permanent_moved::Destination::Exile as i32
            && moved.owner_player_id == 1
    }));
}

#[test]
fn issue_155_watery_grasp_uses_the_current_attachment_at_resolution() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine =
        GameEngine::new(155_004, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "watery_grasp",
        first,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("Aura")
        .attached_to = Some(AttachmentRecipient::Object(second));
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second].zone, Zone::Library);
    assert!(engine.state.players[1].library.contains(&second));
}

#[test]
fn issue_155_live_detachment_makes_attached_object_resolution_fail_closed() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine =
        GameEngine::new(155_005, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "watery_grasp",
        creature,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("Aura")
        .attached_to = None;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
}

#[test]
fn issue_155_library_move_does_not_leave_a_token_in_a_hidden_zone() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine =
        GameEngine::new(155_006, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let token = inject_creature_on_battlefield(&mut engine, 0, "ally_w_1_1");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "watery_grasp",
        token,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
    pass_both_players(&mut engine);

    assert!(!engine.state.objects.contains_key(&token));
    assert!(!engine.state.players[0].library.contains(&token));
}

#[test]
fn issue_155_attached_library_shuffle_replays_identically() {
    fn run() -> Vec<String> {
        let decks = Some(vec![
            deck_with("island", &["watery_grasp", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine =
            GameEngine::new(155_007, &[0, 1], 20, decks, true).expect("issue #155 cards validate");
        advance_to_main1_from_game_start(&mut engine);
        let creature = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let aura = cast_and_resolve_aura(
            &mut engine,
            "watery_grasp",
            creature,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 5,
                ..Default::default()
            },
        );
        apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
        pass_both_players(&mut engine);
        engine.state.players[0]
            .library
            .iter()
            .map(|oid| engine.state.objects[oid].card_id.clone())
            .collect()
    }

    assert_eq!(run(), run());
}
