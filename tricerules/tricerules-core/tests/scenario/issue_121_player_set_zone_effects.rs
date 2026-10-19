use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_card_model::{AbilityDefinitionId, AbilityId, CardFaceId, ProgramOwnerId};
use tricerules_core::{state::ResolutionContinuation, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, DevCommand, DevMoveCard, DevZone, RuledCommand,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("island".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    for player in 0..2 {
        let cleared: Vec<_> = engine.state.players[player].hand.drain(..).collect();
        engine.state.players[player].library.extend(cleared);
    }
    engine
}

fn engine_three(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[]),
        deck_with("island", &[]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("new three-player engine");
    advance_to_main1_from_game_start(&mut engine);
    for player in 0..3 {
        let cleared: Vec<_> = engine.state.players[player].hand.drain(..).collect();
        engine.state.players[player].library.extend(cleared);
    }
    engine
}

fn fanatic_program_owner() -> ProgramOwnerId {
    ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "fanatic_of_the_harrowing".into(),
        face_id: CardFaceId::new("fanatic_of_the_harrowing").unwrap(),
        ability_path: vec![AbilityId::new("triggered_01").unwrap()],
    })
}

fn legacy_burglar_rat_program_owner() -> ProgramOwnerId {
    ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "burglar_rat".into(),
        face_id: CardFaceId::new("burglar_rat").unwrap(),
        ability_path: vec![AbilityId::new("triggered_01").unwrap()],
    })
}

fn assert_legacy_burglar_rat_stack_item(stack_item: &tricerules_core::state::StackItem) {
    assert_eq!(
        stack_item.ability_program_owner(),
        Some(legacy_burglar_rat_program_owner()),
        "the resolving occurrence keeps Burglar Rat's exact triggered owner"
    );
    assert!(
        stack_item
            .triggered_ability
            .as_ref()
            .is_some_and(|ability| ability.program.is_none()),
        "Burglar Rat's stack occurrence carries its legacy effect-list ability"
    );
}

fn assert_fanatic_program_stack_item(stack_item: &tricerules_core::state::StackItem) {
    assert_eq!(
        stack_item.ability_program_owner(),
        Some(fanatic_program_owner()),
        "the resolving occurrence keeps Fanatic's exact triggered owner"
    );
    assert!(
        stack_item
            .triggered_ability
            .as_ref()
            .is_some_and(|ability| ability.program.is_some()),
        "the actual stack occurrence carries the authored Program"
    );
}

fn assert_fanatic_program_pending(engine: &GameEngine, deciding_player: i32) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Fanatic's Program is parked on a private choice");
    assert_eq!(pending.deciding_player, deciding_player);
    let stack = match &pending.continuation {
        ResolutionContinuation::PlayerSetDiscard { stack, .. }
        | ResolutionContinuation::DiscardReplacement { stack } => stack,
        other => panic!("Fanatic parked on an unexpected continuation: {other:?}"),
    };
    assert_fanatic_program_stack_item(&stack.item);
}

fn dev_move(player: i32, card_name: &str, zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: card_name.into(),
                zone: zone as i32,
                ..Default::default()
            })),
        })),
    }
}

fn cast_fanatic_and_resolve_creature(engine: &mut GameEngine) -> u32 {
    let fanatic = inject_card_into_hand(engine, 0, "fanatic_of_the_harrowing");
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "fanatic_of_the_harrowing");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Fanatic of the Harrowing");
    pass_both_players(engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");
    fanatic
}

fn cast_fanatic_and_resolve_creature_three_player(engine: &mut GameEngine) -> u32 {
    let fanatic = inject_card_into_hand(engine, 0, "fanatic_of_the_harrowing");
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "fanatic_of_the_harrowing");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Fanatic of the Harrowing");
    pass_priority_round(engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");
    fanatic
}

fn park_fanatic_each_player_discard_with_player_one_active(engine: &mut GameEngine) -> u32 {
    let fanatic = cast_fanatic_and_resolve_creature_three_player(engine);
    assert_fanatic_program_stack_item(engine.state.stack.last().unwrap());
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    pass_priority_round(engine);
    assert_fanatic_program_pending(engine, 1);
    fanatic
}

fn cast_creature_and_leave_etb_trigger(engine: &mut GameEngine, card_id: &str, mana: ManaGift) {
    inject_card_into_hand(engine, 0, card_id);
    give_mana(engine, 0, mana);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    pass_both_players(engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");
}

fn park_burglar_rat_each_opponent_discard_with_player_one_active(engine: &mut GameEngine) -> u32 {
    let rat = inject_card_into_hand(engine, 0, "burglar_rat");
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "burglar_rat");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast legacy EachOpponent discard ability source");
    pass_priority_round(engine);
    assert_eq!(engine.state.stack.len(), 1, "legacy ETB trigger is pending");

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("legacy discard is parked on P1's choice");
    assert_eq!(pending.deciding_player, 1);
    let ResolutionContinuation::PlayerSetDiscard { stack, .. } = &pending.continuation else {
        panic!("legacy EachOpponent discard uses the shared player-set continuation");
    };
    assert_legacy_burglar_rat_stack_item(&stack.item);
    rat
}

fn resolve_trigger_to_choice(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_scenario_command(1, &pass())
        .expect("resolve trigger to a choice")
}

#[test]
fn fanatic_collects_private_apnap_choices_before_committing_the_discard() {
    let mut engine = engine(121_101);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);

    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes");
    let first = engine
        .apply_scenario_command(1, &pass())
        .expect("resolve trigger to first private choice");
    let first_choice = find_resolution_choice(&first).expect("active player's discard choice");
    assert_eq!(first_choice.deciding_player_id, 0);
    assert_eq!(first_choice.choice_kind, ChoiceKind::HandCards as i32);
    assert_eq!((first_choice.min, first_choice.max), (1, 1));
    assert!(first_choice.candidate_object_ids.contains(&controller_card));
    assert!(!first_choice.candidate_object_ids.contains(&opponent_card));

    let second = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("record active player's hidden choice");
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Hand);
    let second_choice = find_resolution_choice(&second).expect("next player's discard choice");
    assert_eq!(second_choice.deciding_player_id, 1);
    assert!(second_choice.candidate_object_ids.contains(&opponent_card));
    assert!(!second_choice
        .candidate_object_ids
        .contains(&controller_card));

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("commit the complete discard action");
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].hand.len(),
        1,
        "Fanatic controller drew"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn fanatic_does_not_draw_when_only_the_opponent_discarded() {
    let mut engine = engine(121_102);
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);

    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("opponent discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("opponent discards");

    assert!(engine.state.players[0].hand.is_empty());
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
}

#[test]
fn player_set_discard_rejects_wrong_player_cardinality_and_stale_identity() {
    let mut engine = engine(121_103);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);
    resolve_trigger_to_choice(&mut engine);

    assert!(engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![controller_card]))
        .is_err());
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .is_err());
    *engine
        .state
        .zone_change_generation
        .entry(controller_card)
        .or_default() += 1;
    let error = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect_err("stale physical identity must fail closed");
    assert!(error.to_string().contains("stale"));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
}

#[test]
fn conceding_during_the_second_hidden_choice_commits_no_staged_discard() {
    let mut engine = engine(121_104);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);
    resolve_trigger_to_choice(&mut engine);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("stage the first choice");

    engine
        .apply_scenario_command(1, &concede())
        .expect("concede at any time");
    assert_eq!(engine.state.winner(), Some(0));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Hand);
}

#[test]
fn fanatic_each_player_choice_order_starts_at_the_active_player_in_three_player_game() {
    let mut engine = engine_three(121_105);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let active_player_card = inject_card_into_hand(&mut engine, 1, "island");
    let next_opponent_card = inject_card_into_hand(&mut engine, 2, "swamp");
    let fanatic = cast_fanatic_and_resolve_creature_three_player(&mut engine);
    assert_fanatic_program_stack_item(engine.state.stack.last().unwrap());
    assert_eq!(engine.state.objects[&fanatic].controller, 0);

    // Seed a different active seat while the real ETB trigger waits on the stack. This isolates
    // the Program's three-player active-first order from its controller-relative draw condition.
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    assert_eq!(engine.state.active_player_id(), 1);
    pass_priority_round(&mut engine);
    assert_fanatic_program_pending(&engine, 1);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::HandCards
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![active_player_card]
    );

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![active_player_card]))
        .expect("active player makes the first discard choice");
    assert_fanatic_program_pending(&engine, 2);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![next_opponent_card]
    );
    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![next_opponent_card]))
        .expect("next opponent makes the second discard choice");
    assert_fanatic_program_pending(&engine, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![controller_card]
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("trigger controller makes the last discard choice");

    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&active_player_card].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&next_opponent_card].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        1,
        "the controller draws once for the controller's one discard"
    );
    assert!(engine.state.players[1].hand.is_empty());
    assert!(engine.state.players[2].hand.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn fanatic_skips_choices_when_every_player_has_an_empty_hand() {
    let mut engine = engine(121_106);
    let _fanatic = cast_fanatic_and_resolve_creature(&mut engine);
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| player.hand.is_empty()));
    assert_fanatic_program_stack_item(engine.state.stack.last().unwrap());

    let batch = resolve_trigger_to_choice(&mut engine);

    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| player.hand.is_empty()));
}

#[test]
fn fanatic_source_can_leave_before_a_library_replaced_controller_discard_draws() {
    let mut engine = engine(121_107);
    let discard = inject_card_into_hand(&mut engine, 0, "island");
    inject_card_into_hand(&mut engine, 0, "library_of_leng");
    let library_of_leng = relocate_to_battlefield(&mut engine, 0, "library_of_leng", false);
    let sentinel = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0]
        .library
        .retain(|oid| *oid != sentinel);
    engine.state.players[0].library.push_front(sentinel);
    let fanatic = cast_fanatic_and_resolve_creature(&mut engine);
    assert_eq!(
        engine.state.objects[&library_of_leng].zone,
        Zone::Battlefield
    );

    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &dev_move(0, "Fanatic of the Harrowing", DevZone::Graveyard),
        )
        .expect("the source leaves after its trigger is on the stack");
    assert_eq!(engine.state.objects[&fanatic].zone, Zone::Graveyard);
    assert_fanatic_program_stack_item(engine.state.stack.last().unwrap());

    let batch = resolve_trigger_to_choice(&mut engine);
    let discard_choice = find_resolution_choice(&batch).expect("controller discard choice");
    assert_eq!(discard_choice.deciding_player_id, 0);
    assert_eq!(discard_choice.candidate_object_ids, vec![discard]);
    assert_fanatic_program_pending(&engine, 0);
    let replacement = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect("controller makes the discard choice");
    let replacement_choice = find_resolution_choice(&replacement)
        .expect("Library of Leng offers its private destination replacement");
    assert_eq!(replacement_choice.deciding_player_id, 0);
    assert_eq!(
        replacement_choice.choice_kind,
        ChoiceKind::PrivateReplacement as i32
    );
    assert_fanatic_program_pending(&engine, 0);

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("put the hidden discard on top with Library of Leng");

    assert_eq!(engine.state.objects[&discard].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&discard));
    assert_eq!(
        engine.state.players[0].hand.len(),
        1,
        "the hidden discard was counted and drawn exactly once"
    );
    assert_eq!(engine.state.players[0].library.front(), Some(&sentinel));
    assert_eq!(engine.state.objects[&sentinel].zone, Zone::Library);
    assert!(!engine.state.players[0].graveyard.contains(&discard));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn fanatic_skips_a_departed_opponent_after_another_players_choice_is_pending() {
    let mut engine = engine_three(121_108);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let deciding_opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    let departing_opponent_card = inject_card_into_hand(&mut engine, 2, "swamp");
    let fanatic = cast_fanatic_and_resolve_creature_three_player(&mut engine);
    assert_fanatic_program_stack_item(engine.state.stack.last().unwrap());

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    pass_priority_round(&mut engine);
    assert_fanatic_program_pending(&engine, 1);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![deciding_opponent_card]
    );

    engine
        .apply_scenario_command(2, &concede())
        .expect("the other opponent concedes while P1's choice is pending");
    assert!(engine.state.players[2].has_lost);
    assert!(!engine.state.objects.contains_key(&departing_opponent_card));
    assert_fanatic_program_pending(&engine, 1);
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![deciding_opponent_card]))
        .expect("the surviving opponent completes its own choice");

    assert_fanatic_program_pending(&engine, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![controller_card]
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("the trigger controller makes the remaining choice");

    assert_eq!(engine.state.objects[&fanatic].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&deciding_opponent_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.players[0].hand.len(), 1);
    assert!(engine.state.players[1].hand.is_empty());
    assert!(engine.state.players[2].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn fanatic_skips_a_departed_current_chooser_and_continues_with_the_next_player() {
    let mut engine = engine_three(121_109);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let departing_chooser_card = inject_card_into_hand(&mut engine, 1, "island");
    let next_chooser_card = inject_card_into_hand(&mut engine, 2, "swamp");
    let fanatic = park_fanatic_each_player_discard_with_player_one_active(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![departing_chooser_card]
    );

    let departed = engine
        .apply_scenario_command(1, &concede())
        .expect("the current chooser leaves before making a selection");
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&departing_chooser_card));
    assert_fanatic_program_pending(&engine, 2);
    let next_choice = find_resolution_choice(&departed).expect("next live player is offered");
    assert_eq!(next_choice.deciding_player_id, 2);
    assert_eq!(next_choice.candidate_object_ids, vec![next_chooser_card]);

    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![next_chooser_card]))
        .expect("the next live player chooses");
    assert_fanatic_program_pending(&engine, 0);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("the surviving trigger controller chooses");

    assert_eq!(engine.state.objects[&fanatic].zone, Zone::Battlefield);
    assert!(!engine.state.objects.contains_key(&departing_chooser_card));
    assert_eq!(
        engine.state.objects[&next_chooser_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), 1);
    assert!(engine.state.players[1].hand.is_empty());
    assert!(engine.state.players[2].hand.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn fanatic_drops_an_accepted_discard_when_its_player_leaves_before_commit() {
    let mut engine = engine_three(121_111);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let accepted_member_card = inject_card_into_hand(&mut engine, 1, "island");
    let current_chooser_card = inject_card_into_hand(&mut engine, 2, "swamp");
    let fanatic = park_fanatic_each_player_discard_with_player_one_active(&mut engine);

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![accepted_member_card]))
        .expect("P1's choice is recorded but the group has not committed");
    assert_eq!(engine.state.objects[&accepted_member_card].zone, Zone::Hand);
    assert_fanatic_program_pending(&engine, 2);

    engine
        .apply_scenario_command(1, &concede())
        .expect("an accepted member leaves before the cohort commits");
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&accepted_member_card));
    assert_fanatic_program_pending(&engine, 2);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![current_chooser_card]
    );

    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![current_chooser_card]))
        .expect("the surviving current player chooses");
    assert_fanatic_program_pending(&engine, 0);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("the trigger controller makes the last choice");

    assert_eq!(engine.state.objects[&fanatic].zone, Zone::Battlefield);
    assert!(!engine.state.objects.contains_key(&accepted_member_card));
    assert_eq!(
        engine.state.objects[&current_chooser_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn begun_fanatic_ability_finishes_after_its_controller_concedes_without_drawing() {
    let mut engine = engine_three(121_112);
    let departed_controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let accepted_member_card = inject_card_into_hand(&mut engine, 1, "island");
    let current_chooser_card = inject_card_into_hand(&mut engine, 2, "swamp");
    let fanatic = park_fanatic_each_player_discard_with_player_one_active(&mut engine);
    let controller_draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![accepted_member_card]))
        .expect("the first live opponent's mandatory choice is accepted");
    assert_fanatic_program_pending(&engine, 2);
    engine
        .apply_scenario_command(0, &concede())
        .expect("the trigger controller leaves after the ability has begun resolving");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&fanatic));
    assert!(!engine.state.objects.contains_key(&departed_controller_card));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun ability-owned player set continues for surviving recipients");
    assert_eq!(pending.deciding_player, 2);
    let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation else {
        panic!("the begun Fanatic Program remains on its player-set discard");
    };
    assert_eq!(stack.item.controller, 0, "retain the original controller");
    assert_eq!(
        stack.item.ability_program_owner(),
        Some(fanatic_program_owner())
    );
    assert_eq!(discard.current, 1);
    assert_eq!(discard.selections, vec![vec![accepted_member_card]]);

    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![current_chooser_card]))
        .expect("the remaining opponent completes the begun ability");

    assert_eq!(
        engine.state.objects[&accepted_member_card].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&current_chooser_card].zone,
        Zone::Graveyard
    );
    assert!(!engine.state.objects.contains_key(&departed_controller_card));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        controller_draws_before,
        "a departed controller receives no trailing Program draw"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn fanatic_commits_prior_discards_when_final_controller_concedes() {
    let mut engine = engine_three(121_115);
    let departed_controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let first_discard = inject_card_into_hand(&mut engine, 1, "island");
    let second_discard = inject_card_into_hand(&mut engine, 2, "swamp");
    let fanatic = park_fanatic_each_player_discard_with_player_one_active(&mut engine);
    let controller_draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    let first_discard_generation = engine
        .state
        .zone_change_generation
        .get(&first_discard)
        .copied()
        .unwrap_or_default();
    let second_discard_generation = engine
        .state
        .zone_change_generation
        .get(&second_discard)
        .copied()
        .unwrap_or_default();

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![first_discard]))
        .expect("P1 stages its discard");
    assert_eq!(engine.state.objects[&first_discard].zone, Zone::Hand);
    assert_fanatic_program_pending(&engine, 2);
    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![second_discard]))
        .expect("P2 stages its discard");
    assert_eq!(engine.state.objects[&second_discard].zone, Zone::Hand);
    let final_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the controller is the final chooser")
        .clone();
    assert_eq!(final_choice.deciding_player, 0);
    assert_eq!(
        final_choice.presentation.candidates,
        vec![departed_controller_card]
    );
    let ResolutionContinuation::PlayerSetDiscard { discard, .. } = &final_choice.continuation
    else {
        panic!("Fanatic's final choice remains on its player-set discard");
    };
    assert_eq!(
        discard.selections,
        vec![vec![first_discard], vec![second_discard]]
    );

    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("the final chooser and trigger controller concedes");

    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&fanatic));
    assert!(!engine.state.objects.contains_key(&departed_controller_card));
    for (player, card) in [(1, first_discard), (2, second_discard)] {
        assert_eq!(engine.state.objects[&card].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.players[player]
                .graveyard
                .iter()
                .filter(|&&object_id| object_id == card)
                .count(),
            1,
            "each staged discard commits exactly once"
        );
    }
    assert_eq!(
        engine.state.zone_change_generation[&first_discard],
        first_discard_generation + 1
    );
    assert_eq!(
        engine.state.zone_change_generation[&second_discard],
        second_discard_generation + 1
    );
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        controller_draws_before,
        "the departed controller receives no draw"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(
        find_resolution_choice(&departed).is_none(),
        "the completed player-set discard leaves no choice event"
    );
}

#[test]
fn legacy_burglar_rat_skips_a_departed_current_player_set_chooser() {
    let mut engine = engine_three(121_113);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let departing_chooser_card = inject_card_into_hand(&mut engine, 1, "island");
    let next_chooser_card = inject_card_into_hand(&mut engine, 2, "swamp");
    park_burglar_rat_each_opponent_discard_with_player_one_active(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("legacy discard is parked on P1's choice");
    assert_eq!(
        pending.presentation.candidates,
        vec![departing_chooser_card]
    );

    let departed = engine
        .apply_scenario_command(1, &concede())
        .expect("the current legacy chooser leaves before selecting");
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&departing_chooser_card));
    let next_choice = find_resolution_choice(&departed).expect("next opponent is offered");
    assert_eq!(next_choice.deciding_player_id, 2);
    assert_eq!(next_choice.candidate_object_ids, vec![next_chooser_card]);
    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![next_chooser_card]))
        .expect("the remaining opponent discards");

    assert_eq!(
        engine.state.objects[&next_chooser_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
    assert!(!engine.state.objects.contains_key(&departing_chooser_card));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn begun_legacy_burglar_rat_ability_finishes_after_its_controller_concedes() {
    let mut engine = engine_three(121_114);
    let departed_controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let accepted_member_card = inject_card_into_hand(&mut engine, 1, "island");
    let current_chooser_card = inject_card_into_hand(&mut engine, 2, "swamp");
    let rat = park_burglar_rat_each_opponent_discard_with_player_one_active(&mut engine);

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![accepted_member_card]))
        .expect("the first opponent's mandatory choice is accepted");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the next opponent is choosing");
    assert_eq!(pending.deciding_player, 2);
    let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation else {
        panic!("legacy EachOpponent discard remains on the shared continuation");
    };
    assert_eq!(stack.item.controller, 0);
    assert_legacy_burglar_rat_stack_item(&stack.item);
    assert_eq!(discard.selections, vec![vec![accepted_member_card]]);

    engine
        .apply_scenario_command(0, &concede())
        .expect("the begun legacy ability's controller concedes");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.objects.contains_key(&rat));
    assert!(!engine.state.objects.contains_key(&departed_controller_card));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun ability-owned discard continues for the remaining opponent");
    assert_eq!(pending.deciding_player, 2);
    let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation else {
        panic!("the begun legacy ability remains on its player-set discard");
    };
    assert_eq!(stack.item.controller, 0, "retain the original controller");
    assert_legacy_burglar_rat_stack_item(&stack.item);
    assert_eq!(discard.current, 1);
    assert_eq!(discard.selections, vec![vec![accepted_member_card]]);

    engine
        .apply_scenario_command(2, &submit_resolution_choice(vec![current_chooser_card]))
        .expect("the surviving opponent finishes the begun ability");

    assert_eq!(
        engine.state.objects[&accepted_member_card].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&current_chooser_card].zone,
        Zone::Graveyard
    );
    assert!(!engine.state.objects.contains_key(&departed_controller_card));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn burglar_rat_discards_only_from_each_opponent() {
    let mut engine = engine(121_110);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_creature_and_leave_etb_trigger(
        &mut engine,
        "burglar_rat",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("opponent discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&opponent_card));
    assert!(!choice.candidate_object_ids.contains(&controller_card));
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("opponent discards");
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
}

#[test]
fn friendly_teddy_death_trigger_draws_for_each_player() {
    let mut engine = engine(121_120);
    inject_card_into_hand(&mut engine, 0, "friendly_teddy");
    let teddy = relocate_to_battlefield(&mut engine, 0, "friendly_teddy", false);
    let hands_before = [
        engine.state.players[0].hand.len(),
        engine.state.players[1].hand.len(),
    ];
    engine.state.objects.get_mut(&teddy).unwrap().damage = 99;
    engine
        .apply_scenario_command(0, &pass())
        .expect("SBA destroys Teddy");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&teddy].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hands_before[0] + 1);
    assert_eq!(engine.state.players[1].hand.len(), hands_before[1] + 1);
}

#[test]
fn macabre_waltz_discards_with_zero_targets() {
    let mut engine = engine(121_130);
    let discard = inject_card_into_hand(&mut engine, 0, "forest");
    inject_card_into_hand(&mut engine, 0, "macabre_waltz");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "macabre_waltz");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast with zero targets");
    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("mandatory self-discard");
    assert_eq!(choice.candidate_object_ids, vec![discard]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect("discard despite choosing zero targets");
    assert_eq!(engine.state.objects[&discard].zone, Zone::Graveyard);
}

#[test]
fn macabre_waltz_can_discard_the_creature_it_returned() {
    let mut engine = engine(121_131);
    let returned = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "macabre_waltz");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "macabre_waltz");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(returned)))
        .expect("cast targeting one creature card");
    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("discard after return");
    assert!(choice.candidate_object_ids.contains(&returned));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![returned]))
        .expect("discard the returned creature");
    assert_eq!(engine.state.objects[&returned].zone, Zone::Graveyard);
}
