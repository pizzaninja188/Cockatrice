//! Issue #116: live conditions on creature cohorts and automatic resolution branches.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    ChoiceKind, DevCommand, DevMoveCard, DevPutCardInZone, DevZone, ResolutionChoiceDecision,
    ResolutionChoiceRequired, RuledCommand, RuledEventBatch, SubmitResolutionChoice,
};

fn dev(target: i32, payload: Dev) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    }
}

fn put_ready(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::PutCardInZone(DevPutCardInZone {
            card_name: card_name.to_string(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        }),
    )
}

fn move_to_hand(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::MoveCard(DevMoveCard {
            card_name: card_name.to_string(),
            zone: DevZone::Hand as i32,
            ready: false,
        }),
    )
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![vec!["plains".into(); 12], vec!["forest".into(); 12]]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put(engine: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    engine
        .apply_scenario_command(player, &put_ready(player, card_name))
        .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(engine, player as usize, card_id)
}

fn cast_trade_route_envoy(engine: &mut GameEngine) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, "trade_route_envoy");
    grant_pool(engine, 0);
    let hand_index = hand_index_for_card(engine, 0, "trade_route_envoy");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Trade Route Envoy");
    pass_both_players(engine);
    assert_eq!(
        engine.state.objects.get(&object_id).expect("envoy").zone,
        tricerules_core::Zone::Battlefield
    );
    object_id
}

fn resolution_choices(batch: &RuledEventBatch) -> Vec<ResolutionChoiceRequired> {
    batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice.clone()),
            _ => None,
        })
        .collect()
}

fn stack_resolved_count(batch: &RuledEventBatch, stack_id: u32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id))
        .count()
}

fn choose_branch(offer: &ResolutionChoiceRequired, label: &str) -> RuledCommand {
    let branch = offer
        .resolution_branches
        .iter()
        .find(|branch| branch.label == label)
        .unwrap_or_else(|| panic!("offered branch {label:?} is missing"));
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: offer.resolution_prompt_id,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch.branch_index,
            ..Default::default()
        })),
    }
}

#[test]
fn inspiring_paladin_conditions_its_self_and_countered_creature_first_strike() {
    let mut engine = engine(116_001);
    let paladin = put(&mut engine, 0, "Inspiring Paladin", "inspiring_paladin");
    let countered = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    let uncountered = put(&mut engine, 0, "Air Elemental", "air_elemental");
    engine
        .state
        .objects
        .get_mut(&countered)
        .expect("countered creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);

    assert!(engine.effective_has_keyword(paladin, Keyword::FirstStrike));
    assert!(engine.effective_has_keyword(countered, Keyword::FirstStrike));
    assert!(!engine.effective_has_keyword(uncountered, Keyword::FirstStrike));

    engine.state.active_player_idx = 1;
    assert!(!engine.effective_has_keyword(paladin, Keyword::FirstStrike));
    assert!(!engine.effective_has_keyword(countered, Keyword::FirstStrike));

    engine.state.active_player_idx = 0;
    engine
        .apply_scenario_command(0, &move_to_hand(0, "Inspiring Paladin"))
        .expect("remove anthem source");
    assert!(!engine.effective_has_keyword(countered, Keyword::FirstStrike));
}

/// CR 510.4: a creature with first strike deals its combat damage in the first combat damage
/// step and does not deal damage again in the regular combat damage step.
#[test]
fn inspiring_paladin_grant_changes_the_combat_damage_step() {
    let mut engine = engine(116_002);
    put(&mut engine, 0, "Inspiring Paladin", "inspiring_paladin");
    let attacker = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&attacker)
        .expect("attacker")
        .set_counter(CounterKind::PlusOnePlusOne, 1);

    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare countered attacker");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage
    );
    assert_eq!(engine.state.players[1].life, 17);

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 17,
        "the first-strike attacker must not deal damage again in the regular step"
    );
}

#[test]
fn trade_route_envoy_uses_live_counter_state_without_prompting() {
    let mut draw_engine = engine(116_010);
    let qualifier = put(&mut draw_engine, 0, "Grizzly Bears", "grizzly_bears");
    let envoy = cast_trade_route_envoy(&mut draw_engine);
    draw_engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let hand_before = draw_engine.state.players[0].hand.len();

    let draw_batch = pass_both_players_with_batch(&mut draw_engine);

    assert!(resolution_choices(&draw_batch).is_empty());
    assert!(draw_engine.state.pending_resolution.is_none());
    assert_eq!(draw_engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(
        draw_engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let mut fallback_engine = engine(116_011);
    let qualifier = put(&mut fallback_engine, 0, "Grizzly Bears", "grizzly_bears");
    fallback_engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let envoy = cast_trade_route_envoy(&mut fallback_engine);
    fallback_engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    let hand_before = fallback_engine.state.players[0].hand.len();

    let fallback_batch = pass_both_players_with_batch(&mut fallback_engine);

    assert!(resolution_choices(&fallback_batch).is_empty());
    assert!(fallback_engine.state.pending_resolution.is_none());
    assert_eq!(fallback_engine.state.players[0].hand.len(), hand_before);
    assert_eq!(
        fallback_engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn trade_route_envoy_uses_fallback_when_narset_blocks_the_draw() {
    let mut engine = engine(116_012);
    put(
        &mut engine,
        1,
        "Narset, Parter of Veils",
        "narset,_parter_of_veils",
    );
    let qualifier = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine.state.turn_history.current.player_mut(0).cards_drawn = 1;

    let envoy = cast_trade_route_envoy(&mut engine);
    let hand_before = engine.state.players[0].hand.len();
    let batch = pass_both_players_with_batch(&mut engine);

    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(
        engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        1,
        "a draw blocked by Narset must leave the fallback counter on Envoy"
    );
}

#[test]
fn trade_route_envoy_ignores_counters_on_opponent_creatures() {
    let mut engine = engine(116_013);
    let opponent_creature = put(&mut engine, 1, "Grizzly Bears", "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&opponent_creature)
        .expect("opponent creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let envoy = cast_trade_route_envoy(&mut engine);
    let hand_before = engine.state.players[0].hand.len();

    let batch = pass_both_players_with_batch(&mut engine);

    assert!(resolution_choices(&batch).is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(
        engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine
            .state
            .objects
            .get(&opponent_creature)
            .expect("opponent creature")
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn trade_route_envoy_source_with_any_counter_qualifies_for_draw() {
    let mut engine = engine(116_014);
    let envoy = cast_trade_route_envoy(&mut engine);
    engine
        .state
        .objects
        .get_mut(&envoy)
        .expect("envoy")
        .set_counter(CounterKind::Charge, 1);
    let hand_before = engine.state.players[0].hand.len();

    let batch = pass_both_players_with_batch(&mut engine);

    assert!(resolution_choices(&batch).is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    let source = engine.state.objects.get(&envoy).expect("envoy");
    assert_eq!(source.counter_count(CounterKind::Charge), 1);
    assert_eq!(source.counter_count(CounterKind::PlusOnePlusOne), 0);
}

#[test]
fn trade_route_envoy_draw_resumes_abundance_replacement_once() {
    let mut engine = engine(116_015);
    put(&mut engine, 0, "Abundance", "abundance");
    let qualifier = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let top_nonland = inject_library_card(&mut engine, 0, "air_elemental");
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != top_nonland);
    engine.state.players[0].library.push_front(top_nonland);
    let envoy = cast_trade_route_envoy(&mut engine);
    let trigger_stack_id = engine
        .state
        .stack
        .iter()
        .rev()
        .find(|item| item.is_triggered && item.source_permanent_id == Some(envoy))
        .expect("Envoy's triggered ability on the stack")
        .id;
    assert_ne!(
        trigger_stack_id, envoy,
        "trigger and card have distinct IDs"
    );
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    let offered_replacement = pass_both_players_with_batch(&mut engine);
    assert_eq!(
        stack_resolved_count(&offered_replacement, trigger_stack_id),
        0
    );
    let replacement_choices = resolution_choices(&offered_replacement);
    assert_eq!(replacement_choices.len(), 1);
    let replace_draw = &replacement_choices[0];
    assert_eq!(
        replace_draw.choice_kind,
        ChoiceKind::ResolutionBranch as i32
    );
    assert_eq!(
        replace_draw.prompt_text,
        "Choose whether to replace this draw."
    );
    assert!(replace_draw
        .resolution_branches
        .iter()
        .any(|branch| branch.label == "Replace this draw"));
    let parked = engine
        .apply_scenario_command(0, &choose_branch(replace_draw, "Replace this draw"))
        .expect("accept Abundance's draw replacement");
    assert_eq!(stack_resolved_count(&parked, trigger_stack_id), 0);
    assert!(engine.state.pending_resolution.is_some());
    let kind_choices = resolution_choices(&parked);
    assert_eq!(kind_choices.len(), 1);
    let choose_kind = &kind_choices[0];
    assert_eq!(choose_kind.choice_kind, ChoiceKind::ResolutionBranch as i32);
    assert_eq!(choose_kind.prompt_text, "Choose land or nonland.");
    assert!(choose_kind
        .resolution_branches
        .iter()
        .any(|branch| branch.label == "Nonland"));

    let resumed = engine
        .apply_scenario_command(0, &choose_branch(choose_kind, "Nonland"))
        .expect("choose the offered nonland replacement");

    assert_eq!(stack_resolved_count(&resumed, trigger_stack_id), 1);
    assert!(resolution_choices(&resumed).is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.players[0].hand.contains(&top_nonland));
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "Abundance replaces the attempted draw without a CardDrawn event"
    );
    let source = engine.state.objects.get(&envoy).expect("envoy");
    assert_eq!(source.counter_count(CounterKind::PlusOnePlusOne), 0);
}

#[test]
fn trade_route_envoy_empty_library_attempt_does_not_add_fallback_counter() {
    let mut engine = engine(116_016);
    let qualifier = put(&mut engine, 0, "Grizzly Bears", "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let envoy = cast_trade_route_envoy(&mut engine);
    engine.state.players[0].library.clear();
    let hand_before = engine.state.players[0].hand.len();

    let batch = pass_both_players_with_batch(&mut engine);

    assert!(resolution_choices(&batch).is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].has_lost);
    assert!(
        !engine.state.players[0].pending_library_loss,
        "the command boundary commits the pending empty-library loss"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(
        engine
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the permitted draw attempt is not converted into the Otherwise effect"
    );
}

#[test]
fn trade_route_envoy_replay_matches_exact_batches_and_diagnostic_state() {
    let mut fixture = engine(116_017);
    let qualifier = put(&mut fixture, 0, "Grizzly Bears", "grizzly_bears");
    fixture
        .state
        .objects
        .get_mut(&qualifier)
        .expect("qualifying creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let envoy = inject_card_into_hand(&mut fixture, 0, "trade_route_envoy");
    grant_pool(&mut fixture, 0);
    let hand_before = fixture.state.players[0].hand.len();
    let mut first = fixture.clone();
    let mut replay = fixture;
    let envoy_hand_index = hand_index_for_card(&first, 0, "trade_route_envoy");
    let accepted_commands = vec![
        (0, cast_spell(envoy_hand_index, Vec::new())),
        (0, pass()),
        (1, pass()),
        (0, pass()),
        (1, pass()),
    ];

    for (player, command) in accepted_commands {
        let first_batch = first
            .apply_scenario_command(player, &command)
            .expect("first accepted command");
        let replay_batch = replay
            .apply_scenario_command(player, &command)
            .expect("replayed accepted command");
        assert_eq!(first_batch, replay_batch);
        assert_eq!(
            first
                .diagnostic_snapshot()
                .expect("first deterministic snapshot"),
            replay
                .diagnostic_snapshot()
                .expect("replayed deterministic snapshot")
        );
        assert!(resolution_choices(&first_batch).is_empty());
    }

    assert!(first.state.pending_resolution.is_none());
    assert_eq!(first.state.players[0].hand.len(), hand_before);
    assert_eq!(
        first
            .state
            .objects
            .get(&envoy)
            .expect("envoy")
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(first.state.turn_history.current.player(0).cards_drawn, 1);
}
