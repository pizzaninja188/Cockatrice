//! Issue #429 — command-boundary scenarios for the four newly eligible Aura identities.
//!
//! Every expectation is the reviewed printed Oracle behavior. Exact Scryfall records and
//! `rulings_uri` responses were fetched 2026-09-20 against pinned snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e`. Exercised rulings: New Horizons (a cast with no
//! controlled creature is legal and its entry trigger simply has no legal target; a fizzled Aura
//! never triggers), Friendly Neighborhood (the creature count is determined as the granted
//! ability resolves), and Stop Cold (an ability granted after attachment is kept). Governing CR
//! concepts: 608.2b (fizzled Aura), 603.6a (entry triggers), 701.26 (tap), 122.1 (counters),
//! 111.1 (tokens), 605.1a (granted mana ability), 613.1f/613.7 (layer-6 removal and timestamps),
//! and 502.3 (untap-step restriction).

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

fn advance_to_active_player_upkeep(engine: &mut GameEngine, player: i32) {
    for _ in 0..50 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass through turn");
        if engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn activate_mana_option(
    engine: &GameEngine,
    permanent_id: u32,
    ability_index: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, permanent_id, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for returns an activation")
    };
    ability.mana_option_index = option;
    command
}

#[test]
fn stuck_in_summoners_sanctum_taps_locks_activation_and_untap_until_detached() {
    let decks = Some(vec![
        deck_with("island", &["stuck_in_summoners_sanctum"]),
        forest_only_deck(),
    ]);
    let mut engine = GameEngine::new(429_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let skeletons = inject_creature_on_battlefield(&mut engine, 1, "drudge_skeletons");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 1, skeletons), [true]);

    ensure_card_in_hand(&mut engine, 0, "stuck_in_summoners_sanctum");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "stuck_in_summoners_sanctum");
    engine
        .apply_command(0, &cast_spell(slot, target_object(skeletons)))
        .expect("cast Stuck in Summoner's Sanctum on a creature");
    resolve_entire_stack_two_player(&mut engine);

    let aura = battlefield_object_for_card(&engine, 0, "stuck_in_summoners_sanctum");
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(skeletons))
    );
    assert!(
        engine.state.objects[&skeletons].tapped,
        "its entry trigger taps the enchanted creature"
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 1, skeletons), [false]);
    let labels = zone_view_rules_annotation_labels(&mut engine, 1, skeletons);
    assert!(labels
        .iter()
        .any(|label| label == "Doesn't untap during its controller's untap step"));
    assert!(labels
        .iter()
        .any(|label| label == "Activated abilities can't be activated"));
    assert!(matches!(
        engine.apply_command(1, &activate_ability_for(&engine, skeletons, 0, vec![])),
        Err(tricerules_core::EngineError::Illegal(_))
    ));
    assert_eq!(
        engine.state.players[1].mana_pool.black, 1,
        "a rejected activation pays no cost"
    );

    end_active_turn(&mut engine, 0);
    advance_to_active_player_upkeep(&mut engine, 1);
    assert!(
        engine.state.objects[&skeletons].tapped,
        "the restriction applies at its controller's untap step"
    );

    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("Stuck")
        .attached_to = None;
    assert_eq!(zone_view_ability_flags(&mut engine, 1, skeletons), [true]);
    assert!(
        !zone_view_rules_annotation_labels(&mut engine, 1, skeletons)
            .iter()
            .any(|label| label == "Activated abilities can't be activated")
    );
    let priority = engine.state.priority_player_id();
    if priority != 1 {
        engine
            .apply_command(priority, &pass())
            .expect("pass priority to the enchanted creature's controller");
    }
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(1, &activate_ability_for(&engine, skeletons, 0, vec![]))
        .expect("the ability can be activated after its Aura no longer applies");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the ability is activated and waits to resolve"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&skeletons].regeneration_shields, 1);
}

#[test]
fn petrify_prohibits_activated_abilities_and_attacks() {
    let decks = Some(vec![deck_with("plains", &["petrify"]), forest_only_deck()]);
    let mut engine = GameEngine::new(429_008, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let petrified = inject_creature_on_battlefield(&mut engine, 0, "drudge_skeletons");
    let legal_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    assert_eq!(zone_view_ability_flags(&mut engine, 0, petrified), [true]);

    ensure_card_in_hand(&mut engine, 0, "petrify");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "petrify");
    engine
        .apply_command(0, &cast_spell(slot, target_object(petrified)))
        .expect("cast Petrify on a creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(zone_view_ability_flags(&mut engine, 0, petrified), [false]);
    assert!(matches!(
        engine.apply_command(0, &activate_ability_for(&engine, petrified, 0, vec![])),
        Err(tricerules_core::EngineError::Illegal(_))
    ));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, petrified),
        [
            "Activated abilities can't be activated",
            "Can't attack",
            "Can't block",
        ]
    );

    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes");
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    assert!(engine
        .apply_command(0, &declare_attackers(vec![petrified]))
        .is_err());
    engine
        .apply_command(0, &declare_attackers(vec![legal_attacker]))
        .expect("an unenchanted creature remains able to attack");
}

#[test]
fn petrify_prohibits_blocking_and_accepts_artifact_targets() {
    let decks = Some(vec![
        deck_with("plains", &["petrify", "petrify"]),
        forest_only_deck(),
    ]);
    let mut engine = GameEngine::new(429_009, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let petrified_blocker = inject_creature_on_battlefield(&mut engine, 1, "drudge_skeletons");
    let legal_blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let treasure = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );

    ensure_card_in_hand(&mut engine, 0, "petrify");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    let first_slot = hand_index_for_card(&engine, 0, "petrify");
    engine
        .apply_command(0, &cast_spell(first_slot, target_object(petrified_blocker)))
        .expect("cast Petrify on the creature blocker");
    resolve_entire_stack_two_player(&mut engine);
    let creature_aura = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| {
            let object = &engine.state.objects[id];
            object.card_id == "petrify"
                && object.attached_to == Some(AttachmentRecipient::Object(petrified_blocker))
        })
        .expect("the actual Petrify object is attached to the creature blocker");
    assert_eq!(
        engine.state.objects[&creature_aura].attached_to,
        Some(AttachmentRecipient::Object(petrified_blocker))
    );
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, petrified_blocker),
        [false]
    );
    assert!(engine
        .apply_command(
            1,
            &activate_ability_for(&engine, petrified_blocker, 0, vec![])
        )
        .is_err());

    ensure_card_in_hand(&mut engine, 0, "petrify");
    let second_slot = hand_index_for_card(&engine, 0, "petrify");
    engine
        .apply_command(0, &cast_spell(second_slot, target_object(treasure)))
        .expect("Petrify can enchant an artifact token");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[0].battlefield.iter().any(|id| {
        let object = &engine.state.objects[id];
        object.card_id == "petrify"
            && object.attached_to == Some(AttachmentRecipient::Object(treasure))
    }));
    assert_eq!(zone_view_ability_flags(&mut engine, 1, treasure), [false]);
    assert!(engine.state.objects[&treasure].token_origin.is_some());
    assert!(engine
        .apply_command(1, &activate_ability_for(&engine, treasure, 0, vec![]))
        .is_err());

    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare an unenchanted attacker");
    engine
        .apply_command(0, &pass())
        .expect("attacking player passes");
    let blockers = engine
        .apply_command(1, &pass())
        .expect("defending player receives blockers");
    let legal = blockers
        .legal_by_player
        .get(&1)
        .expect("defending-player legal actions");
    assert_eq!(
        legal
            .legal_block_pairs
            .iter()
            .map(|pair| pair.blocker_id)
            .collect::<Vec<_>>(),
        [legal_blocker],
        "Petrify removes only its attached creature from legal blockers"
    );
    assert!(engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: petrified_blocker,
            }])
        )
        .is_err());
}

#[test]
fn stop_cold_taps_removes_abilities_and_keeps_a_later_grant() {
    let decks = Some(vec![
        deck_with("island", &["stop_cold", "flight"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let farrier = inject_creature_on_battlefield(&mut engine, 1, "surly_farrier");
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, farrier).len(),
        1,
        "Surly Farrier publishes its printed activated ability before the Aura attaches"
    );

    ensure_card_in_hand(&mut engine, 0, "stop_cold");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "stop_cold");
    engine
        .apply_command(0, &cast_spell(slot, target_object(farrier)))
        .expect("cast Stop Cold on a creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.objects[&farrier].tapped,
        "the entry trigger taps the enchanted creature"
    );
    assert!(
        zone_view_ability_flags(&mut engine, 1, farrier).is_empty(),
        "the enchanted permanent loses every activated ability"
    );

    // Stop Cold's 2024-04-12 ruling: an ability gained after attachment is kept. Flight is
    // attached later, so its layer-6 grant has a later timestamp than the removal effect.
    ensure_card_in_hand(&mut engine, 0, "flight");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "flight");
    engine
        .apply_command(0, &cast_spell(slot, target_object(farrier)))
        .expect("cast Flight on the same creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.effective_has_keyword(farrier, Keyword::Flying),
        "a later ability grant is kept"
    );

    end_active_turn(&mut engine, 0);
    assert!(
        engine.state.objects[&farrier].tapped,
        "the enchanted creature does not untap during its controller's untap step"
    );
}

#[test]
fn flood_the_engine_requires_a_creature_or_vehicle_and_taps_it() {
    let decks = Some(vec![
        deck_with("island", &["flood_the_engine"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "flood_the_engine");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "flood_the_engine");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "a land is not a legal Enchant creature or Vehicle target"
    );
    let slot = hand_index_for_card(&engine, 0, "flood_the_engine");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Flood the Engine on a creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.objects[&bear].tapped,
        "the entry trigger taps the enchanted permanent"
    );
}

#[test]
fn new_horizons_counters_and_grants_two_mana_of_one_color() {
    let decks = Some(vec![
        deck_with("forest", &["new_horizons"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "new_horizons");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "new_horizons");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast New Horizons on a land");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("New Horizons resolves");
    assert!(
        engine
            .apply_command(0, &choose_trigger_target(opposing))
            .is_err(),
        "the counter trigger targets only a creature you control"
    );
    engine
        .apply_command(0, &choose_trigger_target(bear))
        .expect("choose the controlled creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(3));
    assert_eq!(engine.effective_toughness(bear), Some(3));

    // The printed Forest mana ability is index 0; the granted ability is index 1.
    let command = activate_mana_option(&engine, land, 1, 0);
    engine
        .apply_command(0, &command)
        .expect("activate the granted two-mana ability");
    assert_eq!(engine.state.players[0].mana_pool.white, 2);
    assert!(engine.state.objects[&land].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "a mana ability does not stack"
    );
}

#[test]
fn new_horizons_cast_with_no_creature_is_legal_and_has_no_counter_target() {
    let decks = Some(vec![
        deck_with("forest", &["new_horizons"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    ensure_card_in_hand(&mut engine, 0, "new_horizons");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "new_horizons");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("New Horizons is castable with no creatures you control");
    resolve_entire_stack_two_player(&mut engine);
    let aura = battlefield_object_for_card(&engine, 0, "new_horizons");
    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(
        engine.state.pending_trigger_order.is_none()
            && engine.state.pending_triggers.is_empty()
            && engine.state.stack.is_empty(),
        "the entry trigger has no legal target and leaves no pending choice"
    );
}

#[test]
fn new_horizons_fizzles_and_never_triggers_when_its_land_leaves() {
    let decks = Some(vec![
        deck_with("forest", &["new_horizons"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    ensure_card_in_hand(&mut engine, 0, "new_horizons");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "new_horizons");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast New Horizons");
    let aura_stack_id = engine.state.stack.last().expect("Aura on the stack").id;
    {
        let object = engine.state.objects.get_mut(&land).expect("land object");
        object.zone = tricerules_core::Zone::Graveyard;
    }
    engine.state.players[0].battlefield.retain(|&id| id != land);
    engine.state.players[0].graveyard.push(land);

    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("fizzled Aura resolves");
    assert_eq!(
        engine.state.objects.get(&aura_stack_id).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "the Aura goes to the graveyard when its target left (CR 608.2b)"
    );
    assert!(
        engine
            .state
            .continuous_effects
            .iter()
            .all(|effect| effect.source_id != Some(aura_stack_id)),
        "a fizzled Aura creates no attached modifier"
    );
    assert!(
        engine.state.pending_trigger_order.is_none()
            && engine.state.pending_triggers.is_empty()
            && engine.state.stack.is_empty(),
        "the fizzled Aura never triggers its entry ability"
    );
}

#[test]
fn friendly_neighborhood_creates_three_citizens_and_pumps_per_creature() {
    let decks = Some(vec![
        deck_with("forest", &["friendly_neighborhood"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "friendly_neighborhood");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "friendly_neighborhood");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast Friendly Neighborhood on a land");
    resolve_entire_stack_two_player(&mut engine);
    let tokens = battlefield_token_oids(&engine, 0, "human_citizen_gw_1_1");
    assert_eq!(tokens.len(), 3, "the entry trigger creates three tokens");
    for token in &tokens {
        assert_eq!(engine.effective_power(*token), Some(1));
        assert_eq!(engine.effective_toughness(*token), Some(1));
    }

    // The printed Forest mana ability is index 0; the granted pump is index 1. The count is
    // determined as the ability resolves (2025-09-19 ruling): at activation the Bear plus three
    // Citizens is four, and a second Bear joins before the ability resolves so the bonus is +5/+5.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, land, 1, target_object(bear))
        .expect("activate the granted pump ability");
    let reinforcement = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(7));
    assert_eq!(engine.effective_toughness(bear), Some(7));
    assert_eq!(engine.effective_power(reinforcement), Some(2));
    assert!(engine.state.objects[&land].tapped);
}
