use super::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration, StaticAbilityDef};
use tricerules_cards::{CardRegistry, CounterKind, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, dev_command, CostChoiceKind, CostObjectRef, CostObjectRefs,
    CostSelection, DevCommand, DevMoveCard, DevZone,
};

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn tap_selection(engine: &GameEngine, object_id: u32) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation: generation(engine, object_id),
            }],
        })),
    }
}

fn activate_station(engine: &GameEngine, station: u32, crew: u32) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(station, 0, vec![], vec![tap_selection(engine, crew)]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, station);
    command
}

fn station_engine(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wurmwall_sweeper"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("Wurmwall Sweeper and its Station mechanics must be registered");
    advance_to_main1_from_game_start(&mut engine);
    let station = move_ready_to_battlefield(&mut engine, 0, "wurmwall_sweeper");
    answer_trigger_order_in_engine_order(&mut engine);
    let first = engine.state.priority_player_id();
    let second = engine
        .state
        .players
        .iter()
        .map(|player| player.id)
        .find(|player| *player != first)
        .unwrap();
    engine.apply_command(first, &pass()).unwrap();
    let surveil = engine.apply_command(second, &pass()).unwrap();
    let candidates = find_resolution_choice(&surveil)
        .expect("Wurmwall entry surveil choice")
        .candidate_object_ids
        .clone();
    engine
        .apply_command(0, &submit_resolution_choice(candidates))
        .expect("put both surveilled cards into the graveyard");
    assert!(engine.state.pending_resolution.is_none());
    let crew = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&crew).unwrap().summoning_sick = true;
    (engine, station, crew)
}

#[test]
fn issue_147_cards_are_registered_with_exact_station_and_modal_shapes() {
    let registry = CardRegistry::global();
    let sweeper = registry
        .get("wurmwall_sweeper")
        .expect("Wurmwall Sweeper is registered")
        .primary_face();
    assert_eq!(sweeper.triggered_abilities.len(), 1);
    assert_eq!(sweeper.activated_abilities.len(), 1);
    assert_eq!(sweeper.static_abilities.len(), 1);
    assert!(sweeper.power.is_none() && sweeper.toughness.is_none());
    assert!(matches!(
        sweeper.static_abilities[0].definition,
        StaticAbilityDef::ConditionalSelfModifier {
            base_power: Some(2),
            base_toughness: Some(2),
            ..
        }
    ));

    let drill = registry
        .get("drill_too_deep")
        .expect("Drill Too Deep is registered")
        .primary_face();
    assert_eq!(drill.modal_spell.as_ref().unwrap().modes.len(), 2);
}

#[test]
fn station_publishes_generation_bound_costs_and_uses_power_on_resolution() {
    let (mut engine, station, crew) = station_engine(147_002);
    let before = engine.characteristics(station).unwrap();
    assert!(before.has_type("Artifact"));
    assert!(!before.has_type("Creature"));
    assert_eq!((before.power, before.toughness), (None, None));
    assert!(!before.has_keyword(Keyword::Flying));

    let legal = engine.initial_response_batch();
    let key = u64::from(station) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable, "{choices:?}");
    assert_eq!(choices.choices.len(), 1);
    assert_eq!(choices.choices[0].kind(), CostChoiceKind::Tap);
    assert_eq!(choices.choices[0].candidate_ids, [crew]);
    let published = choices.choices[0].candidate_objects[0]
        .object
        .as_ref()
        .expect("Station payment is generation-bound");
    assert_eq!(published.object_id, crew);
    assert_eq!(published.zone_change_generation, generation(&engine, crew));

    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect("a summoning-sick creature can pay the separate Station tap cost");
    assert!(engine.state.objects[&crew].tapped);
    assert!(!engine.state.objects[&station].tapped);

    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&crew).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        3,
        "Station reads the tapped creature's current 3 power on resolution"
    );

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .add_counters(CounterKind::Charge, 1, timestamp);
    let active = engine.characteristics(station).unwrap();
    assert!(active.has_type("Artifact") && active.has_type("Creature"));
    assert_eq!((active.power, active.toughness), (Some(2), Some(2)));
    assert!(active.has_keyword(Keyword::Flying));

    engine.state.objects.get_mut(&crew).unwrap().tapped = false;
    let above_threshold = engine.initial_response_batch();
    assert!(above_threshold.legal_by_player[&0]
        .cost_choices_by_ability
        .contains_key(&key));
    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 3);
    let inactive = engine.characteristics(station).unwrap();
    assert!(!inactive.has_type("Creature"));
    assert_eq!((inactive.power, inactive.toughness), (None, None));
    assert!(!inactive.has_keyword(Keyword::Flying));
}

#[test]
fn station_uses_generation_lki_and_clamps_negative_final_amounts() {
    let (mut engine, station, crew) = station_engine(147_003);
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&crew).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        2,
        timestamp,
    );
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .unwrap();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Grizzly Bears".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the tapped creature before Station resolves");
    assert_eq!(engine.state.objects[&crew].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        4,
        "the exact departed generation contributes its last known 4 power"
    );

    let (mut engine, station, crew) = station_engine(147_004);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(crew),
        kind: ContinuousEffectKind::PtModify {
            delta_power: -3,
            delta_toughness: 0,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    assert_eq!(engine.effective_power(crew), Some(0));
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        0,
        "a negative power result becomes zero only at the final Amount boundary"
    );
}

#[test]
fn copied_station_keeps_printed_abilities_but_not_charge_counters_or_threshold_state() {
    let (mut engine, station, _) = station_engine(147_006);
    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 4);
    assert!(engine
        .characteristics(station)
        .unwrap()
        .has_type("Creature"));

    inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(station)))
        .expect("copy the currently animated Spacecraft");
    engine.apply_command(0, &pass()).unwrap();
    let copied = engine.apply_command(1, &pass()).unwrap();
    let token = token_created_events(&copied)[0].object_id;

    assert!(engine.state.objects[&token].is_token());
    assert_eq!(
        engine.state.objects[&token].counter_count(CounterKind::Charge),
        0
    );
    let copied_characteristics = engine.characteristics(token).unwrap();
    assert!(copied_characteristics.has_type("Artifact"));
    assert!(!copied_characteristics.has_type("Creature"));
    assert_eq!(
        (
            copied_characteristics.power,
            copied_characteristics.toughness
        ),
        (None, None)
    );
    assert!(!copied_characteristics.has_keyword(Keyword::Flying));
    assert_eq!(
        engine.state.objects[&token]
            .token_origin
            .as_ref()
            .expect("copy snapshot")
            .face
            .activated_abilities
            .len(),
        1,
        "the copied permanent retains its printed Station ability"
    );
}

#[test]
fn drill_too_deep_executes_each_mode_with_its_own_target_filter() {
    let mut engine = GameEngine::new(147_005, &[0, 1], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let spacecraft = inject_permanent_on_battlefield(&mut engine, 0, "wurmwall_sweeper");
    inject_card_into_hand(&mut engine, 0, "drill_too_deep");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "drill_too_deep");
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, target_object(spacecraft))]),
        )
        .expect("choose the charge-counter mode targeting a controlled Spacecraft");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&spacecraft].counter_count(CounterKind::Charge),
        5
    );

    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "explosive_apparatus");
    inject_card_into_hand(&mut engine, 0, "drill_too_deep");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "drill_too_deep");
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(artifact))]),
        )
        .expect("choose the destroy mode targeting an opponent's artifact");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
}
