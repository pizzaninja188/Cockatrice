use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::CounterKind;
use tricerules_core::{EngineDeck, Zone};

fn published_ability_mana_cost(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
    ability_index: usize,
) -> String {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.get(player))
        .into_iter()
        .flat_map(|view| view.battlefield_objects.iter())
        .find(|object| object.object_id == object_id)
        .and_then(|object| object.activated_abilities.get(ability_index))
        .map(|ability| ability.mana_cost.clone())
        .unwrap_or_else(|| panic!("missing published ability {ability_index} for {object_id}"))
}

fn flaring_cinder_etb_fixture(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["flaring_cinder", "shock", "opt"]),
        deck_with("island", &["island"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Flaring Cinder ETB fixture");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "flaring_cinder");
    ensure_in_hand(&mut engine, 0, "shock");
    ensure_in_hand(&mut engine, 1, "island");
    engine
}

fn select_flaring_cinder_discard_branch_if_legacy(engine: &mut GameEngine) {
    let Some(pending) = engine.state.pending_resolution.as_ref() else {
        panic!("Flaring Cinder's optional payment is offered");
    };
    if pending.presentation.choice_kind == ChoiceKind::ResolutionBranch {
        let chooser = pending.deciding_player;
        engine
            .apply_scenario_command(
                chooser,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .expect("select the legacy discard branch before its payment");
    }
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Flaring Cinder discard prompt")
            .presentation
            .choice_kind,
        ChoiceKind::HandCards
    );
}

fn decline_flaring_cinder_optional_payment(engine: &mut GameEngine) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Flaring Cinder's optional payment prompt");
    let chooser = pending.deciding_player;
    let command = match pending.presentation.choice_kind {
        ChoiceKind::ResolutionBranch => {
            submit_resolution_decision(ResolutionChoiceDecision::Decline)
        }
        ChoiceKind::HandCards => submit_resolution_choice(Vec::new()),
        other => panic!("unexpected Flaring Cinder choice {other:?}"),
    };
    engine
        .apply_scenario_command(chooser, &command)
        .expect("decline Flaring Cinder's optional discard");
}

fn assert_rejected_flaring_cinder_answer_is_atomic(
    engine: &mut GameEngine,
    actor: i32,
    command: RuledCommand,
) {
    let before = format!("{:?}", engine.state);
    let command_index = engine.state.command_index;
    assert!(engine.apply_command(actor, &command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert_eq!(engine.state.command_index, command_index);
}

fn empty_flaring_cinder_controller_hand(engine: &mut GameEngine) {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    engine.enable_dev_commands();
    while let Some(object_id) = engine.state.players[0].hand.first().copied() {
        let card_id = engine.state.objects[&object_id].card_id.clone();
        let card_name = tricerules_cards::registry::global()
            .get(&card_id)
            .expect("hand card definition")
            .name
            .clone();
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name,
                            zone: DevZone::Library as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("move the fixture card out of the controller's hand");
    }
    assert!(engine.state.players[0].hand.is_empty());
}

#[test]
fn starport_security_reduces_only_for_a_plus_one_plus_one_counter() {
    let decks = Some(vec![
        deck_with("plains", &["starport_security", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let security = relocate_to_battlefield(&mut engine, 0, "starport_security", false);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    engine
        .state
        .objects
        .get_mut(&security)
        .expect("Starport Security")
        .add_counters(CounterKind::Stun, 1, 1);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, security, 0),
        "{3}{W}",
        "an unrelated counter must not satisfy the Oracle condition"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let mana_before = (
        engine.state.players[0].mana_pool.white,
        engine.state.players[0].mana_pool.colorless,
    );
    let command_before = engine.state.command_index;
    assert!(apply_ability(&mut engine, 0, security, 0, target_object(target)).is_err());
    assert_eq!(
        (
            engine.state.players[0].mana_pool.white,
            engine.state.players[0].mana_pool.colorless,
        ),
        mana_before
    );
    assert_eq!(engine.state.command_index, command_before);
    assert!(!engine.state.objects[&security].tapped);

    engine
        .state
        .objects
        .get_mut(&security)
        .expect("Starport Security")
        .add_counters(CounterKind::PlusOnePlusOne, 1, 2);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, security, 0),
        "{1}{W}"
    );
    apply_ability(&mut engine, 0, security, 0, target_object(target))
        .expect("activate reduced Starport Security ability");
    assert!(engine.state.objects[&security].tapped);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&target].tapped);
}

#[test]
fn boneclub_berserker_counts_other_controlled_goblins_in_layer_seven() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["boneclub_berserker", "raging_goblin", "goblin_arsonist"],
        ),
        deck_with("mountain", &["crazed_goblin"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_002,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "boneclub_berserker");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "boneclub_berserker");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Boneclub Berserker");
    resolve_entire_stack_two_player(&mut engine);
    let berserker = battlefield_object_for_card(&engine, 0, "boneclub_berserker");
    assert_eq!(engine.characteristics(berserker).unwrap().power, Some(2));

    relocate_to_battlefield(&mut engine, 0, "raging_goblin", false);
    relocate_to_battlefield(&mut engine, 1, "crazed_goblin", false);
    assert_eq!(
        engine.characteristics(berserker).unwrap().power,
        Some(4),
        "the source and an opponent's Goblin are excluded"
    );

    relocate_to_battlefield(&mut engine, 0, "goblin_arsonist", false);
    assert_eq!(engine.characteristics(berserker).unwrap().power, Some(6));
}

#[test]
fn sold_out_uses_the_chosen_targets_pre_exile_damage_identity() {
    let decks = Some(vec![
        deck_with("swamp", &["sold_out", "shock"]),
        deck_with("forest", &["giant_spider"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_003,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    ensure_in_hand(&mut engine, 0, "shock");
    ensure_in_hand(&mut engine, 0, "sold_out");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let shock = hand_index_for_card(&engine, 0, "shock");
    engine
        .apply_scenario_command(0, &cast_spell(shock, target_object(target)))
        .expect("cast Shock");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 2);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    engine
        .apply_scenario_command(0, &cast_spell(sold_out, target_object(target)))
        .expect("cast Sold Out");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "clue").len(), 1);
}

#[test]
fn sold_out_exiles_an_undamaged_target_without_creating_a_clue() {
    let decks = Some(vec![
        deck_with("swamp", &["sold_out"]),
        deck_with("forest", &["giant_spider"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_005,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    ensure_in_hand(&mut engine, 0, "sold_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    engine
        .apply_scenario_command(0, &cast_spell(sold_out, target_object(target)))
        .expect("cast Sold Out at an undamaged creature");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(battlefield_token_oids(&engine, 0, "clue").is_empty());
}

#[test]
fn sold_out_does_not_use_damage_from_a_prior_incarnation_of_the_same_object_id() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    let decks = Some(vec![
        deck_with("swamp", &["sold_out", "shock"]),
        deck_with("forest", &["giant_spider"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_006,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    ensure_in_hand(&mut engine, 0, "shock");
    ensure_in_hand(&mut engine, 0, "sold_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let shock = hand_index_for_card(&engine, 0, "shock");
    engine
        .apply_scenario_command(0, &cast_spell(shock, target_object(target)))
        .expect("damage the first incarnation");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 2);
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);

    engine.enable_dev_commands();
    for zone in [DevZone::Graveyard, DevZone::Battlefield] {
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 1,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Giant Spider".into(),
                            zone: zone as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("move the same physical card through a zone change");
    }
    assert_eq!(
        battlefield_object_for_card(&engine, 1, "giant_spider"),
        target
    );
    assert!(
        engine
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0)
            > old_generation
    );
    assert_eq!(engine.state.objects[&target].damage, 0);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    engine
        .apply_scenario_command(0, &cast_spell(sold_out, target_object(target)))
        .expect("cast Sold Out at the returned incarnation");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(battlefield_token_oids(&engine, 0, "clue").is_empty());
}

#[test]
fn sold_out_fizzles_without_a_clue_when_its_only_target_becomes_illegal() {
    let decks = Some(vec![
        deck_with("swamp", &["sold_out", "shock"]),
        deck_with("swamp", &["murder", "giant_spider"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_007,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    ensure_in_hand(&mut engine, 0, "shock");
    ensure_in_hand(&mut engine, 0, "sold_out");
    ensure_in_hand(&mut engine, 1, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let shock = hand_index_for_card(&engine, 0, "shock");
    engine
        .apply_scenario_command(0, &cast_spell(shock, target_object(target)))
        .expect("damage the target first");
    resolve_entire_stack_two_player(&mut engine);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    engine
        .apply_scenario_command(0, &cast_spell(sold_out, target_object(target)))
        .expect("cast Sold Out");
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass priority so the opponent can respond");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&engine, 1, "murder");
    engine
        .apply_scenario_command(1, &cast_spell(murder, target_object(target)))
        .expect("remove the sole target in response");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(battlefield_token_oids(&engine, 0, "clue").is_empty());
}

#[test]
fn sold_out_finishes_its_clue_rider_before_the_commander_903_9a_choice() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    let decks = Some(vec![
        EngineDeck {
            mainboard: vec!["swamp".into(); 12]
                .into_iter()
                .chain(["shock".into(), "sold_out".into()])
                .collect(),
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 12],
            commanders: vec!["kami_of_the_crescent_moon".into()],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        158_009,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("commander engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = engine.state.players[1].command_zone[0];
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 1,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Kami of the Crescent Moon".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the declared commander onto the battlefield");
    ensure_in_hand(&mut engine, 0, "shock");
    ensure_in_hand(&mut engine, 0, "sold_out");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let shock = hand_index_for_card(&engine, 0, "shock");
    engine
        .apply_scenario_command(0, &cast_spell(shock, target_object(target)))
        .expect("damage the commander before Sold Out");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 2);

    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    engine
        .apply_scenario_command(0, &cast_spell(sold_out, target_object(target)))
        .expect("cast Sold Out at the damaged commander");
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass priority after casting Sold Out");
    let resolved = engine
        .apply_scenario_command(1, &pass())
        .expect("resolve the whole spell before checking state-based actions");

    // CR 903.9a's command-zone choice is offered after the whole spell finishes resolving.
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "clue").len(), 1);
    let commander_choice = find_resolution_choice(&resolved)
        .expect("the commander owner receives the post-resolution choice");
    assert_eq!(commander_choice.deciding_player_id, 1);
    assert!(commander_choice.prompt_text.contains("command zone"));
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn sold_out_serialized_cast_and_priority_commands_replay_the_same_batches_and_state() {
    use prost::Message;

    fn fresh() -> (GameEngine, u32) {
        let decks = Some(vec![
            deck_with("swamp", &["sold_out", "shock"]),
            deck_with("forest", &["giant_spider"]),
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            158_008,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let target = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
        ensure_in_hand(&mut engine, 0, "shock");
        ensure_in_hand(&mut engine, 0, "sold_out");
        // Fixture setup is applied identically to the recording and replay
        // engines; the log below contains only accepted player commands.
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                b: 1,
                c: 3,
                ..Default::default()
            },
        );
        (engine, target)
    }
    fn send(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, Vec<u8>, Vec<u8>)>,
        player: i32,
        command: RuledCommand,
    ) {
        let command_bytes = command.encode_to_vec();
        let batch_bytes = engine
            .apply_scenario_command(player, &command)
            .expect("accepted Sold Out cast or priority command")
            .encode_to_vec();
        log.push((player, command_bytes, batch_bytes));
    }
    fn resolve_stack(engine: &mut GameEngine, log: &mut Vec<(i32, Vec<u8>, Vec<u8>)>) {
        while !engine.state.stack.is_empty() {
            let actor = engine.state.priority_player_id();
            send(engine, log, actor, pass());
        }
    }

    let (mut engine, target) = fresh();
    let mut log = Vec::new();
    let shock = hand_index_for_card(&engine, 0, "shock");
    send(
        &mut engine,
        &mut log,
        0,
        cast_spell(shock, target_object(target)),
    );
    resolve_stack(&mut engine, &mut log);
    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    send(
        &mut engine,
        &mut log,
        0,
        cast_spell(sold_out, target_object(target)),
    );
    resolve_stack(&mut engine, &mut log);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "clue").len(), 1);

    let (mut replay, replay_target) = fresh();
    assert_eq!(replay_target, target);
    for (player, command_bytes, expected_batch) in &log {
        let command = RuledCommand::decode(command_bytes.as_slice())
            .expect("serialized accepted command decodes");
        let actual_batch = replay
            .apply_scenario_command(*player, &command)
            .expect("replay accepted command")
            .encode_to_vec();
        assert_eq!(actual_batch, *expected_batch);
    }
    assert_eq!(
        replay
            .diagnostic_snapshot()
            .expect("serialize replay diagnostic state"),
        engine
            .diagnostic_snapshot()
            .expect("serialize recorded diagnostic state")
    );
    assert_eq!(replay.state.command_index, engine.state.command_index);
}

#[test]
fn flaring_cinder_cast_trigger_includes_announced_x_in_mana_value() {
    let decks = Some(vec![
        deck_with("mountain", &["flaring_cinder", "blaze", "blaze", "shock"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_004,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cinder = relocate_to_battlefield(&mut engine, 0, "flaring_cinder", false);
    ensure_in_hand(&mut engine, 0, "blaze");
    ensure_in_hand(&mut engine, 0, "shock");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let first_blaze = hand_index_for_card(&engine, 0, "blaze");
    engine
        .apply_scenario_command(0, &cast_spell_x(first_blaze, target_player(1), 2))
        .expect("cast Blaze with mana value three");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "X=2 makes Blaze mana value three and does not trigger Cinder"
    );
    resolve_entire_stack_two_player(&mut engine);

    ensure_in_hand(&mut engine, 0, "blaze");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let second_blaze = hand_index_for_card(&engine, 0, "blaze");
    engine
        .apply_scenario_command(0, &cast_spell_x(second_blaze, target_player(1), 3))
        .expect("cast Blaze with mana value four");
    assert_eq!(engine.state.stack.len(), 2);
    let trigger = engine.state.stack.last().expect("Flaring Cinder trigger");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(cinder));

    let shock = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| {
            engine
                .state
                .objects
                .get(oid)
                .map(|object| object.card_id.as_str())
                == Some("shock")
        })
        .expect("the controller has a card available to pay Flaring Cinder");
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    pass_both_players(&mut engine);
    select_flaring_cinder_discard_branch_if_legacy(&mut engine);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![shock]))
        .expect("pay the actual mana-value-four Flaring Cinder trigger");
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "paying the actual qualifying cast trigger draws exactly once"
    );
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "shock"), 1);
    resolve_entire_stack_two_player(&mut engine);
}

#[test]
fn flaring_cinder_enters_and_optional_discard_draws_once_after_authorized_payment() {
    let mut engine = flaring_cinder_etb_fixture(158_005);
    let shock = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| {
            engine
                .state
                .objects
                .get(oid)
                .map(|object| object.card_id.as_str())
                == Some("shock")
        })
        .expect("the controller has a discard candidate");
    let foreign_card = *engine.state.players[1]
        .hand
        .first()
        .expect("the other player has a foreign hand object");
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    pass_both_players(&mut engine);
    select_flaring_cinder_discard_branch_if_legacy(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Flaring Cinder's ETB discard offer");
    let prompt_id = pending.resolution_prompt_id.get();
    assert_ne!(prompt_id, 0);
    assert_eq!(pending.deciding_player, 0);
    assert!(pending.presentation.candidates.contains(&shock));
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);

    let mut wrong_actor = submit_resolution_choice(vec![shock]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = wrong_actor.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    assert_rejected_flaring_cinder_answer_is_atomic(&mut engine, 1, wrong_actor);

    let mut foreign_candidate = submit_resolution_choice(vec![foreign_card]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = foreign_candidate.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    assert_rejected_flaring_cinder_answer_is_atomic(&mut engine, 0, foreign_candidate);

    let mut stale_prompt = submit_resolution_choice(vec![shock]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_prompt.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id + 1;
    assert_rejected_flaring_cinder_answer_is_atomic(&mut engine, 0, stale_prompt);

    let mut paid = submit_resolution_choice(vec![shock]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = paid.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    engine
        .apply_command(0, &paid)
        .expect("the ETB trigger's controller pays with the offered card");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "shock"), 1);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the actual ETB trigger draws once after the discard payment commits"
    );
}

#[test]
fn flaring_cinder_draws_the_same_card_after_library_of_leng_replacement() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["flaring_cinder", "shock", "opt", "library_of_leng"],
        ),
        deck_with("island", &["island"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        158_008,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Flaring Cinder Library of Leng fixture");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "flaring_cinder");
    relocate_to_battlefield(&mut engine, 0, "library_of_leng", false);
    ensure_in_hand(&mut engine, 0, "shock");
    let shock = engine.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| {
            engine
                .state
                .objects
                .get(oid)
                .map(|object| object.card_id.as_str())
                == Some("shock")
        })
        .expect("the controller has the exact card to pay the trigger");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&shock)
        .copied()
        .unwrap_or(0);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    pass_both_players(&mut engine);
    select_flaring_cinder_discard_branch_if_legacy(&mut engine);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![shock]))
        .expect("pay the actual Flaring Cinder ETB trigger");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Library of Leng offers its replacement")
            .presentation
            .choice_kind,
        ChoiceKind::PrivateReplacement
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("put the paid card on top with Library of Leng");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&shock].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&shock));
    assert_eq!(
        engine.state.zone_change_generation.get(&shock).copied(),
        Some(generation_before + 2),
        "the paid physical card moves through the library and then returns as the draw"
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the selected discard still counts as chosen to pay after its hidden replacement"
    );
}

#[test]
fn flaring_cinder_optional_discard_decline_does_not_draw() {
    let mut engine = flaring_cinder_etb_fixture(158_006);
    let hand_before = engine.state.players[0].hand.clone();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    pass_both_players(&mut engine);
    decline_flaring_cinder_optional_payment(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "declining the actual ETB trigger neither discards nor draws"
    );
}

#[test]
fn flaring_cinder_etb_with_empty_hand_has_no_free_draw_or_choice() {
    let mut engine = flaring_cinder_etb_fixture(158_007);
    empty_flaring_cinder_controller_hand(&mut engine);
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    let batch = pass_both_players_with_batch(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(find_resolution_choice(&batch).is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "an unpayable optional discard does not become a free draw"
    );
}
