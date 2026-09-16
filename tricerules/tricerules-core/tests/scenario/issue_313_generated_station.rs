//! Issue #313 — Extinguisher Battleship and Fell Gravship.
//!
//! These scenarios exercise the shared Station seam and the two exact entry
//! abilities.  Extinguisher has one mandatory noncreature target: when that
//! target is illegal at resolution, the targeted ability fizzles and its
//! untargeted mass-damage instruction is not performed.  Fell mills first and
//! then publishes a mandatory choice over the controller's *current*
//! graveyard, including cards that were already there and cards that were
//! milled by this trigger.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-15.  The
//! Station activation follows CR 602.1-2 and 702.184a; continuous threshold
//! characteristics follow CR 611.3/613.1 and the card's ETB triggers follow
//! CR 603.1-3.  Target legality and fizzle use CR 115.1/608.2b, the mill and
//! graveyard choice use CR 701.13/701.17 and 608.2h, and physical-object
//! identity is checked with CR 400.7 zone-change generations.

use super::helpers::*;
use prost::Message;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    CanonicalGameplayCommand, ChoiceCandidateSourceZone, ChooseTriggerTarget, TargetRef,
};

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn trigger_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![trigger_target(object_id)],
            ..Default::default()
        })),
    }
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

fn activate_station(engine: &GameEngine, source: u32, creature: u32) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(source, 0, Vec::new(), vec![tap_selection(engine, creature)]);
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

fn canonical_command(inner: RuledCommand) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CanonicalGameplay(CanonicalGameplayCommand {
            command: inner.encode_to_vec(),
            auto_pass_policies: Vec::new(),
        })),
    }
}

fn put_on_top(engine: &mut GameEngine, player: usize, ids: &[&str]) -> Vec<u32> {
    let objects = ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
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
    let target = (card_id == "extinguisher_battleship")
        .then(|| inject_permanent_on_battlefield(&mut engine, 1, "short_sword"));
    let source = move_ready_to_battlefield(&mut engine, 0, card_id);
    if let Some(target) = target {
        engine
            .apply_command(0, &choose_trigger_target(target))
            .expect("Extinguisher noncreature target");
    }
    // Fell's default padded library contains only lands, so its mandatory
    // choice has no candidates and the entry trigger resolves without a
    // synthetic answer.  Extinguisher has already chosen its target.
    resolve_entire_stack_two_player(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    (engine, source, creature)
}

fn fell_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["fell_gravship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Fell engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_fell_to_choice(engine: &mut GameEngine) -> ResolutionChoiceRequired {
    engine.apply_command(0, &pass()).expect("controller pass");
    let batch = engine
        .apply_command(1, &pass())
        .expect("resolve Fell trigger");
    find_resolution_choice(&batch).expect("Fell graveyard choice")
}

#[test]
fn issue_313_station_thresholds_are_exact_and_remain_usable_above_them() {
    for (card_id, threshold, power, toughness, keywords, seed) in [
        (
            "extinguisher_battleship",
            5,
            10,
            10,
            vec![Keyword::Flying, Keyword::Trample],
            313_001,
        ),
        (
            "fell_gravship",
            8,
            3,
            2,
            vec![Keyword::Flying, Keyword::Lifelink],
            313_002,
        ),
    ] {
        let (mut engine, source, creature) = station_engine(seed, card_id);
        let initial = engine
            .characteristics(source)
            .expect("initial characteristics");
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
        assert_eq!(
            [Keyword::Flying, Keyword::Trample, Keyword::Lifelink]
                .into_iter()
                .filter(|keyword| at.has_keyword(*keyword))
                .collect::<Vec<_>>(),
            keywords
        );

        engine
            .apply_command(0, &activate_station(&engine, source, creature))
            .expect("Station remains activatable at threshold");
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            threshold + 2
        );

        engine.state.objects.get_mut(&creature).unwrap().tapped = false;
        engine.state.turn_step = TurnStep::Upkeep;
        let before = engine.state.objects[&source].counter_count(CounterKind::Charge);
        assert!(engine
            .apply_command(0, &activate_station(&engine, source, creature))
            .is_err());
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Charge),
            before
        );
        assert!(!engine.state.objects[&creature].tapped);
    }
}

#[test]
fn issue_313_station_revalidates_source_payment_generations_and_current_controller() {
    for (card_id, seed) in [
        ("extinguisher_battleship", 313_004),
        ("fell_gravship", 313_005),
    ] {
        let (mut source_stale, source, creature) = station_engine(seed, card_id);
        let command = activate_station(&source_stale, source, creature);
        let source_generation = generation(&source_stale, source);
        move_battlefield_to_hand(&mut source_stale, 0, source);
        assert!(generation(&source_stale, source) > source_generation);
        assert!(source_stale.apply_command(0, &command).is_err());
        assert!(!source_stale.state.objects[&creature].tapped);

        let (mut payment_stale, source, creature) = station_engine(seed + 10, card_id);
        let command = activate_station(&payment_stale, source, creature);
        let before = payment_stale.state.objects[&source].counter_count(CounterKind::Charge);
        move_battlefield_to_hand(&mut payment_stale, 0, creature);
        payment_stale.state.players[0]
            .hand
            .retain(|object_id| *object_id != creature);
        payment_stale.state.players[0].battlefield.push(creature);
        payment_stale.state.objects.get_mut(&creature).unwrap().zone = Zone::Battlefield;
        *payment_stale
            .state
            .zone_change_generation
            .entry(creature)
            .or_default() += 1;
        assert!(payment_stale.apply_command(0, &command).is_err());
        assert!(!payment_stale.state.objects[&creature].tapped);
        assert_eq!(
            payment_stale.state.objects[&source].counter_count(CounterKind::Charge),
            before
        );

        let (mut controlled, source, _) = station_engine(seed + 20, card_id);
        let controller_creature = inject_creature_on_battlefield(&mut controlled, 1, "storm_crow");
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
        controlled.state.active_player_idx = 1;
        controlled.state.priority_idx = 1;
        controlled
            .apply_command(
                1,
                &activate_station(&controlled, source, controller_creature),
            )
            .expect("current controller can activate Station");
        resolve_entire_stack_two_player(&mut controlled);
        assert_eq!(
            controlled.state.objects[&source].counter_count(CounterKind::Charge),
            2
        );
        assert!(controlled.state.objects[&controller_creature].tapped);
    }
}

#[test]
fn issue_313_extinguisher_destroys_only_the_legal_target_then_damages_all_creatures() {
    // Keep one high-toughness creature to observe damage, one lethal creature
    // to exercise state-based actions, and one creature with a regeneration
    // shield to pin the supported destruction interaction.
    let mut fresh = GameEngine::new(
        313_011,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["extinguisher_battleship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Extinguisher engine");
    advance_to_main1_from_game_start(&mut fresh);
    let artifact = inject_permanent_on_battlefield(&mut fresh, 0, "short_sword");
    let own = inject_creature_on_battlefield(&mut fresh, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut fresh, 1, "grizzly_bears");
    let lethal = inject_creature_on_battlefield(&mut fresh, 1, "grizzly_bears");
    let regenerating = inject_creature_on_battlefield(&mut fresh, 0, "grizzly_bears");
    fresh.state.objects.get_mut(&own).unwrap().toughness = Some(20);
    fresh.state.objects.get_mut(&opponent).unwrap().toughness = Some(20);
    fresh.state.objects.get_mut(&lethal).unwrap().toughness = Some(4);
    fresh
        .state
        .objects
        .get_mut(&regenerating)
        .unwrap()
        .toughness = Some(2);
    fresh
        .state
        .objects
        .get_mut(&regenerating)
        .unwrap()
        .regeneration_shields = 1;
    let source = move_ready_to_battlefield(&mut fresh, 0, "extinguisher_battleship");
    let key = u64::from(source) << 32;
    let legal_targets = &fresh.initial_response_batch().legal_by_player[&0]
        .valid_targets_by_ability[&key]
        .groups[0]
        .valid_permanent_ids;
    assert!(legal_targets.contains(&artifact));
    assert!(legal_targets.contains(&source));
    assert!(!legal_targets.contains(&own));
    assert!(!legal_targets.contains(&opponent));
    assert!(
        fresh
            .apply_command(1, &choose_trigger_target(artifact))
            .is_err(),
        "only the trigger controller may publish Extinguisher's target"
    );
    assert!(
        fresh.apply_command(0, &choose_trigger_target(own)).is_err(),
        "creatures are excluded from Extinguisher's target"
    );
    fresh
        .apply_command(0, &choose_trigger_target(artifact))
        .expect("noncreature target");
    resolve_entire_stack_two_player(&mut fresh);
    assert_eq!(fresh.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(fresh.state.objects[&own].damage, 4);
    assert_eq!(fresh.state.objects[&opponent].damage, 4);
    assert_eq!(fresh.state.objects[&own].zone, Zone::Battlefield);
    assert_eq!(fresh.state.objects[&opponent].zone, Zone::Battlefield);
    assert_eq!(fresh.state.objects[&lethal].zone, Zone::Graveyard);
    assert_eq!(fresh.state.objects[&regenerating].zone, Zone::Battlefield);
    assert_eq!(fresh.state.objects[&regenerating].damage, 0);
}

#[test]
fn issue_313_extinguisher_target_fizzle_suppresses_mass_damage_and_indestructible_survives() {
    let mut stale = GameEngine::new(
        313_012,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["extinguisher_battleship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Extinguisher stale-target engine");
    advance_to_main1_from_game_start(&mut stale);
    let target = inject_permanent_on_battlefield(&mut stale, 1, "short_sword");
    let creature = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    stale.state.objects.get_mut(&creature).unwrap().toughness = Some(20);
    let source = move_ready_to_battlefield(&mut stale, 0, "extinguisher_battleship");
    stale
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose target");
    let old_generation = generation(&stale, target);
    move_battlefield_to_hand(&mut stale, 1, target);
    assert!(generation(&stale, target) > old_generation);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&target].zone, Zone::Hand);
    assert_eq!(stale.state.objects[&creature].damage, 0);
    assert_eq!(stale.state.objects[&source].zone, Zone::Battlefield);

    let mut indestructible = GameEngine::new(
        313_013,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["extinguisher_battleship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Extinguisher indestructible engine");
    advance_to_main1_from_game_start(&mut indestructible);
    let citadel = inject_permanent_on_battlefield(&mut indestructible, 1, "darksteel_citadel");
    let creature = inject_creature_on_battlefield(&mut indestructible, 1, "grizzly_bears");
    indestructible
        .state
        .objects
        .get_mut(&creature)
        .unwrap()
        .toughness = Some(20);
    let _source = move_ready_to_battlefield(&mut indestructible, 0, "extinguisher_battleship");
    indestructible
        .apply_command(0, &choose_trigger_target(citadel))
        .expect("indestructible noncreature target");
    resolve_entire_stack_two_player(&mut indestructible);
    assert_eq!(
        indestructible.state.objects[&citadel].zone,
        Zone::Battlefield
    );
    assert_eq!(indestructible.state.objects[&creature].damage, 4);
}

#[test]
fn issue_313_extinguisher_source_changes_and_multiplayer_replay_remain_authoritative() {
    fn run(seed: u64) -> (Vec<Vec<u8>>, Zone, [u32; 3]) {
        let mut engine = GameEngine::new(
            seed,
            &[0, 1],
            20,
            Some(vec![
                deck_with("island", &["extinguisher_battleship"]),
                forest_only_deck(),
            ]),
            true,
        )
        .expect("Extinguisher multiplayer engine");
        advance_to_main1_from_game_start(&mut engine);
        append_third_player(&mut engine);
        let target = inject_permanent_on_battlefield(&mut engine, 2, "short_sword");
        let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let third = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
        for object_id in [first, second, third] {
            engine.state.objects.get_mut(&object_id).unwrap().toughness = Some(20);
        }
        let source = move_ready_to_battlefield(&mut engine, 0, "extinguisher_battleship");
        let mut responses = vec![engine
            .apply_command(0, &canonical_command(choose_trigger_target(target)))
            .expect("P0 chooses opponent's noncreature")
            .encode_to_vec()];
        // The source changes controller after target publication.  The trigger
        // still resolves for its original controller and its DamageAll remains
        // a player-set-wide, untargeted effect.
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
        while !engine.state.stack.is_empty() {
            answer_trigger_order_in_engine_order(&mut engine);
            let player_count = engine.state.players.len();
            for _ in 0..player_count {
                if engine.state.stack.is_empty() {
                    break;
                }
                let player = engine.state.priority_player_id();
                responses.push(
                    engine
                        .apply_command(player, &canonical_command(pass()))
                        .expect("player-set pass")
                        .encode_to_vec(),
                );
            }
        }
        (
            responses,
            engine.state.objects[&target].zone,
            [
                engine.state.objects[&first].damage,
                engine.state.objects[&second].damage,
                engine.state.objects[&third].damage,
            ],
        )
    }

    assert_eq!(run(313_014), run(313_014));
    let (_, target_zone, damage) = run(313_014);
    assert_eq!(target_zone, Zone::Graveyard);
    assert_eq!(damage, [4, 4, 4]);

    let mut leaves = GameEngine::new(
        313_015,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["extinguisher_battleship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Extinguisher source-leave engine");
    advance_to_main1_from_game_start(&mut leaves);
    let target = inject_permanent_on_battlefield(&mut leaves, 1, "short_sword");
    let creature = inject_creature_on_battlefield(&mut leaves, 1, "grizzly_bears");
    leaves.state.objects.get_mut(&creature).unwrap().toughness = Some(20);
    let source = move_ready_to_battlefield(&mut leaves, 0, "extinguisher_battleship");
    leaves
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose target before source leaves");
    move_battlefield_to_hand(&mut leaves, 0, source);
    resolve_entire_stack_two_player(&mut leaves);
    assert_eq!(leaves.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(leaves.state.objects[&creature].damage, 4);
    assert_eq!(leaves.state.objects[&source].zone, Zone::Hand);
}

#[test]
fn issue_313_fell_mills_three_then_chooses_preexisting_or_new_current_graveyard_card() {
    let mut engine = fell_engine(313_020);
    let preexisting = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let milled = put_on_top(
        &mut engine,
        0,
        &["island", "galvanizing_sawship", "grizzly_bears"],
    );
    let source = move_ready_to_battlefield(&mut engine, 0, "fell_gravship");
    let library_before = engine.state.players[0].library.len();
    let choice = resolve_fell_to_choice(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), library_before - 3);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::GraveyardCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.candidate_object_ids.len(), 3);
    assert!(choice.candidate_object_ids.contains(&preexisting));
    assert!(choice.candidate_object_ids.contains(&milled[1]));
    assert!(choice.candidate_object_ids.contains(&milled[2]));
    assert!(!choice.candidate_object_ids.contains(&milled[0]));
    assert_eq!(
        choice.candidate_source_zones,
        [ChoiceCandidateSourceZone::Graveyard as i32; 3]
    );
    assert!(choice
        .candidate_selectable
        .iter()
        .all(|selectable| *selectable));
    assert!(milled
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    // SubmitResolutionChoice carries physical object ids but no generation
    // field.  A stale-generation choice is therefore not representable in
    // this protocol command (N/A); zone movement is still generation-bound by
    // the engine, and the current choice must reject an ineligible id without
    // consuming the pending resolution.
    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![milled[0]]))
            .is_err(),
        "unrelated same-graveyard land is not selectable"
    );
    assert!(engine.state.pending_resolution.is_some());
    let old_generation = generation(&engine, preexisting);
    engine
        .apply_command(0, &submit_resolution_choice(vec![preexisting]))
        .expect("choose pre-existing current graveyard creature");
    assert_eq!(engine.state.objects[&preexisting].zone, Zone::Hand);
    assert!(generation(&engine, preexisting) > old_generation);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

    let mut newly_milled = fell_engine(313_021);
    let milled_spacecraft = put_on_top(&mut newly_milled, 0, &["galvanizing_sawship"])[0];
    // Only one injected card is needed: the remaining padded lands ensure the
    // mill instruction is short only in card type, not in library capacity.
    let _source = move_ready_to_battlefield(&mut newly_milled, 0, "fell_gravship");
    let choice = resolve_fell_to_choice(&mut newly_milled);
    assert_eq!(choice.candidate_object_ids, [milled_spacecraft]);
    newly_milled
        .apply_command(0, &submit_resolution_choice(vec![milled_spacecraft]))
        .expect("choose newly milled Spacecraft");
    assert_eq!(
        newly_milled.state.objects[&milled_spacecraft].zone,
        Zone::Hand
    );
}

#[test]
fn issue_313_fell_keeps_trigger_controller_choice_after_source_leaves() {
    let mut engine = fell_engine(313_023);
    let selected = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_graveyard = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    put_on_top(&mut engine, 0, &["island", "forest", "mountain"]);
    let source = move_ready_to_battlefield(&mut engine, 0, "fell_gravship");
    move_battlefield_to_hand(&mut engine, 0, source);
    let choice = resolve_fell_to_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [selected]);
    assert!(!choice.candidate_object_ids.contains(&opponent_graveyard));
    assert!(
        engine
            .apply_command(1, &submit_resolution_choice(vec![selected]))
            .is_err(),
        "only the trigger controller may answer Fell's choice"
    );
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(0, &submit_resolution_choice(vec![selected]))
        .expect("trigger controller answers after source leaves");
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&selected].zone, Zone::Hand);
}

#[test]
fn issue_313_fell_uses_trigger_controller_current_graveyard_and_player_set_replay() {
    fn run(seed: u64) -> (Vec<Vec<u8>>, u32) {
        let mut engine = fell_engine(seed);
        append_third_player(&mut engine);
        let selected = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        put_on_top(&mut engine, 0, &["island", "forest", "mountain"]);
        let source = move_ready_to_battlefield(&mut engine, 0, "fell_gravship");
        // The trigger belongs to P0 even if its source is stolen before
        // resolution; the current-choice candidates remain P0's graveyard.
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

        let mut responses = Vec::new();
        responses.push(
            engine
                .apply_command(0, &canonical_command(pass()))
                .expect("P0 pass")
                .encode_to_vec(),
        );
        responses.push(
            engine
                .apply_command(1, &canonical_command(pass()))
                .expect("P1 pass")
                .encode_to_vec(),
        );
        let batch = engine
            .apply_command(2, &canonical_command(pass()))
            .expect("P2 pass resolves Fell trigger")
            .encode_to_vec();
        let batch = tricerules_proto::ruled::v1::RuledEventBatch::decode(batch.as_slice())
            .expect("decode canonical response");
        let choice = find_resolution_choice(&batch).expect("current P0 graveyard choice");
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, [selected]);
        assert!(
            engine
                .apply_command(1, &submit_resolution_choice(vec![selected]))
                .is_err(),
            "stolen source controller cannot answer trigger-controller choice"
        );
        responses.push(
            engine
                .apply_command(
                    0,
                    &canonical_command(submit_resolution_choice(vec![selected])),
                )
                .expect("P0 answers its current graveyard choice")
                .encode_to_vec(),
        );
        (responses, engine.state.objects[&selected].zone as u32)
    }

    assert_eq!(run(313_022), run(313_022));
    assert_eq!(run(313_022).1, Zone::Hand as u32);
}

#[test]
fn issue_313_fell_has_no_impossible_choice_with_short_or_ineligible_libraries() {
    let mut short = fell_engine(313_030);
    // Leave only two lands to mill; no legal graveyard card exists, so the
    // mandatory instruction completes without publishing an impossible choice.
    short.state.players[0].library.truncate(2);
    let before = short.state.players[0].graveyard.len();
    let _source = move_ready_to_battlefield(&mut short, 0, "fell_gravship");
    resolve_entire_stack_two_player(&mut short);
    assert_eq!(short.state.players[0].graveyard.len(), before + 2);
    assert!(short.state.pending_resolution.is_none());

    let mut three_ineligible = fell_engine(313_031);
    let before = three_ineligible.state.players[0].graveyard.len();
    put_on_top(&mut three_ineligible, 0, &["island", "forest", "mountain"]);
    let _source = move_ready_to_battlefield(&mut three_ineligible, 0, "fell_gravship");
    resolve_entire_stack_two_player(&mut three_ineligible);
    assert_eq!(
        three_ineligible.state.players[0].graveyard.len(),
        before + 3
    );
    assert!(three_ineligible.state.pending_resolution.is_none());
    assert!(three_ineligible.state.stack.is_empty());
}
