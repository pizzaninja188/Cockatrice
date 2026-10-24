use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command::Dev, permanent_moved, ruled_command::Cmd, AbilitySourceZone, ActivateAbility,
    ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone, RuledCommand,
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

fn dev_move(player_id: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player_id,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.to_string(),
                zone: zone as i32,
                ready: false,
            })),
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
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast issue #155 Aura");
    resolve_entire_stack_two_player(engine);
    battlefield_object_for_card(engine, 0, card_id)
}

fn three_player_watery_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp", "totally_lost"]),
        deck_with("forest", &[]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("three-player Watery Grasp fixture starts");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_priority_round_with_batch(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    answer_simultaneous_entry_order_in_engine_order(engine);
    answer_trigger_order_in_engine_order(engine);
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent priority pass count");
    let mut batch = None;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("active player passes"),
        );
    }
    answer_simultaneous_entry_order_in_engine_order(engine);
    batch.expect("at least one active player passes")
}

fn cast_and_resolve_watery_in_three_player_engine(engine: &mut GameEngine, target: u32) -> u32 {
    ensure_card_in_hand(engine, 0, "watery_grasp");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "watery_grasp");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Watery Grasp at the three-player target");
    resolve_priority_round_with_batch(engine);
    battlefield_object_for_card(engine, 0, "watery_grasp")
}

fn departed_attachment_owner(engine: &GameEngine, source: u32, generation: u64) -> u32 {
    let diagnostic = engine.diagnostic_snapshot().unwrap();
    let source_key = serde_json::json!([source, generation.to_string()]);
    diagnostic["state"]["last_known_attached_object_by_generation"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == source_key)
        .expect("the departed Aura generation recorded its attached object")["value"]["owner"]
        .as_u64()
        .expect("captured attachment owner is a player id") as u32
}

#[test]
fn issue_155_light_jammer_etb_attaches_and_grants_hexproof() {
    let decks = Some(vec![
        deck_with("island", &["illvoi_light_jammer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #155 cards validate");
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Illvoi Light Jammer");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(creature))
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
fn issue_274_coral_sword_etb_revalidates_shared_target_and_expires_first_strike() {
    fn setup(seed: u64) -> (GameEngine, u32) {
        let decks = Some(vec![
            deck_with("mountain", &["coral_sword", "coral_sword"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            seed,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("Coral Sword validates");
        advance_to_main1_from_game_start(&mut engine);
        let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        ensure_in_hand(&mut engine, 0, "coral_sword");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "coral_sword");
        engine
            .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
            .expect("cast Coral Sword");
        pass_both_players(&mut engine);
        engine
            .apply_scenario_command(0, &choose_trigger_target(creature))
            .expect("choose Coral Sword target");
        (engine, creature)
    }

    let (mut stale, stale_target) = setup(274_001);
    *stale
        .state
        .zone_change_generation
        .entry(stale_target)
        .or_default() += 1;
    pass_both_players(&mut stale);
    let stale_equipment = battlefield_object_for_card(&stale, 0, "coral_sword");
    assert_eq!(stale.state.objects[&stale_equipment].attached_to, None);
    assert!(!stale.effective_has_keyword(stale_target, tricerules_cards::Keyword::FirstStrike));

    let (mut source_left, source_target) = setup(274_002);
    let source_equipment = battlefield_object_for_card(&source_left, 0, "coral_sword");
    source_left.state.players[0]
        .battlefield
        .retain(|oid| *oid != source_equipment);
    source_left.state.players[0]
        .graveyard
        .push(source_equipment);
    source_left
        .state
        .objects
        .get_mut(&source_equipment)
        .unwrap()
        .zone = Zone::Graveyard;
    *source_left
        .state
        .zone_change_generation
        .entry(source_equipment)
        .or_default() += 1;
    pass_both_players(&mut source_left);
    assert_eq!(
        source_left.state.objects[&source_equipment].attached_to,
        None
    );
    assert!(
        source_left.effective_has_keyword(source_target, tricerules_cards::Keyword::FirstStrike)
    );
    end_active_turn(&mut source_left, 0);
    assert!(
        !source_left.effective_has_keyword(source_target, tricerules_cards::Keyword::FirstStrike)
    );
}

#[test]
fn issue_155_new_equipment_generation_does_not_receive_the_old_attach_effect() {
    let decks = Some(vec![
        deck_with("island", &["illvoi_light_jammer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_008,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #155 cards validate");
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Illvoi Light Jammer");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(creature))
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
fn watery_grasp_uses_departed_attachment_owner_when_target_is_stale() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_051,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Watery Grasp validates");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "watery_grasp",
        target,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    engine.state.players[0].mana_pool.colorless = 5;
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Watery Grasp");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(0, &dev_move(0, DevZone::Graveyard, "Watery Grasp"))
        .expect("move the source after its ability is on the stack");
    engine
        .apply_scenario_command(0, &dev_move(1, DevZone::Graveyard, "Grizzly Bears"))
        .expect("make the captured attachment stale before resolution");

    let resolution = pass_both_players_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(!engine.state.players[1].library.contains(&target));
    assert_eq!(
        resolution
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P1 shuffles their library."
            ))
            .count(),
        1,
        "the captured attachment's owner still shuffles once after the target leaves"
    );
}

#[test]
fn watery_grasp_does_not_shuffle_a_departed_target_owners_library() {
    let mut engine = three_player_watery_engine(155_053);
    let target = inject_creature_under_foreign_control(&mut engine, 2, 1, "grizzly_bears");
    let aura = cast_and_resolve_watery_in_three_player_engine(&mut engine, target);
    let source_generation = engine.state.zone_change_generation[&aura];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(0, &dev_move(0, DevZone::Graveyard, "Watery Grasp"))
        .expect("the activated ability's Aura source leaves with the attachment recorded");
    let diagnostic = engine.diagnostic_snapshot().unwrap();
    let source_key = serde_json::json!([aura, source_generation.to_string()]);
    let attachment = diagnostic["state"]["last_known_attached_object_by_generation"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == source_key)
        .expect("the departed Aura generation recorded its attached object");
    assert_eq!(attachment["value"]["owner"], serde_json::json!(2));
    engine
        .apply_scenario_command(2, &concede())
        .expect("the attached permanent's owner leaves before the ability resolves");
    assert!(engine.state.players[2].has_lost);
    assert!(!engine.state.objects.contains_key(&target));
    let departed_owner_library = engine.state.players[2].library.clone();

    let resolution = resolve_priority_round_with_batch(&mut engine);
    assert_eq!(engine.state.players[2].library, departed_owner_library);
    assert!(!engine.state.players[2].library.contains(&target));
    assert!(!engine.state.objects.contains_key(&target));
    assert!(!resolution.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text == "P2 shuffles their library."
    )));
}

#[test]
fn watery_grasp_captures_owner_when_source_and_attachment_leave_together() {
    let mut engine = three_player_watery_engine(155_054);
    let target = inject_creature_under_foreign_control(&mut engine, 2, 1, "grizzly_bears");
    let aura = cast_and_resolve_watery_in_three_player_engine(&mut engine, target);
    let source_generation = engine.state.zone_change_generation[&aura];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");

    let disk = inject_permanent_on_battlefield(&mut engine, 0, "nevinyrrals_disk");
    engine.state.objects.get_mut(&disk).unwrap().tapped = false;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, disk, 0, Vec::new()).expect("activate Nevinyrral's Disk");

    let destruction = resolve_priority_round_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(permanents_moved_in(&destruction).iter().any(|movement| {
        movement.object_id == aura
            && movement.destination == permanent_moved::Destination::Graveyard as i32
    }));
    assert!(permanents_moved_in(&destruction).iter().any(|movement| {
        movement.object_id == target
            && movement.destination == permanent_moved::Destination::Graveyard as i32
    }));
    assert_eq!(
        departed_attachment_owner(&engine, aura, source_generation),
        2
    );

    let resolution = resolve_priority_round_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(!engine.state.players[2].library.contains(&target));
    assert_eq!(
        resolution
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P2 shuffles their library."
            ))
            .count(),
        1,
        "the source-departure record still identifies the live attachment owner's library"
    );
}

#[test]
fn watery_grasp_retains_owner_lki_after_attached_token_ceases() {
    let mut engine = three_player_watery_engine(155_055);
    let target = inject_creature_under_foreign_control(&mut engine, 2, 1, "ally_w_1_1");
    let aura = cast_and_resolve_watery_in_three_player_engine(&mut engine, target);
    let source_generation = engine.state.zone_change_generation[&aura];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(0, &dev_move(0, DevZone::Graveyard, "Watery Grasp"))
        .expect("the activated Aura source leaves with its attachment recorded");
    assert_eq!(
        departed_attachment_owner(&engine, aura, source_generation),
        2
    );

    ensure_card_in_hand(&mut engine, 0, "totally_lost");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
    );
    let totally_lost = hand_index_for_card(&engine, 0, "totally_lost");
    engine
        .apply_scenario_command(0, &cast_spell(totally_lost, target_object(target)))
        .expect("Totally Lost legally targets the still-live attached token");
    resolve_priority_round_with_batch(&mut engine);
    assert!(!engine.state.objects.contains_key(&target));
    assert!(!engine.state.players[2].library.contains(&target));

    let resolution = resolve_priority_round_with_batch(&mut engine);
    assert!(!engine.state.objects.contains_key(&target));
    assert!(!engine.state.players[2].library.contains(&target));
    assert_eq!(
        resolution
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P2 shuffles their library."
            ))
            .count(),
        1,
        "the source's LKI retains the live token owner's library after the token ceases"
    );
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
        .apply_scenario_command(0, &command)
        .expect("activate Merchant from the graveyard");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&merchant].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&merchant));
    engine
        .apply_scenario_command(0, &command)
        .expect_err("the stale graveyard generation cannot be replayed");
}

#[test]
fn issue_155_path_uses_pre_sacrifice_attachment_and_the_creatures_owner() {
    let decks = Some(vec![
        deck_with("plains", &["path_to_redemption"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_003,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #155 cards validate");
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
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller pass");
    let resolution = engine
        .apply_scenario_command(1, &pass())
        .expect("opponent pass");

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
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_004,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #155 cards validate");
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
fn issue_155_watery_grasp_old_source_generation_keeps_departure_attachment() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_052,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Watery Grasp validates");
    advance_to_main1_from_game_start(&mut engine);
    let departed_target = inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    let returned_source_target =
        inject_creature_under_foreign_control(&mut engine, 0, 1, "grizzly_bears");
    let aura = cast_and_resolve_aura(
        &mut engine,
        "watery_grasp",
        departed_target,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let old_source_generation = engine.state.zone_change_generation[&aura];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, aura, 0, Vec::new()).expect("activate Waterbend");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(0, &dev_move(0, DevZone::Graveyard, "Watery Grasp"))
        .expect("old Aura generation leaves after activation");
    // Model the same physical card returning as a new object, then attaching before state-based
    // actions. The dev placement verb has no target-selection step for an Aura and would correctly
    // put an unattached Aura straight back in the graveyard.
    engine.state.players[0]
        .graveyard
        .retain(|&object_id| object_id != aura);
    engine.state.players[0].battlefield.push(aura);
    engine.state.objects.get_mut(&aura).unwrap().zone = Zone::Battlefield;
    *engine.state.zone_change_generation.entry(aura).or_default() += 1;
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_ne!(
        engine.state.zone_change_generation[&aura],
        old_source_generation
    );
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(returned_source_target));

    let resolution = pass_both_players_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&departed_target].zone, Zone::Library);
    assert_eq!(
        engine.state.objects[&returned_source_target].zone,
        Zone::Battlefield
    );
    assert!(engine.state.players[1].library.contains(&departed_target));
    assert_eq!(
        resolution
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text == "P1 shuffles their library."
            ))
            .count(),
        1
    );
}

#[test]
fn issue_155_target_departure_before_source_has_no_attachment_lki_or_shuffle() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_005,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #155 cards validate");
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
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(0, &dev_move(0, DevZone::Graveyard, "Grizzly Bears"))
        .expect("the target leaves before the Aura");
    assert_eq!(engine.state.objects[&aura].attached_to, None);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    // The now-unattached Aura was put in its owner's graveyard by state-based actions before a
    // separate dev move could occur, so this is the target-first detachment/source-departure case.
    let resolution = pass_both_players_with_batch(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert!(!resolution.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text == "P0 shuffles their library."
    )));
}

#[test]
fn issue_155_library_move_does_not_leave_a_token_in_a_hidden_zone() {
    let decks = Some(vec![
        deck_with("island", &["watery_grasp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        155_006,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #155 cards validate");
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            155_007,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("issue #155 cards validate");
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
