//! Actual-card coverage for Nesting Grounds' targeted counter-move ability.
//!
//! CR 115.3, 122.5, 602.5d, 608.2b, and 614.16 govern the target groups, atomic move,
//! activation timing, target identity, and counter-placement replacement.

use super::helpers::*;
use tricerules_cards::{
    primitives::{AbilityCost, ActivationTiming, SpellEffectKind, TargetChooser},
    CardRegistry, CounterKind,
};
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision,
    ResolutionChoiceRequired, RuledCommand, SubmitResolutionChoice, TargetRef, TargetRefKind,
};

const NESTING_GROUNDS: &str = "nesting_grounds";

fn four_player_engine(seed: u64) -> GameEngine {
    let mut deck = std::iter::repeat_n("forest".to_string(), 30).collect::<Vec<_>>();
    deck.extend(["grizzly_bears".to_string(), "sol_ring".to_string()]);
    let mut engine = GameEngine::new(seed, &[0, 1, 2, 3], 20, Some(vec![deck; 4]), true)
        .expect("new four-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn target_in_group(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        ..Default::default()
    }
}

fn move_counter_command(
    engine: &GameEngine,
    grounds: u32,
    source: u32,
    destination: u32,
) -> RuledCommand {
    activate_ability_for(
        engine,
        grounds,
        1,
        vec![target_in_group(source, 0), target_in_group(destination, 1)],
    )
}

fn pass_until_choice(engine: &mut GameEngine) -> ResolutionChoiceRequired {
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("the current priority holder passes");
        if engine.state.pending_resolution.is_some() {
            return find_resolution_choice(&batch)
                .expect("counter-kind choice is published when the ability resolves");
        }
    }
    panic!("ability should pause for a counter-kind choice");
}

#[test]
fn nesting_grounds_registers_its_complete_two_ability_land_definition() {
    let definition = CardRegistry::global()
        .get(NESTING_GROUNDS)
        .expect("the exact Nesting Grounds definition is registered");
    assert_eq!(definition.name, "Nesting Grounds");
    assert_eq!(definition.faces.len(), 1);
    let face = &definition.faces[0];
    assert_eq!(face.name, "Nesting Grounds");
    assert_eq!(face.types, vec!["Land"]);
    assert_eq!(face.activated_abilities.len(), 2);

    let mana_ability = &face.activated_abilities[0];
    assert!(matches!(mana_ability.costs.as_slice(), [AbilityCost::Tap]));
    assert!(matches!(
        mana_ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1
    ));

    let counter_move = &face.activated_abilities[1];
    assert!(matches!(
        counter_move.costs.as_slice(),
        [AbilityCost::Mana(_), AbilityCost::Tap]
    ));
    assert!(matches!(
        counter_move.effect.as_slice(),
        [SpellEffectKind::MoveOneCounterBetweenTargets]
    ));
    assert_eq!(counter_move.timing, ActivationTiming::SorcerySpeed);
    let groups = &counter_move
        .targeting
        .as_ref()
        .expect("two target groups")
        .groups;
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|group| {
        group.min == 1 && group.max == 1 && group.chooser == TargetChooser::Controller
    }));
    assert_eq!(groups[0].distinct_from, vec![1]);
    assert_eq!(groups[1].distinct_from, vec![0]);
}

fn resolve_without_counter_choice(engine: &mut GameEngine) {
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("the current priority holder passes");
        assert!(
            find_resolution_choice(&batch).is_none(),
            "this counter move has no legal choice to offer"
        );
        if engine.state.stack.is_empty() && engine.state.pending_resolution.is_none() {
            return;
        }
    }
    panic!("the no-choice resolution should finish after one four-player priority round");
}

fn move_owned_card(engine: &mut GameEngine, owner: i32, card_id: &str, destination: DevZone) {
    let definition = tricerules_cards::CardRegistry::global()
        .get(card_id)
        .expect("registered card");
    engine.enable_dev_commands();
    engine
        .apply_command(
            owner,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: definition.name.clone(),
                        zone: destination as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the owner's exact card through the engine");
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

#[test]
fn nesting_grounds_moves_one_selected_counter_between_distinct_player_targets() {
    let mut engine = four_player_engine(202_610_080);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 2);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Stun, 1);
    let source_counter_timestamps = engine.state.objects[&source].counter_timestamps.clone();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let mana_before = engine.state.players[0].mana_pool.colorless;
    engine
        .apply_command(
            0,
            &move_counter_command(&engine, grounds, source, destination),
        )
        .expect("activate Nesting Grounds at sorcery speed");
    assert!(
        engine.state.objects[&grounds].tapped,
        "the ability pays its tap cost"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before - 1);
    let choice = pass_until_choice(&mut engine);

    assert_eq!(
        choice.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ResolutionBranch as i32
    );
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.resolution_branches.len(), 2);
    assert_eq!(choice.resolution_branches[0].label, "Move one stun counter");
    assert_eq!(
        choice.resolution_branches[1].label,
        "Move one quest counter"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        2
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Stun),
        1
    );
    assert_eq!(
        engine.state.objects[&source].counter_timestamps, source_counter_timestamps,
        "resolution preflight restores the simulated counter timestamp"
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );

    engine
        .apply_command(0, &select_branch(1))
        .expect("select Quest counters");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Stun),
        1
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(engine.state.objects[&grounds].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn nesting_grounds_keeps_its_mana_ability_and_sorcery_speed_costs() {
    let mut engine = four_player_engine(202_610_081);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let activation = move_counter_command(&engine, grounds, source, destination);

    assert!(
        engine.apply_command(0, &activation).is_err(),
        "the {{1}} cost cannot be skipped"
    );
    assert!(!engine.state.objects[&grounds].tapped);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine.state.turn_step = TurnStep::Upkeep;
    assert!(
        engine.apply_command(0, &activation).is_err(),
        "Activate only as a sorcery is unavailable outside a main phase"
    );
    assert!(!engine.state.objects[&grounds].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.stack.is_empty());

    engine.state.turn_step = TurnStep::Main1;
    engine
        .apply_command(0, &activation)
        .expect("the card's second ability is usable in its controller's main phase");
    assert!(engine.state.objects[&grounds].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    resolve_without_counter_choice(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        0
    );

    engine.state.objects.get_mut(&grounds).unwrap().tapped = false;
    engine
        .apply_command(0, &activate_ability_for(&engine, grounds, 0, vec![]))
        .expect("the first ability is a usable mana ability");
    assert!(engine.state.objects[&grounds].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );
}

#[test]
fn nesting_grounds_rejects_same_or_uncontrolled_source_targets_before_payment() {
    let mut engine = four_player_engine(202_610_082);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let controlled = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let other_controlled = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_controlled = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&controlled)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    for targets in [
        vec![
            target_in_group(controlled, 0),
            target_in_group(controlled, 1),
        ],
        vec![
            target_in_group(opponent_controlled, 0),
            target_in_group(other_controlled, 1),
        ],
        vec![
            TargetRef {
                object_id: engine.state.players[0].id as u32,
                group_index: 0,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            },
            target_in_group(other_controlled, 1),
        ],
        vec![
            target_in_group(controlled, 0),
            TargetRef {
                object_id: engine.state.players[0].id as u32,
                group_index: 1,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            },
        ],
    ] {
        let command = activate_ability_for(&engine, grounds, 1, targets);
        assert!(engine.apply_command(0, &command).is_err());
        assert!(!engine.state.objects[&grounds].tapped);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        assert_eq!(
            engine.state.objects[&controlled].counter_count(CounterKind::Quest),
            1
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn nesting_grounds_does_not_offer_a_choice_without_a_movable_counter() {
    let mut engine = four_player_engine(202_610_083);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &move_counter_command(&engine, grounds, source, destination),
        )
        .expect("activate the ability with an empty source");
    resolve_without_counter_choice(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        0
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );

    let mut blocked = four_player_engine(202_610_084);
    let grounds = inject_permanent_on_battlefield(&mut blocked, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut blocked, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut blocked, 2, "tatterkite");
    blocked
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut blocked,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    blocked
        .apply_command(
            0,
            &move_counter_command(&blocked, grounds, source, destination),
        )
        .expect("the counter-prohibiting permanent remains a legal target");
    resolve_without_counter_choice(&mut blocked);
    assert_eq!(
        blocked.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        blocked.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );
    assert!(blocked.state.pending_triggers.is_empty());
}

#[test]
fn nesting_grounds_source_can_leave_without_affecting_its_activated_ability() {
    let mut engine = four_player_engine(202_610_085);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &move_counter_command(&engine, grounds, source, destination),
        )
        .expect("activate Nesting Grounds");

    move_owned_card(&mut engine, 0, NESTING_GROUNDS, DevZone::Graveyard);
    assert_eq!(engine.state.objects[&grounds].zone, Zone::Graveyard);
    let choice = pass_until_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 0);
    engine
        .apply_command(0, &select_branch(0))
        .expect("finish the ability after its source left");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        0
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        1
    );
}

#[test]
fn nesting_grounds_invalidates_either_target_after_a_zone_change_and_return() {
    let mut source_target_engine = four_player_engine(202_610_086);
    let grounds = inject_permanent_on_battlefield(&mut source_target_engine, 0, NESTING_GROUNDS);
    let source = move_ready_to_battlefield(&mut source_target_engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut source_target_engine, 2, "grizzly_bears");
    source_target_engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut source_target_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let generation = source_target_engine.state.zone_change_generation[&source];
    source_target_engine
        .apply_command(
            0,
            &move_counter_command(&source_target_engine, grounds, source, destination),
        )
        .expect("activate with the original source incarnation");
    move_owned_card(
        &mut source_target_engine,
        0,
        "grizzly_bears",
        DevZone::Graveyard,
    );
    assert_eq!(
        source_target_engine.state.objects[&source].zone,
        Zone::Graveyard
    );
    assert_eq!(
        source_target_engine.state.objects[&source].counter_count(CounterKind::Quest),
        0
    );
    assert_eq!(
        move_ready_to_battlefield(&mut source_target_engine, 0, "grizzly_bears"),
        source
    );
    assert!(source_target_engine.state.zone_change_generation[&source] > generation);
    resolve_without_counter_choice(&mut source_target_engine);
    assert_eq!(
        source_target_engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );

    let mut destination_target_engine = four_player_engine(202_610_087);
    let grounds =
        inject_permanent_on_battlefield(&mut destination_target_engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut destination_target_engine, 0, "grizzly_bears");
    let destination = move_ready_to_battlefield(&mut destination_target_engine, 0, "sol_ring");
    destination_target_engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut destination_target_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let generation = destination_target_engine.state.zone_change_generation[&destination];
    destination_target_engine
        .apply_command(
            0,
            &move_counter_command(&destination_target_engine, grounds, source, destination),
        )
        .expect("activate with the original destination incarnation");
    move_owned_card(
        &mut destination_target_engine,
        0,
        "sol_ring",
        DevZone::Graveyard,
    );
    assert_eq!(
        destination_target_engine.state.objects[&destination].zone,
        Zone::Graveyard
    );
    assert_eq!(
        move_ready_to_battlefield(&mut destination_target_engine, 0, "sol_ring"),
        destination
    );
    assert!(destination_target_engine.state.zone_change_generation[&destination] > generation);
    resolve_without_counter_choice(&mut destination_target_engine);
    assert_eq!(
        destination_target_engine.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        destination_target_engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );
}

#[test]
fn nesting_grounds_revalidates_the_selected_counter_before_completing_a_parked_choice() {
    let mut engine = four_player_engine(202_610_088);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &move_counter_command(&engine, grounds, source, destination),
        )
        .expect("activate with an available counter");
    let choice = pass_until_choice(&mut engine);
    assert_eq!(choice.resolution_branches.len(), 1);

    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 0);
    engine
        .apply_command(0, &select_branch(0))
        .expect("a now-unavailable valid option completes as a no-op");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        0
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );

    let mut stale_target_engine = four_player_engine(202_610_094);
    let grounds = inject_permanent_on_battlefield(&mut stale_target_engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut stale_target_engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut stale_target_engine, 2, "grizzly_bears");
    stale_target_engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut stale_target_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    stale_target_engine
        .apply_command(
            0,
            &move_counter_command(&stale_target_engine, grounds, source, destination),
        )
        .expect("activate with current target generations");
    let choice = pass_until_choice(&mut stale_target_engine);
    assert_eq!(choice.resolution_branches.len(), 1);
    // A parked resolution rejects every other command. Advance the fixture generation directly
    // to model a zone change and return before its already-offered answer is processed.
    let generation = stale_target_engine
        .state
        .zone_change_generation
        .get(&destination)
        .copied()
        .unwrap_or(0);
    stale_target_engine
        .state
        .zone_change_generation
        .insert(destination, generation + 1);
    stale_target_engine
        .apply_command(0, &select_branch(0))
        .expect("a formerly valid target completes as a no-op");
    assert!(stale_target_engine.state.pending_resolution.is_none());
    assert!(stale_target_engine.state.stack.is_empty());
    assert_eq!(
        stale_target_engine.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        stale_target_engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );
}

#[test]
fn nesting_grounds_preserves_doubling_saga_and_siege_counter_paths() {
    let mut doubling = four_player_engine(202_610_089);
    let grounds = inject_permanent_on_battlefield(&mut doubling, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut doubling, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut doubling, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut doubling, 0, "doubling_season");
    doubling
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut doubling,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    doubling
        .apply_command(
            0,
            &move_counter_command(&doubling, grounds, source, destination),
        )
        .expect("activate with a legal destination under a counter doubler");
    let choice = pass_until_choice(&mut doubling);
    assert_eq!(
        choice.resolution_branches[0].label,
        "Move one quest counter"
    );
    doubling
        .apply_command(0, &select_branch(0))
        .expect("move a quest counter");
    assert_eq!(
        doubling.state.objects[&source].counter_count(CounterKind::Quest),
        0
    );
    assert_eq!(
        doubling.state.objects[&destination].counter_count(CounterKind::Quest),
        2
    );

    let mut saga_engine = four_player_engine(202_610_090);
    let grounds = inject_permanent_on_battlefield(&mut saga_engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut saga_engine, 0, "grizzly_bears");
    let saga = inject_permanent_on_battlefield(&mut saga_engine, 2, "burn,_burn,_tree_and_fern");
    saga_engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Lore, 1);
    give_mana(
        &mut saga_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    saga_engine
        .apply_command(
            0,
            &move_counter_command(&saga_engine, grounds, source, saga),
        )
        .expect("activate with a Saga as the destination");
    let choice = pass_until_choice(&mut saga_engine);
    assert_eq!(choice.resolution_branches[0].label, "Move one lore counter");
    saga_engine
        .apply_command(0, &select_branch(0))
        .expect("move a lore counter");
    assert_eq!(
        saga_engine.state.objects[&saga].counter_count(CounterKind::Lore),
        1
    );
    assert_eq!(
        saga_engine.state.pending_triggers.len(),
        1,
        "the lore chapter trigger is collected"
    );

    let mut siege_engine = four_player_engine(202_610_091);
    let grounds = inject_permanent_on_battlefield(&mut siege_engine, 0, NESTING_GROUNDS);
    let siege = inject_permanent_on_battlefield(
        &mut siege_engine,
        0,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
    );
    let destination = inject_creature_on_battlefield(&mut siege_engine, 2, "grizzly_bears");
    let siege_characteristics = siege_engine
        .characteristics(siege)
        .expect("Siege characteristics");
    assert!(siege_characteristics.has_type("Battle"));
    assert!(siege_characteristics.has_type("Siege"));
    siege_engine
        .state
        .objects
        .get_mut(&siege)
        .unwrap()
        .set_counter(CounterKind::Defense, 1);
    give_mana(
        &mut siege_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    siege_engine
        .apply_command(
            0,
            &move_counter_command(&siege_engine, grounds, siege, destination),
        )
        .expect("activate with the final defense counter as the source");
    let choice = pass_until_choice(&mut siege_engine);
    assert_eq!(
        choice.resolution_branches[0].label,
        "Move one defense counter"
    );
    siege_engine
        .apply_command(0, &select_branch(0))
        .expect("move the final defense counter");
    assert_eq!(
        siege_engine.state.objects[&siege].counter_count(CounterKind::Defense),
        0
    );
    assert_eq!(
        siege_engine.state.objects[&destination].counter_count(CounterKind::Defense),
        1
    );
    let siege_trigger = siege_engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(siege))
        .expect("the existing Siege defeat trigger is placed on the stack");
    assert!(siege_trigger
        .triggered_ability
        .as_ref()
        .is_some_and(|ability| {
            ability.effect.iter().any(|effect| {
                matches!(
                    effect,
                    tricerules_cards::primitives::SpellEffectKind::SiegeDefeat
                )
            })
        }));

    let mut blocked_siege_engine = four_player_engine(202_610_093);
    let grounds = inject_permanent_on_battlefield(&mut blocked_siege_engine, 0, NESTING_GROUNDS);
    let siege = inject_permanent_on_battlefield(
        &mut blocked_siege_engine,
        0,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
    );
    let blocked_destination =
        inject_creature_on_battlefield(&mut blocked_siege_engine, 2, "tatterkite");
    blocked_siege_engine
        .state
        .objects
        .get_mut(&siege)
        .unwrap()
        .set_counter(CounterKind::Defense, 1);
    give_mana(
        &mut blocked_siege_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    blocked_siege_engine
        .apply_command(
            0,
            &move_counter_command(&blocked_siege_engine, grounds, siege, blocked_destination),
        )
        .expect("activate at the final defense counter");
    resolve_without_counter_choice(&mut blocked_siege_engine);
    assert_eq!(
        blocked_siege_engine.state.objects[&siege].counter_count(CounterKind::Defense),
        1
    );
    assert_eq!(
        blocked_siege_engine.state.objects[&blocked_destination]
            .counter_count(CounterKind::Defense),
        0
    );
    assert!(blocked_siege_engine.state.stack.is_empty());
    assert!(blocked_siege_engine.state.pending_triggers.is_empty());
}

#[test]
fn nesting_grounds_rejects_wrong_decider_malformed_and_unoffered_choices_without_consuming_them() {
    let mut engine = four_player_engine(202_610_092);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 2);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Stun, 1);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &move_counter_command(&engine, grounds, source, destination),
        )
        .expect("activate Nesting Grounds");
    let choice = pass_until_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 0);
    let pending_before = serde_json::to_value(&engine.state).expect("serializable pending state");

    assert!(engine.apply_command(1, &select_branch(0)).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), pending_before);

    let malformed = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 1,
            chosen_object_ids: vec![source],
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(0, &malformed).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), pending_before);

    assert!(engine.apply_command(0, &select_branch(2)).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), pending_before);
    engine
        .apply_command(0, &select_branch(1))
        .expect("the valid offered index selects Quest");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Stun),
        1
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        1
    );
    let before_no_pending = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_command(0, &select_branch(0)).is_err());
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        before_no_pending
    );
}

#[test]
fn nesting_grounds_revalidates_first_target_control_during_parked_choice() {
    let mut engine = four_player_engine(202_610_094);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Quest, 1);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &move_counter_command(&engine, grounds, source, destination),
        )
        .expect("activate Nesting Grounds");
    let choice = pass_until_choice(&mut engine);
    assert_eq!(choice.deciding_player_id, 0);

    // A parked choice has no priority window. Change this fixture's derived controller directly
    // to prove the continuation rechecks the target role before committing the move.
    let source_object = engine.state.objects.get_mut(&source).unwrap();
    source_object.base_controller = 1;
    source_object.controller = 1;
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    engine.state.players[1].battlefield.push(source);
    assert_eq!(engine.characteristics(source).unwrap().controller, 1);

    engine
        .apply_command(0, &select_branch(0))
        .expect("a valid offered answer completes as a no-op after control changes");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        engine.state.objects[&destination].counter_count(CounterKind::Quest),
        0
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn nesting_grounds_ability_and_branch_choice_replay_deterministically() {
    use prost::Message;

    fn prepared() -> (GameEngine, u32, u32, u32) {
        let mut engine = four_player_engine(202_610_095);
        let grounds = inject_permanent_on_battlefield(&mut engine, 0, NESTING_GROUNDS);
        let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let destination = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .set_counter(CounterKind::Quest, 2);
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .set_counter(CounterKind::Stun, 1);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        (engine, grounds, source, destination)
    }

    let (mut original, grounds, source, destination) = prepared();
    let mut accepted_commands = Vec::new();
    let activation = move_counter_command(&original, grounds, source, destination);
    let batch = original
        .apply_command(0, &activation)
        .expect("activate Nesting Grounds");
    accepted_commands.push((0, activation.encode_to_vec(), batch.encode_to_vec()));

    let mut offered_choice = None;
    for _ in 0..4 {
        let actor = original.state.priority_player_id();
        let command = pass();
        let batch = original
            .apply_command(actor, &command)
            .expect("priority pass is accepted");
        accepted_commands.push((actor, command.encode_to_vec(), batch.encode_to_vec()));
        if let Some(choice) = find_resolution_choice(&batch) {
            offered_choice = Some(choice);
            break;
        }
    }
    let choice = offered_choice.expect("the counter-kind choice is part of the command sequence");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.resolution_branches.len(), 2);

    let answer = select_branch(1);
    let batch = original
        .apply_command(0, &answer)
        .expect("select the offered Quest counter branch");
    accepted_commands.push((0, answer.encode_to_vec(), batch.encode_to_vec()));
    assert_eq!(
        original.state.objects[&source].counter_count(CounterKind::Quest),
        1
    );
    assert_eq!(
        original.state.objects[&destination].counter_count(CounterKind::Quest),
        1
    );

    // Both runs start from the same explicit fixture; only accepted protobuf commands are
    // replayed below. The fixture setup is not claimed to be a native session journal.
    let (mut replay, replay_grounds, replay_source, replay_destination) = prepared();
    assert_eq!(
        (grounds, source, destination),
        (replay_grounds, replay_source, replay_destination)
    );
    for (actor, encoded_command, expected_batch) in accepted_commands {
        let command = RuledCommand::decode(encoded_command.as_slice()).expect("decode command");
        let batch = replay
            .apply_command(actor, &command)
            .expect("accepted command replays");
        assert_eq!(batch.encode_to_vec(), expected_batch);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        original.diagnostic_snapshot().unwrap()
    );
    assert!(replay.state.pending_resolution.is_none());
    assert!(replay.state.stack.is_empty());
}
