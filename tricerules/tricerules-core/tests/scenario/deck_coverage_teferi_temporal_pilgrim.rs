//! Teferi, Temporal Pilgrim complete-card rules coverage.

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::primitives::{ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, Zone};

const TEFERI: &str = "teferi,_temporal_pilgrim";

#[test]
fn exact_card_identity_is_registered() {
    let card = tricerules_cards::registry::global()
        .get(TEFERI)
        .expect("Teferi, Temporal Pilgrim needs a complete definition");

    assert_eq!(card.name, "Teferi, Temporal Pilgrim");
}

fn multiplayer_engine(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[10, 20, 30, 40],
        20,
        Some(vec![deck.clone(), deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("four-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_until_permanent_choice(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    let mut choice = None;
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        let priority = engine.state.priority_player_id();
        let batch = engine
            .apply_command(priority, &pass())
            .expect("pass priority toward Teferi's ultimate");
        if engine.state.pending_resolution.is_some() {
            choice = find_resolution_choice(&batch);
            break;
        }
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Teferi's ultimate parks for the target opponent's choice");
    assert_eq!(pending.deciding_player, 20);
    choice.expect("permanent choice event")
}

fn resolve_current_stack(engine: &mut GameEngine) {
    let _ = resolve_current_stack_with_events(engine);
}

fn resolve_current_stack_with_events(
    engine: &mut GameEngine,
) -> Vec<tricerules_proto::ruled::v1::RuledEvent> {
    let mut events = Vec::new();
    for _ in 0..32 {
        for (_, _, batch) in answer_simultaneous_entry_order_in_engine_order(engine) {
            events.extend(batch.events);
        }
        answer_trigger_order_in_engine_order(engine);
        if engine.state.pending_resolution.is_some() || engine.state.stack.is_empty() {
            break;
        }
        let priority = engine.state.priority_player_id();
        let batch = engine
            .apply_command(priority, &pass())
            .expect("pass priority toward the resolving Teferi ability");
        events.extend(batch.events);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    events
}

fn targeted_player_choice_answer(
    object_ids: Vec<u32>,
) -> tricerules_proto::ruled::v1::RuledCommand {
    submit_resolution_choice(object_ids)
}

fn select_replacement_chooser(index: u32) -> tricerules_proto::ruled::v1::RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn give_control_to(engine: &mut GameEngine, object_id: u32, controller: i32) {
    let owner = engine.state.objects[&object_id].owner;
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("stolen permanent")
        .base_controller = owner;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(controller),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn teferis_ultimate_returns_the_chosen_land_and_shuffles_each_remaining_nonland_by_owner() {
    let teferi = tricerules_cards::registry::global()
        .get(TEFERI)
        .expect("Teferi, Temporal Pilgrim needs a complete definition");
    assert_eq!(teferi.primary_face().activated_abilities.len(), 3);
    let mut engine = multiplayer_engine(26_106_000);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 12);
    let returned_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let artifact_land = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_citadel");
    let target_nonland = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let foreign_nonland = inject_permanent_on_battlefield(&mut engine, 1, "bonesplitter");
    let target_token =
        inject_permanent_on_battlefield(&mut engine, 1, "spirit_u_2_2_vigilance_draw_counter");
    engine
        .state
        .objects
        .get_mut(&foreign_nonland)
        .unwrap()
        .owner = 40;

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 2, target_player(20)),
        )
        .expect("activate Teferi's ultimate targeting an opponent");
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "paying the ultimate at exactly twelve loyalty removes Teferi before the ability resolves"
    );
    assert!(
        !engine.state.stack.is_empty(),
        "the activated ability remains on the stack after its source leaves"
    );
    let choice = resolve_until_permanent_choice(&mut engine);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.max, 1);
    assert!(choice.candidate_object_ids.contains(&returned_land));
    assert!(choice.candidate_object_ids.contains(&target_nonland));
    assert!(choice.candidate_object_ids.contains(&foreign_nonland));

    let batch = engine
        .apply_command(20, &submit_resolution_choice(vec![returned_land]))
        .expect("the target opponent chooses a permanent");

    assert_eq!(engine.state.objects[&returned_land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&artifact_land].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target_nonland].zone, Zone::Library);
    assert_eq!(engine.state.objects[&foreign_nonland].zone, Zone::Library);
    assert!(!engine.state.objects.contains_key(&target_token));
    let owner_shuffle_logs = batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                if log.text.contains("shuffles their library") =>
            {
                Some(log.text.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owner_shuffle_logs,
        vec![
            "P20 shuffles their library.".to_string(),
            "P40 shuffles their library.".to_string()
        ]
    );
}

#[test]
fn ultimate_resolution_replays_identically_from_same_seed_and_accepted_commands() {
    use prost::Message;

    fn setup() -> (GameEngine, u32, u32, u32) {
        let mut engine = multiplayer_engine(26_106_008);
        let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
        engine
            .state
            .objects
            .get_mut(&source)
            .expect("Teferi")
            .set_counter(CounterKind::Loyalty, 12);
        let returned_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
        let shuffled_nonland = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
        (engine, source, returned_land, shuffled_nonland)
    }

    let (mut original, source, returned_land, shuffled_nonland) = setup();
    let mut accepted_commands = Vec::new();
    let mut expected_batches = Vec::new();

    let activate = activate_ability_for(&original, source, 2, target_player(20));
    expected_batches.push(original.apply_command(10, &activate).unwrap());
    accepted_commands.push((10, activate));
    assert_eq!(original.state.objects[&source].zone, Zone::Graveyard);

    for _ in 0..12 {
        if original.state.pending_resolution.is_some() {
            break;
        }
        let actor = original.state.priority_player_id();
        let command = pass();
        expected_batches.push(original.apply_command(actor, &command).unwrap());
        accepted_commands.push((actor, command));
    }
    assert_eq!(
        original
            .state
            .pending_resolution
            .as_ref()
            .expect("the opponent chooses during resolution")
            .deciding_player,
        20
    );

    let choice = submit_resolution_choice(vec![returned_land]);
    expected_batches.push(original.apply_command(20, &choice).unwrap());
    accepted_commands.push((20, choice));
    assert_eq!(original.state.objects[&returned_land].zone, Zone::Hand);
    assert_eq!(
        original.state.objects[&shuffled_nonland].zone,
        Zone::Library
    );
    assert!(original.state.pending_resolution.is_none());

    let (mut replay, replay_source, replay_land, replay_nonland) = setup();
    assert_eq!(
        (source, returned_land, shuffled_nonland),
        (replay_source, replay_land, replay_nonland),
        "the same seed and setup must assign identical object identities"
    );
    for ((actor, command), expected) in accepted_commands.iter().zip(&expected_batches) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(*actor, &decoded).unwrap(), *expected);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        original.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn teferis_zero_draws_and_puts_a_loyalty_counter_on_each_draw() {
    let mut engine = multiplayer_engine(26_106_002);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 4);
    let starting_library = engine.state.players[0].library.len();

    engine
        .apply_command(10, &activate_ability_for(&engine, source, 0, Vec::new()))
        .expect("activate Teferi's draw ability");
    resolve_current_stack(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), starting_library - 1);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn teferis_minus_two_creates_a_vigilant_spirit_that_grows_on_each_draw() {
    let mut engine = multiplayer_engine(26_106_003);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 4);
    engine
        .apply_command(10, &activate_ability_for(&engine, source, 1, Vec::new()))
        .expect("activate Teferi's Spirit ability");
    resolve_current_stack(&mut engine);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| {
            engine.state.objects[object_id].card_id == "spirit_u_2_2_vigilance_draw_counter"
        })
        .expect("the blue Spirit token");
    let token_characteristics = engine
        .characteristics(token)
        .expect("Spirit characteristics");
    assert_eq!(token_characteristics.power, Some(2));
    assert_eq!(token_characteristics.toughness, Some(2));
    assert!(token_characteristics
        .colors
        .contains(&tricerules_cards::primitives::Color::Blue));
    assert!(token_characteristics.has_keyword(tricerules_cards::primitives::Keyword::Vigilance));

    inject_card_into_hand(&mut engine, 0, "divination");
    let divination_index = (engine.state.players[0].hand.len() - 1) as u32;
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.players[0].mana_pool.blue = 1;
    engine
        .apply_command(10, &cast_spell(divination_index as usize, Vec::new()))
        .expect("cast Divination to draw two cards");
    resolve_current_stack(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        4,
        "Teferi's trigger resolves once for each successful draw"
    );
    assert_eq!(
        engine.state.objects[&token].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the token trigger resolves once for each successful draw"
    );
}

#[test]
fn target_departure_assigns_a_replacement_chooser_and_limits_it_to_the_lki_control_cohort() {
    let teferi = tricerules_cards::registry::global()
        .get(TEFERI)
        .expect("Teferi, Temporal Pilgrim needs a complete definition");
    let mut engine = multiplayer_engine(26_106_001);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 12);
    let target_owned = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let foreign_owned_controlled_by_target =
        inject_permanent_on_battlefield(&mut engine, 2, "bonesplitter");
    {
        let object = engine
            .state
            .objects
            .get_mut(&foreign_owned_controlled_by_target)
            .expect("foreign-owned permanent controlled by the target");
        object.base_controller = 20;
        object.controller = 20;
    }
    let stolen_survivor = inject_permanent_on_battlefield(&mut engine, 3, "bonesplitter");
    give_control_to(&mut engine, stolen_survivor, 20);

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 2, target_player(20)),
        )
        .expect("activate the targeted-player ultimate");
    resolve_until_permanent_choice(&mut engine);
    let departure = engine.apply_command(20, &concede()).unwrap();

    assert!(!engine.state.objects.contains_key(&target_owned));
    assert_eq!(
        engine.state.objects[&foreign_owned_controlled_by_target].zone,
        Zone::Exile,
        "CR 800.4a exiles a foreign-owned permanent still controlled by the departing target"
    );
    assert_eq!(
        engine.state.objects[&stolen_survivor].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&stolen_survivor].controller, 40);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the source controller must choose a replacement chooser");
    assert_eq!(pending.deciding_player, 10);
    let delegate_event = find_resolution_choice(&departure).expect("replacement chooser event");
    assert_eq!(delegate_event.deciding_player_id, 10);
    assert_eq!(
        delegate_event.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ResolutionBranch as i32
    );
    assert_eq!(delegate_event.resolution_branches.len(), 2);

    engine
        .apply_command(10, &select_replacement_chooser(0))
        .expect("the source controller assigns another opponent");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("replacement chooser gets the permanent choice");
    assert_eq!(pending.deciding_player, 30);
    assert_eq!(pending.presentation.candidates, [stolen_survivor]);

    let batch = engine
        .apply_command(30, &targeted_player_choice_answer(vec![stolen_survivor]))
        .expect("the replacement chooser selects the surviving cohort permanent");
    assert_eq!(engine.state.objects[&stolen_survivor].zone, Zone::Hand);
    let owner_shuffle_logs = batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                if log.text.contains("shuffles their library") =>
            {
                Some(log.text.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owner_shuffle_logs,
        vec![
            "P20 shuffles their library.".to_string(),
            "P30 shuffles their library.".to_string(),
            "P40 shuffles their library.".to_string(),
        ],
        "the captured specific-object set still names surviving P30's library after 800.4a exiles their permanent"
    );
    assert_eq!(teferi.primary_face().activated_abilities.len(), 3);
}

#[test]
fn owner_departure_refreshes_the_target_choice_and_rejects_the_removed_candidate() {
    let mut engine = multiplayer_engine(26_106_004);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 12);
    let chosen_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let owner_leaves_nonland = inject_permanent_on_battlefield(&mut engine, 2, "bonesplitter");
    let surviving_nonland = inject_permanent_on_battlefield(&mut engine, 3, "sol_ring");
    give_control_to(&mut engine, owner_leaves_nonland, 20);
    give_control_to(&mut engine, surviving_nonland, 20);

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 2, target_player(20)),
        )
        .expect("activate the targeted-player ultimate");
    let initial_choice = resolve_until_permanent_choice(&mut engine);
    assert!(initial_choice
        .candidate_object_ids
        .contains(&owner_leaves_nonland));
    let departure = engine.apply_command(30, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&owner_leaves_nonland));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original target remains the chooser");
    assert_eq!(pending.deciding_player, 20);
    assert!(!pending
        .presentation
        .candidates
        .contains(&owner_leaves_nonland));
    let refreshed_choice = find_resolution_choice(&departure).expect("refreshed choice event");
    assert!(!refreshed_choice
        .candidate_object_ids
        .contains(&owner_leaves_nonland));

    let command_index = engine.state.command_index;
    assert!(engine
        .apply_command(
            20,
            &targeted_player_choice_answer(vec![owner_leaves_nonland])
        )
        .is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert!(engine.state.pending_resolution.is_some());

    let batch = engine
        .apply_command(20, &targeted_player_choice_answer(vec![chosen_land]))
        .expect("the original target chooses a remaining permanent");
    assert_eq!(engine.state.objects[&chosen_land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&surviving_nonland].zone, Zone::Library);
    let owner_shuffle_logs = batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                if log.text.contains("shuffles their library") =>
            {
                Some(log.text.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owner_shuffle_logs,
        vec!["P40 shuffles their library.".to_string()],
        "the still-live target uses current control information, so the departed owner's absent permanent is no longer in the set"
    );
}

#[test]
fn live_target_choices_and_shuffle_use_current_control_after_another_player_leaves() {
    let mut engine = multiplayer_engine(26_106_007);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 12);
    let chosen_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let target_nonland = inject_permanent_on_battlefield(&mut engine, 1, "bonesplitter");
    let control_reverts_to_target = inject_permanent_on_battlefield(&mut engine, 3, "sol_ring");
    give_control_to(&mut engine, control_reverts_to_target, 20);
    give_control_to(&mut engine, control_reverts_to_target, 30);
    engine
        .state
        .continuous_effects
        .last_mut()
        .expect("the departing player's control effect")
        .timestamp += 1;

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 2, target_player(20)),
        )
        .expect("activate the targeted-player ultimate");
    let initial_choice = resolve_until_permanent_choice(&mut engine);
    assert!(initial_choice.candidate_object_ids.contains(&chosen_land));
    assert!(initial_choice
        .candidate_object_ids
        .contains(&target_nonland));
    assert!(!initial_choice
        .candidate_object_ids
        .contains(&control_reverts_to_target));

    let departure = engine.apply_command(30, &concede()).unwrap();
    assert_eq!(
        engine.state.objects[&control_reverts_to_target].controller, 20,
        "the departing player's control effect ends and the permanent reverts to its owner"
    );
    let refreshed_choice = find_resolution_choice(&departure).expect("refreshed choice event");
    assert!(refreshed_choice
        .candidate_object_ids
        .contains(&control_reverts_to_target));

    let batch = engine
        .apply_command(20, &targeted_player_choice_answer(vec![target_nonland]))
        .expect("the live target chooses one of their current permanents");
    assert_eq!(engine.state.objects[&target_nonland].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&chosen_land].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&control_reverts_to_target].zone,
        Zone::Library,
        "the later instruction enumerates the live target's current nonland permanents"
    );
    let owner_shuffle_logs = batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                if log.text.contains("shuffles their library") =>
            {
                Some(log.text.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owner_shuffle_logs,
        vec!["P40 shuffles their library.".to_string()],
        "the selected P20-owned nonland has left the current set, while the reverted P40-owned permanent remains in it"
    );
}

#[test]
fn a_target_who_leaves_before_resolution_makes_the_ultimate_fail_to_resolve() {
    let mut engine = multiplayer_engine(26_106_005);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 13);
    let stolen_permanent = inject_permanent_on_battlefield(&mut engine, 3, "bonesplitter");
    give_control_to(&mut engine, stolen_permanent, 20);

    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 2, target_player(20)),
        )
        .expect("activate Teferi's ultimate");
    engine.apply_command(20, &concede()).unwrap();
    resolve_current_stack(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        1,
        "the activation cost remains paid"
    );
    assert_eq!(
        engine.state.objects[&stolen_permanent].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&stolen_permanent].controller, 40);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn an_empty_target_control_cohort_skips_the_choice_and_shuffles_no_libraries() {
    let mut engine = multiplayer_engine(26_106_006);
    let source = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 13);
    engine
        .apply_command(
            10,
            &activate_ability_for(&engine, source, 2, target_player(20)),
        )
        .expect("activate Teferi's ultimate against the empty target battlefield");

    let events = resolve_current_stack_with_events(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!events.iter().any(|event| match event.ev.as_ref() {
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log)) => {
            log.text.contains("shuffles their library")
        }
        _ => false,
    }));
}
