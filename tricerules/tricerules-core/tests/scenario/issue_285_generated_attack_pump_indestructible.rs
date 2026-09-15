//! Issue #285 — Hardened Escort and Foot Elite share an exact attack-triggered pump and
//! indestructible effect for another creature controlled by the trigger controller.
//!
//! Oracle and rulings checked 2026-09-14. CR 115.1d and 608.2b govern target publication and
//! resolution revalidation; CR 508.1m/508.3a and 603.2c govern declaration-time triggers;
//! CR 611.2c/613.1f/613.4c govern the ordered temporary P/T and keyword effects; CR 702.12b
//! governs destruction prevention; and CR 514.2 governs cleanup expiry.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn advance_to_next_main1(engine: &mut GameEngine, active_player: i32) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..240 {
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == active_player
            && engine.state.turn_step == TurnStep::Main1
            && engine.state.priority_player_id() == active_player
        {
            return;
        }
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance through the rest of the turn");
    }
    panic!("turn advancement stalled before the next main phase");
}

fn cast_murder(engine: &mut GameEngine, target: u32) {
    inject_card_into_hand(engine, 0, "murder");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Murder at the granted-indestructible creature");
    resolve_entire_stack_two_player(engine);
}

#[test]
fn attack_trigger_publishes_exact_targets_and_applies_ordered_temporary_effects() {
    for (seed, card_id) in [(285_001, "hardened_escort"), (285_002, "foot_elite")] {
        let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
        advance_to_declare_attackers(&mut engine);
        let attacker = inject_creature_on_battlefield(&mut engine, 0, card_id);
        let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let unrelated = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");

        let declaration = engine
            .apply_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the generated creature as an attacker");
        assert_eq!(
            engine.state.pending_triggers.len(),
            1,
            "one attack, one trigger"
        );
        assert_eq!(
            engine.state.stack.len(),
            0,
            "targeted trigger waits for its target"
        );
        let key = u64::from(attacker) << 32;
        let published = &declaration.legal_by_player[&0].valid_targets_by_ability[&key];
        assert_eq!(published.groups.len(), 1);
        let published_ids = &published.groups[0].valid_permanent_ids;
        assert!(published_ids.contains(&target));
        assert!(published_ids.contains(&unrelated));
        assert!(!published_ids.contains(&attacker));
        assert!(!published_ids.contains(&opponent));

        assert!(
            engine
                .apply_command(0, &choose_trigger_target(attacker))
                .is_err(),
            "the attacking source is excluded"
        );
        assert!(
            engine
                .apply_command(0, &choose_trigger_target(opponent))
                .is_err(),
            "an opponent creature is outside the You filter"
        );
        engine
            .apply_command(0, &choose_trigger_target(target))
            .expect("choose another controlled creature");
        assert_eq!(engine.state.pending_triggers.len(), 0);
        assert_eq!(
            engine.state.stack.len(),
            1,
            "one chosen trigger resolves once"
        );

        assert_eq!(engine.effective_power(target), Some(2));
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.effective_power(target), Some(3));
        assert_eq!(engine.effective_toughness(target), Some(2));
        assert!(engine.effective_has_keyword(target, Keyword::Indestructible));
        assert_eq!(engine.effective_power(attacker), Some(2));
        assert_eq!(engine.effective_power(unrelated), Some(2));
        assert!(!engine.effective_has_keyword(unrelated, Keyword::Indestructible));
        assert_eq!(engine.effective_power(opponent), Some(2));

        cast_murder(&mut engine, target);
        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Battlefield,
            "destroy cannot remove the temporarily indestructible target"
        );

        advance_to_next_main1(&mut engine, 0);
        assert_eq!(engine.effective_power(target), Some(2));
        assert_eq!(engine.effective_toughness(target), Some(2));
        assert!(!engine.effective_has_keyword(target, Keyword::Indestructible));
        assert_eq!(engine.effective_power(unrelated), Some(2));
    }
}

#[test]
fn attack_trigger_revalidates_a_removed_target_by_exact_zone_generation() {
    let mut engine = GameEngine::new(285_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "hardened_escort");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let unrelated = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare Hardened Escort as an attacker");
    engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose the initially legal controlled creature");

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[0].hand.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert!(engine.state.continuous_effects.is_empty());
    assert_eq!(engine.effective_power(unrelated), Some(2));
    assert!(!engine.effective_has_keyword(unrelated, Keyword::Indestructible));
}

#[test]
fn attack_trigger_rejects_same_object_reentry_with_new_zone_generation() {
    let mut engine = GameEngine::new(285_004, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "hardened_escort");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let unrelated = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare Hardened Escort as an attacker");
    engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose the initially legal controlled creature");

    let old_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or_default();
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[0].hand.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;
    engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != target);
    engine.state.players[0].battlefield.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_insert(0) += 1;
    let new_generation = *engine
        .state
        .zone_change_generation
        .get(&target)
        .expect("reentered target generation");
    assert!(new_generation > old_generation);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(target), Some(2));
    assert_eq!(engine.effective_toughness(target), Some(2));
    assert!(!engine.effective_has_keyword(target, Keyword::Indestructible));
    assert_eq!(engine.effective_power(unrelated), Some(2));
    assert!(!engine.effective_has_keyword(unrelated, Keyword::Indestructible));
    assert!(engine.state.continuous_effects.is_empty());
}
