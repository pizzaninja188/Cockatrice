//! Issue #311 — targeted-ETB Station Spacecraft.
//!
//! The two generated cards share the reviewed Station payment and conditional
//! characteristic assembly from issues #309/#310. Their entry triggers cover
//! an optional creature damage target and a resolution-time battlefield artifact
//! count. Rescue Skiff remains blocked by issue #312 because its Aura return
//! needs an engine-owned CR 303.4f attachment choice.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-15.  The
//! Station payment is a sorcery-speed activated ability (CR 602.1/602.2 and
//! 702.184a), the threshold characteristic striation is continuous (CR
//! 721.2a-b), and the entry abilities use trigger-controller identity and
//! resolution-time target revalidation (CR 603.2/603.3, 608.2b, 113.7a,
//! 400.7). Rescue Skiff is intentionally not generated or exercised here;
//! blocker #312 owns its Aura-return attachment-choice surface.

use super::helpers::*;
use prost::Message;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{CanonicalGameplayCommand, ChooseTriggerTarget, TargetRef};

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn trigger_target(object_id: u32, kind: TargetRefKind) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: kind as i32,
        ..Default::default()
    }
}

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    }
}

fn choose_zero_trigger_targets() -> RuledCommand {
    choose_trigger_targets(Vec::new())
}

fn tap_selection(engine: &GameEngine, object_id: u32) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(
            tricerules_proto::ruled::v1::cost_selection::Selection::BattlefieldObjects(
                tricerules_proto::ruled::v1::CostObjectRefs {
                    objects: vec![tricerules_proto::ruled::v1::CostObjectRef {
                        object_id,
                        zone_change_generation: generation(engine, object_id),
                    }],
                },
            ),
        ),
    }
}

fn activate_station(engine: &GameEngine, source: u32, crew: u32) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(source, 0, Vec::new(), vec![tap_selection(engine, crew)]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, source);
    command
}

fn move_battlefield_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn append_third_player(engine: &mut GameEngine) {
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.battlefield.clear();
    third.hand.clear();
    third.library.clear();
    third.graveyard.clear();
    third.exile.clear();
    engine.state.players.push(third);
}

fn station_engine(seed: u64, card_id: &str) -> (GameEngine, u32, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &[card_id]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .expect("generated Station card must be registered");
    advance_to_main1_from_game_start(&mut engine);

    // Put the entry trigger's legal target in place before the Spacecraft enters.
    let warmaker_target = (card_id == "warmaker_gunship")
        .then(|| inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"));
    let source = move_ready_to_battlefield(&mut engine, 0, card_id);
    match card_id {
        "pinnacle_kill-ship" => engine
            .apply_command(0, &choose_zero_trigger_targets())
            .expect("Pinnacle allows no target"),
        "warmaker_gunship" => engine
            .apply_command(
                0,
                &choose_trigger_targets(vec![trigger_target(
                    warmaker_target.expect("Warmaker target"),
                    TargetRefKind::Permanent,
                )]),
            )
            .expect("Warmaker opponent creature target"),
        _ => unreachable!("unknown #311 card {card_id}"),
    };
    resolve_entire_stack_two_player(&mut engine);
    let crew = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    (engine, source, crew)
}

fn canonical_command(inner: RuledCommand) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CanonicalGameplay(CanonicalGameplayCommand {
            command: inner.encode_to_vec(),
            auto_pass_policies: Vec::new(),
        })),
    }
}

fn resolve_stack_for_player_set(engine: &mut GameEngine, responses: &mut Vec<Vec<u8>>) {
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        let player = engine.state.priority_player_id();
        responses.push(
            engine
                .apply_command(player, &canonical_command(pass()))
                .expect("player-set priority pass")
                .encode_to_vec(),
        );
    }
}

#[test]
fn issue_311_station_thresholds_are_exact_and_remain_usable_above_them() {
    for (card_id, threshold, power, toughness, seed) in [
        ("pinnacle_kill-ship", 7, 7, 7, 311_001),
        ("warmaker_gunship", 6, 4, 3, 311_002),
    ] {
        let (mut engine, source, crew) = station_engine(seed, card_id);
        let initial = engine
            .characteristics(source)
            .expect("initial characteristics");
        assert!(initial.has_type("Artifact"));
        assert!(!initial.has_type("Creature"));
        assert_eq!((initial.power, initial.toughness), (None, None));
        assert!(!initial.has_keyword(Keyword::Flying));

        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .set_counter(CounterKind::Charge, threshold - 1);
        let below = engine.characteristics(source).expect("below threshold");
        assert!(!below.has_type("Creature"));
        assert_eq!((below.power, below.toughness), (None, None));
        assert!(!below.has_keyword(Keyword::Flying));

        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .set_counter(CounterKind::Charge, threshold);
        let at = engine.characteristics(source).expect("at threshold");
        assert!(at.has_type("Artifact") && at.has_type("Creature"));
        assert_eq!((at.power, at.toughness), (Some(power), Some(toughness)));
        assert!(at.has_keyword(Keyword::Flying));

        engine
            .apply_command(0, &activate_station(&engine, source, crew))
            .expect("Station remains activated at threshold");
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            threshold + 2,
            "Station records the payment creature's power"
        );

        engine.state.objects.get_mut(&crew).unwrap().tapped = false;
        engine.state.turn_step = TurnStep::Upkeep;
        let before = engine.state.objects[&source].counter_count(CounterKind::Charge);
        engine
            .apply_command(0, &activate_station(&engine, source, crew))
            .expect_err("Station remains sorcery-speed");
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            before
        );
        assert!(!engine.state.objects[&crew].tapped);
    }
}

#[test]
fn issue_311_station_revalidates_source_and_payment_generations() {
    let (mut source_stale, source, crew) = station_engine(311_004, "warmaker_gunship");
    source_stale
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Charge, 6);
    let source_generation = generation(&source_stale, source);
    let command = activate_station(&source_stale, source, crew);
    move_battlefield_to_hand(&mut source_stale, 0, source);
    assert!(generation(&source_stale, source) > source_generation);
    assert!(source_stale.apply_command(0, &command).is_err());
    assert!(!source_stale.state.objects[&crew].tapped);

    let (mut payment_stale, source, crew) = station_engine(311_005, "warmaker_gunship");
    payment_stale
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Charge, 6);
    let command = activate_station(&payment_stale, source, crew);
    move_battlefield_to_hand(&mut payment_stale, 0, crew);
    payment_stale.state.players[0]
        .hand
        .retain(|object_id| *object_id != crew);
    payment_stale.state.players[0].battlefield.push(crew);
    payment_stale.state.objects.get_mut(&crew).unwrap().zone = Zone::Battlefield;
    *payment_stale
        .state
        .zone_change_generation
        .entry(crew)
        .or_default() += 1;
    assert!(payment_stale.apply_command(0, &command).is_err());
    assert!(!payment_stale.state.objects[&crew].tapped);
    assert_eq!(
        payment_stale.state.objects[&source].counter_count(CounterKind::Charge),
        6
    );
}

#[test]
fn issue_311_station_activation_uses_current_controller_and_payment_power() {
    let (mut engine, source, _) = station_engine(311_006, "warmaker_gunship");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Charge, 6);
    let controller_crew = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    engine.state.players[1].battlefield.push(source);
    engine.state.objects.get_mut(&source).unwrap().controller = 1;
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 1;
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    let command = activate_station(&engine, source, controller_crew);
    engine
        .apply_command(1, &command)
        .expect("current controller can activate Station");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        8,
        "Station uses the tapped payment creature's current power"
    );
    assert!(engine.state.objects[&controller_crew].tapped);
}

#[test]
fn issue_311_pinnacle_allows_zero_or_one_creature_and_revalidates_target() {
    let mut zero = GameEngine::new(
        311_010,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["pinnacle_kill-ship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Pinnacle engine");
    advance_to_main1_from_game_start(&mut zero);
    let target = inject_creature_on_battlefield(&mut zero, 1, "grizzly_bears");
    let source = move_ready_to_battlefield(&mut zero, 0, "pinnacle_kill-ship");
    let key = u64::from(source) << 32;
    let group =
        &zero.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key].groups[0];
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.valid_permanent_ids, [target]);
    zero.apply_command(0, &choose_zero_trigger_targets())
        .expect("zero-target choice is legal");
    resolve_entire_stack_two_player(&mut zero);
    assert_eq!(zero.state.objects[&target].zone, Zone::Battlefield);

    let mut one = GameEngine::new(
        311_011,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["pinnacle_kill-ship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Pinnacle engine");
    advance_to_main1_from_game_start(&mut one);
    let own = inject_creature_on_battlefield(&mut one, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut one, 1, "grizzly_bears");
    let source = move_ready_to_battlefield(&mut one, 0, "pinnacle_kill-ship");
    let key = u64::from(source) << 32;
    assert_eq!(
        one.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
            .valid_permanent_ids,
        [own, opponent]
    );
    let noncreature = inject_permanent_on_battlefield(&mut one, 0, "short_sword");
    assert!(one
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(noncreature, TargetRefKind::Permanent)]),
        )
        .is_err());
    one.apply_command(
        0,
        &choose_trigger_targets(vec![trigger_target(opponent, TargetRefKind::Permanent)]),
    )
    .expect("opponent creature is legal");
    let source_generation = generation(&one, source);
    move_battlefield_to_hand(&mut one, 0, source);
    assert!(generation(&one, source) > source_generation);
    resolve_entire_stack_two_player(&mut one);
    assert_eq!(one.state.objects[&opponent].zone, Zone::Graveyard);
    assert_eq!(one.state.objects[&source].zone, Zone::Hand);

    let mut stale = GameEngine::new(
        311_012,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["pinnacle_kill-ship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Pinnacle engine");
    advance_to_main1_from_game_start(&mut stale);
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    let source = move_ready_to_battlefield(&mut stale, 0, "pinnacle_kill-ship");
    stale
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(target, TargetRefKind::Permanent)]),
        )
        .expect("choose target");
    move_battlefield_to_hand(&mut stale, 1, target);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&target].zone, Zone::Hand);
    assert_eq!(stale.state.objects[&source].zone, Zone::Battlefield);

    let mut controlled = GameEngine::new(
        311_024,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["warmaker_gunship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Warmaker engine");
    advance_to_main1_from_game_start(&mut controlled);
    let own_artifact = inject_permanent_on_battlefield(&mut controlled, 0, "short_sword");
    let target = inject_creature_on_battlefield(&mut controlled, 1, "grizzly_bears");
    controlled.state.objects.get_mut(&target).unwrap().toughness = Some(20);
    let source = move_ready_to_battlefield(&mut controlled, 0, "warmaker_gunship");
    controlled
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(target, TargetRefKind::Permanent)]),
        )
        .expect("choose target");
    // The source is still present but is no longer controlled by the trigger controller.  The
    // dynamic count must therefore include only the remaining artifact P0 controls.
    controlled.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    controlled.state.players[1].battlefield.push(source);
    controlled
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .controller = 1;
    controlled
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 1;
    resolve_entire_stack_two_player(&mut controlled);
    assert_eq!(controlled.state.objects[&target].damage, 1);
    assert!(controlled.state.players[0]
        .battlefield
        .contains(&own_artifact));
    assert!(controlled.state.players[1].battlefield.contains(&source));
}

#[test]
fn issue_311_pinnacle_is_player_set_generic_and_replays_deterministically() {
    fn run(seed: u64) -> (Vec<Vec<u8>>, Zone, u32) {
        let mut engine = GameEngine::new(
            seed,
            &[0, 1],
            20,
            Some(vec![
                deck_with("island", &["pinnacle_kill-ship"]),
                forest_only_deck(),
            ]),
            true,
        )
        .expect("Pinnacle engine");
        advance_to_main1_from_game_start(&mut engine);
        append_third_player(&mut engine);
        let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
        let source = move_ready_to_battlefield(&mut engine, 0, "pinnacle_kill-ship");
        let key = u64::from(source) << 32;
        assert_eq!(
            engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key]
                .groups[0]
                .valid_permanent_ids,
            [first, second]
        );
        let mut responses = vec![engine
            .apply_command(
                0,
                &canonical_command(choose_trigger_targets(vec![trigger_target(
                    second,
                    TargetRefKind::Permanent,
                )])),
            )
            .expect("choose a legal third-player creature")
            .encode_to_vec()];
        resolve_stack_for_player_set(&mut engine, &mut responses);
        (
            responses,
            engine.state.objects[&second].zone,
            engine.state.objects[&second].damage,
        )
    }
    assert_eq!(run(311_013), run(311_013));
    let (_, zone, damage) = run(311_013);
    assert_eq!(zone, Zone::Graveyard);
    assert_eq!(
        damage, 0,
        "lethal damage is cleaned up by state-based actions"
    );
}

#[test]
fn issue_311_warmaker_counts_controlled_artifacts_on_resolution_and_targets_only_opponents() {
    let mut increased = GameEngine::new(
        311_020,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["warmaker_gunship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Warmaker engine");
    advance_to_main1_from_game_start(&mut increased);
    let own_creature = inject_creature_on_battlefield(&mut increased, 0, "grizzly_bears");
    let initial_artifact = inject_permanent_on_battlefield(&mut increased, 0, "short_sword");
    let opponent_artifact = inject_permanent_on_battlefield(&mut increased, 1, "short_sword");
    let opponent = inject_creature_on_battlefield(&mut increased, 1, "grizzly_bears");
    increased
        .state
        .objects
        .get_mut(&opponent)
        .unwrap()
        .toughness = Some(20);
    let source = move_ready_to_battlefield(&mut increased, 0, "warmaker_gunship");
    let key = u64::from(source) << 32;
    let legal = &increased.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&key]
        .groups[0];
    assert_eq!(legal.valid_permanent_ids, [opponent]);
    assert!(increased
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(own_creature, TargetRefKind::Permanent)]),
        )
        .is_err());
    increased
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(opponent, TargetRefKind::Permanent)]),
        )
        .expect("opponent creature target");
    let added_artifact = inject_permanent_on_battlefield(&mut increased, 0, "bonesplitter");
    resolve_entire_stack_two_player(&mut increased);
    assert_eq!(
        increased.state.objects[&opponent].damage, 3,
        "source plus both controlled artifacts are counted at resolution"
    );
    assert!(increased.state.objects.contains_key(&initial_artifact));
    assert!(increased.state.objects.contains_key(&added_artifact));
    assert!(increased.state.objects.contains_key(&opponent_artifact));

    let mut decreased = GameEngine::new(
        311_021,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["warmaker_gunship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Warmaker engine");
    advance_to_main1_from_game_start(&mut decreased);
    let artifact = inject_permanent_on_battlefield(&mut decreased, 0, "short_sword");
    let opponent = inject_creature_on_battlefield(&mut decreased, 1, "grizzly_bears");
    decreased
        .state
        .objects
        .get_mut(&opponent)
        .unwrap()
        .toughness = Some(20);
    let source = move_ready_to_battlefield(&mut decreased, 0, "warmaker_gunship");
    decreased
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(opponent, TargetRefKind::Permanent)]),
        )
        .expect("choose target");
    // The source is an artifact, but it must not be counted after it leaves the
    // battlefield before this trigger resolves. The remaining controller-owned
    // artifact is still counted.
    move_battlefield_to_hand(&mut decreased, 0, source);
    resolve_entire_stack_two_player(&mut decreased);
    assert_eq!(
        decreased.state.objects[&opponent].damage, 1,
        "the remaining controller-owned artifact is counted after the source leaves"
    );
    assert_eq!(decreased.state.objects[&source].zone, Zone::Hand);
    assert_eq!(decreased.state.objects[&artifact].zone, Zone::Battlefield);

    let mut stale = GameEngine::new(
        311_022,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["warmaker_gunship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Warmaker engine");
    advance_to_main1_from_game_start(&mut stale);
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    stale.state.objects.get_mut(&target).unwrap().toughness = Some(20);
    let source = move_ready_to_battlefield(&mut stale, 0, "warmaker_gunship");
    stale
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(target, TargetRefKind::Permanent)]),
        )
        .expect("choose target");
    move_battlefield_to_hand(&mut stale, 1, target);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&target].zone, Zone::Hand);
    assert_eq!(stale.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn issue_311_warmaker_is_player_set_generic_and_replays_deterministically() {
    fn run(seed: u64) -> (Vec<Vec<u8>>, u32) {
        let mut engine = GameEngine::new(
            seed,
            &[0, 1],
            20,
            Some(vec![
                deck_with("island", &["warmaker_gunship"]),
                forest_only_deck(),
            ]),
            true,
        )
        .expect("Warmaker engine");
        advance_to_main1_from_game_start(&mut engine);
        append_third_player(&mut engine);
        let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
        for object_id in [first, second] {
            engine.state.objects.get_mut(&object_id).unwrap().toughness = Some(20);
        }
        let source = move_ready_to_battlefield(&mut engine, 0, "warmaker_gunship");
        let key = u64::from(source) << 32;
        assert_eq!(
            engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key]
                .groups[0]
                .valid_permanent_ids,
            [first, second]
        );
        let target = first;
        let choose = canonical_command(choose_trigger_targets(vec![trigger_target(
            target,
            TargetRefKind::Permanent,
        )]));
        let mut responses = Vec::new();
        responses.push(
            engine
                .apply_command(0, &choose)
                .expect("choose legal multiplayer target")
                .encode_to_vec(),
        );
        let priority = engine.state.priority_player_id();
        responses.push(
            engine
                .apply_command(priority, &canonical_command(pass()))
                .expect("first pass")
                .encode_to_vec(),
        );
        let priority = engine.state.priority_player_id();
        responses.push(
            engine
                .apply_command(priority, &canonical_command(pass()))
                .expect("second pass")
                .encode_to_vec(),
        );
        (responses, engine.state.objects[&target].damage)
    }
    assert_eq!(run(311_023), run(311_023));
}
