//! Issue #481 — CR 608.2b/c target revalidation and instruction order, CR 701.22 Scry, and
//! CR 701.25 Surveil through an existing conditional effect's library-choice continuation.

use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine_with_spell(seed: u64, spell: &str, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[spell]), vec!["forest".into(); 20]]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, spell);
    engine
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn cast_targeted_instant(engine: &mut GameEngine, card_id: &str, target_id: u32) -> u32 {
    let hand_index = hand_index_for_card(engine, 0, card_id);
    let spell_id = engine.state.players[0].hand[hand_index];
    let mana = match card_id {
        "taken_by_nightmares" => ManaGift {
            b: 2,
            c: 2,
            ..Default::default()
        },
        "failed_fording" => ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        _ => panic!("unexpected issue-481 card {card_id}"),
    };
    give_mana(engine, 0, mana);
    engine
        .apply_command(0, &cast_spell(hand_index, target_object(target_id)))
        .expect("cast with the chosen target");
    spell_id
}

fn resolve_with_both_passes(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes and spell resolves")
}

fn move_battlefield_card_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn taken_by_nightmares_checks_enchantment_at_resolution_and_resumes_after_scry() {
    let mut engine = engine_with_spell(481_001, "taken_by_nightmares", "swamp");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
    let spell = cast_targeted_instant(&mut engine, "taken_by_nightmares", target);

    // The controller had no enchantment while casting. Add one before the spell resolves.
    let enchantment = inject_permanent_on_battlefield(&mut engine, 0, "air_nomad_legacy");
    let scry = resolve_with_both_passes(&mut engine);
    let choice = find_resolution_choice(&scry).expect("resolution-time enchantment enables Scry 2");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, top);
    assert_eq!(choice.min, 0);
    assert_eq!(choice.max, 2);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(engine.state.players[1].exile.contains(&target));
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Scry choice parks the stack item")
            .continuation,
        ResolutionContinuation::LibraryPartition {
            kind: PendingLibraryPartitionKind::Scry,
            ..
        }
    ));

    // The condition is not re-evaluated after a nested choice. The controller can change the
    // battlefield while Scry is parked; answering the existing choice still completes resolution.
    move_battlefield_card_to_graveyard(&mut engine, 0, enchantment);
    let order = engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both cards on top");
    let order_choice = find_resolution_choice(&order).expect("order both cards kept on top");
    assert_eq!(order_choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((order_choice.min, order_choice.max), (2, 2));
    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("finish Scry ordering and resume the spell");
    assert!(find_resolution_choice(&completed).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn taken_by_nightmares_ignores_an_opponents_enchantment() {
    let mut engine = engine_with_spell(481_002, "taken_by_nightmares", "swamp");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 1, "air_nomad_legacy");
    cast_targeted_instant(&mut engine, "taken_by_nightmares", target);

    let resolution = resolve_with_both_passes(&mut engine);
    assert!(find_resolution_choice(&resolution).is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn taken_by_nightmares_with_an_illegal_target_does_not_scry() {
    let mut engine = engine_with_spell(481_003, "taken_by_nightmares", "swamp");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 0, "air_nomad_legacy");
    cast_targeted_instant(&mut engine, "taken_by_nightmares", target);
    move_battlefield_card_to_graveyard(&mut engine, 1, target);

    let resolution = resolve_with_both_passes(&mut engine);
    assert!(find_resolution_choice(&resolution).is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn failed_fording_checks_desert_at_resolution_then_surveils() {
    let mut engine = engine_with_spell(481_004, "failed_fording", "island");
    let target = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
    let spell = cast_targeted_instant(&mut engine, "failed_fording", target);

    // The controller had no Desert while casting. Add one before the spell resolves.
    inject_permanent_on_battlefield(&mut engine, 0, "abraded_bluffs");
    let surveil = resolve_with_both_passes(&mut engine);
    let choice =
        find_resolution_choice(&surveil).expect("resolution-time Desert enables Surveil 1");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [top[0]]);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_selectable[0]);
    assert!(engine.state.players[1].hand.contains(&target));
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Surveil choice parks the stack item")
            .continuation,
        ResolutionContinuation::LibraryPartition {
            kind: PendingLibraryPartitionKind::Surveil,
            ..
        }
    ));

    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put the surveilled card into the graveyard");
    assert!(find_resolution_choice(&completed).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&top[0]));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
}

#[test]
fn failed_fording_ignores_an_opponents_desert() {
    let mut engine = engine_with_spell(481_005, "failed_fording", "island");
    let target = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    inject_permanent_on_battlefield(&mut engine, 1, "abraded_bluffs");
    cast_targeted_instant(&mut engine, "failed_fording", target);

    let resolution = resolve_with_both_passes(&mut engine);
    assert!(find_resolution_choice(&resolution).is_none());
    assert!(engine.state.players[1].hand.contains(&target));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn failed_fording_with_an_illegal_target_does_not_surveil() {
    let mut engine = engine_with_spell(481_006, "failed_fording", "island");
    let target = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    inject_permanent_on_battlefield(&mut engine, 0, "abraded_bluffs");
    cast_targeted_instant(&mut engine, "failed_fording", target);
    move_battlefield_card_to_graveyard(&mut engine, 1, target);

    let resolution = resolve_with_both_passes(&mut engine);
    assert!(find_resolution_choice(&resolution).is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}
