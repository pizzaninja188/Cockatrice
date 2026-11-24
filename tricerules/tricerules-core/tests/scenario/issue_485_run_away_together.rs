//! Run Away Together (#485): "Choose two target creatures controlled by different players.
//! Return those creatures to their owners' hands."
//!
//! CR 115.1a/601.2c govern the cast-time relation, CR 608.2b its resolution recheck, and the
//! card's rulings say a target that became illegal still supplies its controller (last-known
//! information after it leaves) for the other target's legality. Every game has three players so
//! a pair controlled by two different opponents is distinguishable from fixed You/Opponent filters.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{self as rv1, TargetRef};

const CARD: &str = "run_away_together";

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("island", &[CARD, "unsummon"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, CARD);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
}

fn pair(first: u32, second: u32) -> Vec<TargetRef> {
    [first, second]
        .into_iter()
        .map(|object_id| TargetRef {
            object_id,
            kind: rv1::TargetRefKind::Permanent as i32,
            ..Default::default()
        })
        .collect()
}

fn cast(engine: &mut GameEngine, first: u32, second: u32) -> Result<RuledEventBatch, String> {
    let slot = hand_index_for_card(engine, 0, CARD);
    engine
        .apply_scenario_command(0, &cast_spell(slot, pair(first, second)))
        .map_err(|error| format!("{error:?}"))
}

/// Pass every player in priority order until the stack is empty; return the last batch.
fn resolve_stack(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
    let mut batches = Vec::new();
    while !engine.state.stack.is_empty() {
        for _ in 0..engine.state.players.len() {
            let batch = engine
                .apply_scenario_command(engine.state.priority_player_id(), &pass())
                .expect("pass");
            batches.push(batch);
            if engine.state.stack.is_empty() {
                break;
            }
        }
    }
    batches
}

fn published_group(engine: &mut GameEngine) -> rv1::LegalTargetGroup {
    let slot = hand_index_for_card(engine, 0, CARD);
    let batch = engine.initial_response_batch();
    let targets = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert_eq!(targets.groups.len(), 1);
    targets.groups[0].clone()
}

fn give_control(engine: &mut GameEngine, object_id: u32, controller: usize) {
    let new_controller = engine.state.players[controller].id;
    let object = engine.state.objects.get_mut(&object_id).expect("permanent");
    object.controller = new_controller;
    object.base_controller = new_controller;
    for player in &mut engine.state.players {
        player.battlefield.retain(|&id| id != object_id);
    }
    engine.state.players[controller].battlefield.push(object_id);
}

fn cast_unsummon_in_response(engine: &mut GameEngine, target: u32) {
    ensure_card_in_hand(engine, 0, "unsummon");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("Unsummon in response");
}

fn in_hand(engine: &GameEngine, player: usize, object_id: u32) -> bool {
    engine.state.objects[&object_id].zone == Zone::Hand
        && engine.state.players[player].hand.contains(&object_id)
}

#[test]
fn run_away_together_returns_creatures_of_two_different_opponents() {
    let mut engine = setup(485_001);
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    // Owned by the caster but controlled by the third player: it returns to its owner's hand.
    let second = inject_creature_under_foreign_control(&mut engine, 0, 2, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    let group = published_group(&mut engine);
    assert_eq!((group.min, group.max), (2, 2));
    let relation = group.relation.expect("engine-published relation");
    assert_eq!(relation.mode, rv1::TargetRelationMode::AllDistinct as i32);
    let key = |object_id| {
        relation
            .keys
            .iter()
            .find(|key| {
                key.candidate
                    .as_ref()
                    .is_some_and(|c| c.object_id == object_id)
            })
            .map(|key| key.key)
    };
    assert_eq!(key(first), Some(1));
    assert_eq!(key(second), Some(2));
    assert_eq!(key(own), Some(0));

    cast(&mut engine, first, second).expect("two different opponents are a legal pair");
    let batches = resolve_stack(&mut engine);
    assert!(in_hand(&engine, 1, first));
    assert!(
        in_hand(&engine, 0, second),
        "owner, not controller, receives it"
    );
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
    let resolution = batches.last().expect("resolution batch");
    let moved = permanents_moved_in(resolution)
        .into_iter()
        .map(|event| event.object_id)
        .collect::<Vec<_>>();
    assert!(
        [first, second].iter().all(|oid| moved.contains(oid)),
        "both creatures leave in the one resolution batch: {moved:?}"
    );
}

#[test]
fn run_away_together_rejects_and_unpublishes_a_same_controller_pair() {
    let mut engine = setup(485_002);
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let group = published_group(&mut engine);
    assert!(
        group.valid_permanent_ids.is_empty(),
        "one controller cannot supply two different-controller targets"
    );
    assert!(group.relation.expect("relation").keys.is_empty());

    let before = engine.diagnostic_snapshot().expect("before");
    let error = cast(&mut engine, first, second).expect_err("same controller is illegal");
    assert!(error.contains("different players"), "{error}");
    assert_eq!(engine.diagnostic_snapshot().expect("after"), before);

    let third = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let group = published_group(&mut engine);
    assert_eq!(
        group.valid_permanent_ids.len(),
        3,
        "a second controller reopens the pair"
    );
    let before = engine.diagnostic_snapshot().expect("before");
    assert!(cast(&mut engine, first, second).is_err());
    assert_eq!(engine.diagnostic_snapshot().expect("after"), before);
    cast(&mut engine, first, third).expect("different controllers");
}

#[test]
fn run_away_together_fizzles_when_one_player_controls_both_on_resolution() {
    let mut engine = setup(485_003);
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    cast(&mut engine, first, second).expect("legal pair");
    let spell = engine.state.stack.last().expect("spell").id;

    give_control(&mut engine, second, 1);
    resolve_stack(&mut engine);
    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&spell].zone,
        Zone::Graveyard,
        "the spell doesn't resolve and goes to its owner's graveyard"
    );
}

#[test]
fn run_away_together_compares_a_departed_target_by_last_known_controller() {
    // Different last-known controller: the remaining target is still legal and returns.
    let mut engine = setup(485_004);
    let departed = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let remaining = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    cast(&mut engine, departed, remaining).expect("legal pair");
    cast_unsummon_in_response(&mut engine, departed);
    resolve_stack(&mut engine);
    assert!(in_hand(&engine, 1, departed), "Unsummon returned it first");
    assert!(
        in_hand(&engine, 2, remaining),
        "its last-known controller differs, so the other target returns"
    );

    // Same last-known controller as the remaining target: both are illegal, nothing returns.
    let mut engine = setup(485_005);
    let departed = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let remaining = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    cast(&mut engine, departed, remaining).expect("legal pair");
    give_control(&mut engine, remaining, 1);
    cast_unsummon_in_response(&mut engine, departed);
    resolve_stack(&mut engine);
    assert!(in_hand(&engine, 1, departed), "Unsummon returned it");
    assert_eq!(
        engine.state.objects[&remaining].zone,
        Zone::Battlefield,
        "the departed target was last controlled by the same player"
    );
}

#[test]
fn run_away_together_treats_a_new_incarnation_as_a_new_object() {
    let mut engine = setup(485_006);
    let blinked = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    cast(&mut engine, blinked, other).expect("legal pair");
    // CR 400.7: a permanent that leaves and returns is a new object, not the chosen target.
    *engine
        .state
        .zone_change_generation
        .entry(blinked)
        .or_default() += 2;
    resolve_stack(&mut engine);
    assert_eq!(engine.state.objects[&blinked].zone, Zone::Battlefield);
    assert!(
        in_hand(&engine, 2, other),
        "the remaining target still returns"
    );
}
