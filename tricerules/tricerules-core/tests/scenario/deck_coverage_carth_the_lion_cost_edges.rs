//! Carth's cost locking and replacement boundaries; synthetic fixtures are labeled below.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{AbilityCost, CounterKind};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

const CARTH: &str = "carth_the_lion";
const LAEZEL: &str = "laezel,_vlaakiths_champion";

fn game(seed: u64) -> GameEngine {
    assert!(
        tricerules_cards::registry::global().get(CARTH).is_some(),
        "Carth must be registered"
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("forest", &[]),
            deck_with("island", &[]),
            deck_with("swamp", &[]),
        ]),
        true,
    )
    .expect("three-player fixture");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let remaining = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        - engine.state.passes_since_stack_change as usize;
    let mut batch = RuledEventBatch::default();
    for _ in 0..remaining {
        let actor = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority");
    }
    batch
}

fn cast_carth(engine: &mut GameEngine) -> u32 {
    // Synthetic hand and mana; Carth's cast, entry and optional selection are real commands.
    let carth = inject_card_into_hand(engine, 0, CARTH);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 2,
            b: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, CARTH);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Carth");
    resolve_one(engine);
    assert_eq!(engine.state.objects[&carth].zone, Zone::Battlefield);
    let batch = resolve_one(engine);
    assert!(find_resolution_choice(&batch).is_some());
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline Carth's entry selection");
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
    carth
}

fn synthetic_control(engine: &mut GameEngine, object: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|id| *id != object);
    engine.state.players[to].battlefield.push(object);
    let player = engine.state.players[to].id;
    let permanent = engine.state.objects.get_mut(&object).unwrap();
    permanent.controller = player;
    permanent.base_controller = player;
}

fn staged_fixture(seed: u64, foreign_source: bool) -> (GameEngine, u32, u32, u32, u32) {
    let mut engine = game(seed);
    // Synthetic placement and control give P0 a Carth physically owned by P2.
    let carth = inject_creature_on_battlefield(&mut engine, 2, CARTH);
    synthetic_control(&mut engine, carth, 2, 0);
    let owner = if foreign_source { 2 } else { 0 };
    let source = inject_permanent_on_battlefield(&mut engine, owner, "arena");
    if foreign_source {
        synthetic_control(&mut engine, source, 2, 0);
    }
    // Arena's admitted staged two-target ability is copied to a synthetic planeswalker face
    // with the supported optional positive loyalty cost; the transaction itself is real.
    let mut face = tricerules_cards::registry::global()
        .get("arena")
        .unwrap()
        .primary_face()
        .clone();
    face.types = vec!["Planeswalker".into(), "Jace".into()];
    face.activated_abilities[0]
        .costs
        .push(AbilityCost::Loyalty(1));
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "arena".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Staged loyalty fixture".into(),
    });
    object.set_counter(CounterKind::Loyalty, 3);
    assert!(engine
        .characteristics(source)
        .unwrap()
        .has_type("Planeswalker"));
    assert_eq!(engine.characteristics(carth).unwrap().controller, 0);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    (engine, carth, source, own, opponent)
}

fn begin(engine: &GameEngine, source: u32, own: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
            source_object_id: source,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
            opponent_player_id: Some(1),
            own_target: Some(rv1::AbilityActivationTarget {
                object_id: own,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&own)
                    .copied()
                    .unwrap_or(0),
                group_index: 0,
            }),
        })),
    }
}

fn answer(engine: &GameEngine, opponent: u32) -> RuledCommand {
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    RuledCommand {
        cmd: Some(Cmd::SubmitAbilityActivationChoice(
            rv1::SubmitAbilityActivationChoice {
                transaction_id: pending.transaction_id,
                expected_revision: pending.revision,
                target: Some(rv1::AbilityActivationTarget {
                    object_id: opponent,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&opponent)
                        .copied()
                        .unwrap_or(0),
                    group_index: 1,
                }),
                ..Default::default()
            },
        )),
    }
}

fn commit(engine: &GameEngine) -> RuledCommand {
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    RuledCommand {
        cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
            ..Default::default()
        })),
    }
}

fn cancel(engine: &GameEngine) -> RuledCommand {
    let pending = engine.state.pending_ability_activation.as_ref().unwrap();
    RuledCommand {
        cmd: Some(Cmd::CancelAbilityActivation(rv1::CancelAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
        })),
    }
}

fn announce(engine: &mut GameEngine, source: u32, own: u32) {
    let command = begin(engine, source, own);
    engine
        .apply_scenario_command(0, &command)
        .expect("begin the staged activation");
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .stage(),
        rv1::AbilityActivationStage::OpponentTarget
    );
}

fn select_target(engine: &mut GameEngine, opponent: u32) {
    let command = answer(engine, opponent);
    engine
        .apply_scenario_command(1, &command)
        .expect("opponent selects their target");
    assert_eq!(
        engine
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .stage(),
        rv1::AbilityActivationStage::Payment
    );
}

fn pay(engine: &mut GameEngine) {
    let mut command = commit(engine);
    authoring_actions::pay(engine, 0, &mut command).expect("select the three mana payment");
    engine
        .apply_scenario_command(0, &command)
        .expect("commit the locked staged cost");
}

#[test]
fn carth_staged_cost_is_live_before_target_choice_and_locked_after_it() {
    for depart_before_target in [true, false] {
        let (mut engine, carth, source, own, opponent) =
            staged_fixture(2_610_271_040 + u64::from(depart_before_target), false);
        announce(&mut engine, source, own);
        if depart_before_target {
            engine
                .apply_scenario_command(2, &concede())
                .expect("Carth owner leaves before target choice");
        }
        select_target(&mut engine, opponent);
        if !depart_before_target {
            engine
                .apply_scenario_command(2, &concede())
                .expect("Carth owner leaves after the cost locks");
        }
        assert!(!engine.state.objects.contains_key(&carth));
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );
        assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
        pay(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Loyalty),
            if depart_before_target { 4 } else { 5 }
        );
        assert!(engine.state.objects[&source].tapped);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert!(engine.state.pending_ability_activation.is_none());
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(
            engine.state.stack[0]
                .activated_ability
                .as_ref()
                .unwrap()
                .costs
                .last(),
            Some(&AbilityCost::Loyalty(1))
        );
    }
}

#[test]
fn carth_staged_cancellation_and_retry_pay_only_the_successful_activation() {
    let (mut engine, _, source, own, opponent) = staged_fixture(2_610_271_042, false);
    announce(&mut engine, source, own);
    select_target(&mut engine, opponent);
    let command = cancel(&engine);
    engine
        .apply_scenario_command(0, &command)
        .expect("cancel the unpaid announcement");
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        3
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(
        !engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    announce(&mut engine, source, own);
    select_target(&mut engine, opponent);
    pay(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn carth_staged_source_owner_departure_abandons_only_the_unpaid_activation() {
    let (mut engine, _, source, own, opponent) = staged_fixture(2_610_271_043, true);
    announce(&mut engine, source, own);
    select_target(&mut engine, opponent);
    engine
        .apply_scenario_command(2, &concede())
        .expect("foreign source owner leaves");
    assert!(!engine.state.objects.contains_key(&source));
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(
        !engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
}

#[test]
fn carth_staged_actor_departure_clears_the_unpaid_activation() {
    let (mut engine, _, source, own, opponent) = staged_fixture(2_610_271_044, false);
    announce(&mut engine, source, own);
    select_target(&mut engine, opponent);
    engine
        .apply_scenario_command(0, &concede())
        .expect("the activating player leaves");
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_ability_activation.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.blocking_choice().is_none());
}

#[test]
fn carth_combines_positive_cost_before_laezel_and_season_replacements() {
    for with_laezel in [false, true] {
        let mut engine = game(2_610_271_045 + u64::from(with_laezel));
        cast_carth(&mut engine);
        // Synthetic permanent placements; the real printed static abilities handle the real cost.
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        if with_laezel {
            inject_creature_on_battlefield(&mut engine, 0, LAEZEL);
        }
        let walker = inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
        engine
            .state
            .objects
            .get_mut(&walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 3);
        apply_ability(&mut engine, 0, walker, 0, vec![]).expect("activate actual Jace");
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
            if with_laezel { 9 } else { 5 },
            "Season alone does not modify a cost; after Lae'zel it doubles the modified event"
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(
            engine.state.stack[0]
                .activated_ability
                .as_ref()
                .unwrap()
                .costs,
            [AbilityCost::Loyalty(1)]
        );
    }
}

#[test]
fn carth_retains_an_applied_replacement_when_its_owner_leaves_during_cost_choices() {
    let mut engine = game(2_610_271_047);
    cast_carth(&mut engine);
    // Synthetic P2-owned Lae'zel is controlled by P0; both actual Seasons remain with P0.
    let laezel = inject_creature_on_battlefield(&mut engine, 2, LAEZEL);
    synthetic_control(&mut engine, laezel, 2, 0);
    let first = inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let walker = inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    let generation = engine
        .state
        .zone_change_generation
        .get(&walker)
        .copied()
        .unwrap_or(0);
    let laezel_generation = engine
        .state
        .zone_change_generation
        .get(&laezel)
        .copied()
        .unwrap_or(0);
    let generation_text = generation.to_string();
    let laezel_generation_text = laezel_generation.to_string();
    apply_ability(&mut engine, 0, walker, 0, vec![]).expect("begin actual Jace activation");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two Seasons require an order");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.presentation.candidates.len(), 2);
    let snapshot = engine.diagnostic_snapshot().unwrap();
    let event = &snapshot["state"]["pending_replacement_event"]["value"]["events"][0];
    assert_eq!(
        event["recipient"]["value"]["object_id"].as_u64(),
        Some(walker as u64)
    );
    assert_eq!(
        event["recipient"]["value"]["zone_change_generation"].as_str(),
        Some(generation_text.as_str())
    );
    assert_eq!(event["actor"].as_i64(), Some(0));
    assert_eq!(event["origin"], "Cost");
    assert_eq!(event["original_counters"]["Loyalty"].as_u64(), Some(2));
    assert_eq!(event["counters"]["Loyalty"].as_u64(), Some(3));
    assert_eq!(
        event["applied_replacements"][0]["source_object_id"].as_u64(),
        Some(laezel as u64)
    );
    assert_eq!(
        event["applied_replacements"][0]["source_zone_change_generation"].as_str(),
        Some(laezel_generation_text.as_str())
    );
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        3
    );
    assert!(engine.state.stack.is_empty());
    engine
        .apply_scenario_command(2, &concede())
        .expect("Lae'zel's owner leaves after its replacement applied");
    assert!(!engine.state.objects.contains_key(&laezel));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("remaining Seasons still need ordering");
    assert_eq!(pending.presentation.candidates.len(), 2);
    let snapshot = engine.diagnostic_snapshot().unwrap();
    let batch = &snapshot["state"]["pending_replacement_event"]["value"];
    assert_eq!(
        batch["events"][0]["recipient"]["value"]["object_id"].as_u64(),
        Some(walker as u64)
    );
    assert_eq!(
        batch["events"][0]["recipient"]["value"]["zone_change_generation"].as_str(),
        Some(generation_text.as_str())
    );
    assert_eq!(
        batch["events"][0]["original_counters"]["Loyalty"].as_u64(),
        Some(2)
    );
    assert_eq!(batch["events"][0]["counters"]["Loyalty"].as_u64(), Some(3));
    assert_eq!(
        batch["events"][0]["applied_replacements"][0]["source_object_id"].as_u64(),
        Some(laezel as u64)
    );
    assert_eq!(
        batch["events"][0]["applied_replacements"][0]["source_zone_change_generation"].as_str(),
        Some(laezel_generation_text.as_str())
    );
    let remaining: std::collections::BTreeSet<_> = batch["applications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["replacement"]["source_object_id"].as_u64().unwrap() as u32)
        .collect();
    assert_eq!(
        remaining,
        [first, second]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    let selected = pending.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("finish the remaining live replacements using the accepted Lae'zel receipt");
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&walker)
            .copied()
            .unwrap_or(0),
        generation
    );
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        15
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.blocking_choice().is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(walker));
}

#[test]
fn carth_replacement_owner_departure_replays_accepted_commands() {
    use prost::Message;

    fn fixture() -> (GameEngine, u32, u32) {
        let mut engine = game(2_610_271_049);
        cast_carth(&mut engine);
        // Synthetic placements and control; the paid activation and concession are real commands.
        let laezel = inject_creature_on_battlefield(&mut engine, 2, LAEZEL);
        synthetic_control(&mut engine, laezel, 2, 0);
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        let walker = inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
        engine
            .state
            .objects
            .get_mut(&walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 3);
        (engine, laezel, walker)
    }
    fn apply_wire(
        first: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = first
            .apply_scenario_command(actor, &command)
            .expect("accepted command");
        let repeated = replay
            .apply_scenario_command(actor, &decoded)
            .expect("replayed accepted wire command");
        assert_eq!(actual, repeated);
    }

    let (mut first, laezel, walker) = fixture();
    let (mut replay, replay_laezel, replay_walker) = fixture();
    assert_eq!((laezel, walker), (replay_laezel, replay_walker));
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let activation = activate_ability_for(&first, walker, 0, vec![]);
    apply_wire(&mut first, &mut replay, 0, activation);
    assert_eq!(
        first
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .len(),
        2
    );
    apply_wire(&mut first, &mut replay, 2, concede());
    assert!(!first.state.objects.contains_key(&laezel));
    let pending = first
        .state
        .pending_resolution
        .as_ref()
        .expect("live Seasons still require a choice");
    let mut choice = submit_resolution_choice(vec![pending.presentation.candidates[0]]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = choice.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = pending.resolution_prompt_id.get();
    apply_wire(&mut first, &mut replay, 0, choice);
    assert_eq!(
        first.state.objects[&walker].counter_count(CounterKind::Loyalty),
        15
    );
    assert!(first.state.pending_resolution.is_none());
    assert!(first.state.blocking_choice().is_none());
    assert_eq!(first.state.stack.len(), 1);
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );

    let actor = first.state.priority_player_id();
    apply_wire(&mut first, &mut replay, actor, pass());
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn carth_applies_after_a_real_act_of_treason_changes_its_controller() {
    let mut engine = game(2_610_271_048);
    // Synthetic opponent placement; the control-changing spell and activation are actual commands.
    let carth = inject_creature_on_battlefield(&mut engine, 2, CARTH);
    let walker = inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    inject_card_into_hand(&mut engine, 0, "act_of_treason");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "act_of_treason");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(carth)))
        .expect("cast actual Act of Treason");
    resolve_one(&mut engine);
    assert_eq!(engine.characteristics(carth).unwrap().controller, 0);
    assert_eq!(engine.state.objects[&carth].owner, 2);
    apply_ability(&mut engine, 0, walker, 0, vec![])
        .expect("activate Jace under the borrowed Carth");
    assert_eq!(
        engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        5
    );
}
