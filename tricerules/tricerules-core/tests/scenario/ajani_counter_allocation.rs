//! Prepared counter-allocation capability fixtures; no actual Ajani admission.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_proto::ruled::v1 as rv1;

const SOURCE: &str = "ajani_counter_allocation_fixture";

fn game() -> (GameEngine, u32, Vec<u32>) {
    game_with_counter_prevention(false)
}

fn game_with_counter_prevention(prohibit: bool) -> (GameEngine, u32, Vec<u32>) {
    game_with_options(prohibit, false)
}

fn game_with_options(prohibit: bool, separate_role: bool) -> (GameEngine, u32, Vec<u32>) {
    let fixture = r#"(
        id: "ajani_counter_allocation_fixture", name: "Ajani Counter Allocation Fixture",
        face_id: "ajani_counter_allocation_fixture", mana_cost: "{0}",
        types: ["Planeswalker", "Ajani"], loyalty: 4,
        activated_abilities: [(
            ability_id: "distribute_01", presentation: Fallback, costs: [Loyalty(-3)],
            effect: [
                DistributeCounters(counter: PlusOnePlusOne, count: 3, subject: Chosen((kind: Creature))),
                GrantKeywords(subject: Chosen((kind: Creature)), keywords: [Vigilance]),
                GainLife(amount: 7, who: EachPlayer),
            ],
            targeting: Some((groups: [(
                min: 0, max: 3, prompt: "Choose up to three creatures", effect_indices: [0, 1],
            )])),
        )],
    )"#;
    let fixture = if separate_role {
        fixture.replace("DistributeCounters(", "GrantKeywords(subject: Chosen((kind: Creature)), keywords: [Trample]), DistributeCounters(")
            .replace("min: 0, max: 3, prompt: \"Choose up to three creatures\", effect_indices: [0, 1],",
                "min: 1, max: 1, prompt: \"Choose unrelated creature\", effect_indices: [0]), (min: 0, max: 3, prompt: \"Choose allocation\", effect_indices: [1, 2],")
    } else {
        fixture.to_string()
    };
    let bears = include_str!("../../../tricerules-cards/data/grizzly_bears.ron");
    let bears = if prohibit {
        bears.replace("power:", "static_abilities: [(ability_id: \"prohibit\", presentation: Fallback, definition: ProhibitCounters(affected: Self_))], power:")
    } else {
        bears.to_string()
    };
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                &fixture,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                &bears,
            ],
            &[],
        )
        .expect("synthetic counter-allocation profile"),
    ));
    let mut engine = GameEngine::new(
        registry,
        731_300,
        &[17, 29, 41],
        20,
        Some(vec![vec!["forest".into(); 20]; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, SOURCE);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 4);
    let creatures = (0..3)
        .map(|seat| inject_permanent_on_battlefield(&mut engine, seat, "grizzly_bears"))
        .collect();
    (engine, source, creatures)
}

#[test]
fn allocation_and_vigilance_use_their_authored_group_after_an_unrelated_role() {
    let (mut engine, source, creatures) = game_with_options(false, true);
    let mut allocated = target(&engine, creatures[1], 3);
    allocated.group_index = 1;
    let unrelated = target(&engine, creatures[0], 0);
    let mut invalid = unrelated;
    invalid.counter_amount = 1;
    let before = ron::to_string(&engine.state).unwrap();
    assert!(apply_ability(&mut engine, 17, source, 0, vec![invalid, allocated]).is_err());
    assert_eq!(ron::to_string(&engine.state).unwrap(), before);
    let offer = engine.initial_response_batch();
    assert_eq!(
        offer.legal_by_player[&17].valid_targets_by_ability[&(u64::from(source) << 32)]
            .counter_allocation
            .as_ref()
            .unwrap()
            .group_index,
        1
    );
    apply_ability(&mut engine, 17, source, 0, vec![unrelated, allocated]).unwrap();
    resolve_top(&mut engine);
    assert_eq!(
        engine.state.objects[&creatures[0]].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(engine
        .characteristics(creatures[0])
        .unwrap()
        .has_keyword(Keyword::Trample));
    assert!(!engine
        .characteristics(creatures[0])
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(
        engine.state.objects[&creatures[1]].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    assert!(!engine
        .characteristics(creatures[1])
        .unwrap()
        .has_keyword(Keyword::Trample));
    assert!(engine
        .characteristics(creatures[1])
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    end_active_turn(&mut engine, 17);
    assert!(!engine
        .characteristics(creatures[1])
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(
        engine.state.objects[&creatures[1]].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
}

#[test]
fn prepared_allocation_commands_replay_identical_public_and_private_results() {
    use prost::Message;
    let (mut engine, source, creatures) = game();
    let (mut replay, replay_source, replay_creatures) = game();
    assert_eq!((source, &creatures), (replay_source, &replay_creatures));
    let activate = activate_ability_for(
        &engine,
        source,
        0,
        vec![
            target(&engine, creatures[0], 2),
            target(&engine, creatures[1], 1),
        ],
    );
    for (actor, command) in [(17, activate), (17, pass()), (29, pass()), (41, pass())] {
        let original = engine.apply_scenario_command(actor, &command).unwrap();
        let repeated = replay.apply_scenario_command(actor, &command).unwrap();
        assert_eq!(original.encode_to_vec(), repeated.encode_to_vec());
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            replay.diagnostic_snapshot().unwrap()
        );
    }
    assert_eq!(
        engine.state.objects[&creatures[0]].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        engine.state.objects[&creatures[1]].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine
        .characteristics(creatures[0])
        .unwrap()
        .has_keyword(Keyword::Vigilance));
}

#[test]
fn invalid_allocation_receipts_reject_atomically_before_costs() {
    for mutation in 0..10 {
        let (mut engine, source, creatures) = game();
        let mut targets = vec![
            target(&engine, creatures[0], 2),
            target(&engine, creatures[1], 1),
        ];
        match mutation {
            0 => targets[0].expected_zone_change_generation = None,
            1 => targets[0].expected_zone_change_generation = Some(1),
            2 => targets[0].kind = rv1::TargetRefKind::Player as i32,
            3 => targets[0].group_index = 1,
            4 => targets[0].damage_amount = 1,
            5 => targets[1] = target(&engine, creatures[0], 1),
            6 => targets[0].counter_amount = u32::MAX,
            7 => targets.push(target(&engine, creatures[2], 1)),
            8 => targets[0].object_id = source,
            9 => targets[0].object_id = u32::MAX,
            _ => unreachable!(),
        }
        let before = ron::to_string(&engine.state).unwrap();
        assert!(
            apply_ability(&mut engine, 17, source, 0, targets).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(
            ron::to_string(&engine.state).unwrap(),
            before,
            "mutation {mutation}"
        );
    }
}

#[test]
fn counter_prevention_keeps_vigilance_on_every_legal_creature() {
    let (mut engine, source, creatures) = game_with_counter_prevention(true);
    let targets = creatures.iter().map(|&id| target(&engine, id, 1)).collect();
    apply_ability(&mut engine, 17, source, 0, targets).unwrap();
    resolve_top(&mut engine);
    for id in creatures {
        assert_eq!(
            engine.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
            0
        );
        assert!(engine
            .characteristics(id)
            .unwrap()
            .has_keyword(Keyword::Vigilance));
    }
    assert_eq!(engine.state.players[0].life, 27);
}

fn target(engine: &GameEngine, object_id: u32, counter_amount: u32) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id,
        counter_amount,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(
            engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
        ),
        ..Default::default()
    }
}

#[test]
fn malformed_counter_quantities_are_rejected_before_loyalty_payment() {
    let (mut valid, source, creatures) = game();
    let targets = vec![target(&valid, creatures[0], 3)];
    apply_ability(&mut valid, 17, source, 0, targets).expect("the fixed-three activation is legal");
    assert_eq!(
        valid.state.objects[&source].counter_count(CounterKind::Loyalty),
        1
    );
    for amount in [0, 2, 4, u32::MAX] {
        let (mut engine, source, creatures) = game();
        let targets = vec![target(&engine, creatures[0], amount)];
        let before = ron::to_string(&engine.state).unwrap();
        assert!(
            apply_ability(&mut engine, 17, source, 0, targets).is_err(),
            "invalid quantity {amount} must reject before -3"
        );
        assert_eq!(ron::to_string(&engine.state).unwrap(), before);
    }
}

fn resolve_top(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut batch = None;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("counter distribution resolves"),
        );
    }
    batch.unwrap()
}

#[test]
fn fixed_distribution_and_vigilance_cover_the_same_legal_target_cohort() {
    for amounts in [vec![3], vec![2, 1], vec![1, 1, 1], vec![]] {
        let (mut engine, source, creatures) = game();
        let targets = creatures
            .iter()
            .zip(&amounts)
            .map(|(&id, &amount)| target(&engine, id, amount))
            .collect();
        let activation = apply_ability(&mut engine, 17, source, 0, targets).unwrap();
        let announced = activation
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::StackPushed(pushed)) => Some(pushed),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            announced
                .targets
                .iter()
                .map(|target| target.counter_amount)
                .collect::<Vec<_>>(),
            amounts
        );
        assert_eq!(
            engine
                .state
                .stack
                .last()
                .unwrap()
                .targets
                .iter()
                .map(|target| target.counter_amount)
                .collect::<Vec<_>>(),
            amounts
        );
        for &id in &creatures {
            assert_eq!(
                engine.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
                0
            );
        }
        resolve_top(&mut engine);
        for (index, &id) in creatures.iter().enumerate() {
            let amount = amounts.get(index).copied().unwrap_or(0);
            assert_eq!(
                engine.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne),
                amount
            );
            assert_eq!(
                engine
                    .characteristics(id)
                    .unwrap()
                    .has_keyword(Keyword::Vigilance),
                amount > 0
            );
        }
        assert_eq!(
            engine
                .state
                .players
                .iter()
                .map(|p| p.life)
                .collect::<Vec<_>>(),
            vec![27; 3]
        );
    }
}

#[test]
fn ordinary_loyalty_offer_publishes_fixed_counter_group_and_exact_generations() {
    let (mut engine, source, creatures) = game();
    let batch = engine.initial_response_batch();
    let key = (u64::from(source)) << 32;
    let targets = &batch.legal_by_player[&17].valid_targets_by_ability[&key];
    let allocation = targets.counter_allocation.as_ref().unwrap();
    assert_eq!((allocation.fixed_total, allocation.group_index), (3, 0));
    assert!(!targets.is_damage_targets);
    let group = &targets.groups[0];
    assert_eq!((group.min, group.max), (0, 3));
    for id in creatures {
        assert!(group
            .valid_permanent_targets
            .iter()
            .any(|candidate| candidate.object_id == id && candidate.zone_change_generation == 0));
    }
}

#[test]
fn counter_offer_keeps_permanent_and_player_domains_distinct_when_ids_overlap() {
    let (mut engine, source, creatures) = game();
    let id = 17;
    for player in &mut engine.state.players {
        player.library.retain(|oid| *oid != id);
        player.hand.retain(|oid| *oid != id);
    }
    let mut object = engine.state.objects[&creatures[0]].clone();
    object.id = id;
    object.card_id = "grizzly_bears".into();
    object.owner = 17;
    object.controller = 17;
    object.base_controller = 17;
    object.zone = tricerules_core::Zone::Battlefield;
    engine.state.objects.insert(id, object);
    engine.state.players[0].battlefield.push(id);
    let batch = engine.initial_response_batch();
    let targets = &batch.legal_by_player[&17].valid_targets_by_ability[&(u64::from(source) << 32)];
    assert!(targets.groups[0].valid_permanent_ids.contains(&id));
    assert!(
        !targets.groups[0].can_target_self,
        "Creature allocation cannot offer a player sharing its OID"
    );
    assert!(!targets.groups[0].can_target_opponent);
}

#[test]
fn partial_illegality_loses_only_its_share_and_all_illegal_targets_stop_the_tail() {
    for illegal in [1, 2] {
        let (mut engine, source, creatures) = game();
        let targets = vec![
            target(&engine, creatures[0], 2),
            target(&engine, creatures[1], 1),
        ];
        apply_ability(&mut engine, 17, source, 0, targets).unwrap();
        for &id in &creatures[..illegal] {
            engine.state.zone_change_generation.insert(id, 1);
        }
        resolve_top(&mut engine);
        assert_eq!(
            engine.state.objects[&creatures[0]].counter_count(CounterKind::PlusOnePlusOne),
            0
        );
        assert!(!engine
            .characteristics(creatures[0])
            .unwrap()
            .has_keyword(Keyword::Vigilance));
        assert_eq!(
            engine.state.objects[&creatures[1]].counter_count(CounterKind::PlusOnePlusOne),
            u32::from(illegal == 1)
        );
        assert_eq!(
            engine
                .characteristics(creatures[1])
                .unwrap()
                .has_keyword(Keyword::Vigilance),
            illegal == 1
        );
        assert_eq!(
            engine.state.players[0].life,
            if illegal == 1 { 27 } else { 20 }
        );
    }
}
