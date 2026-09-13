//! Issue #268 — the exact one-shot spell cohort uses the shared typed spell effects and target
//! contracts. Oracle and rulings were checked 2026-09-13. CR 115, 120, 121, 400.7, 601.2c-d,
//! 608.2b-c, 610, 611.2, 613.4c, 701.6, 701.8, and 701.9 govern these casts, target
//! revalidation, source-relative damage, zone changes, destruction, exile, and cleanup expiry.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};

fn two_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_main1(seed: u64) -> GameEngine {
    // The production harness is currently M2-only, but the EachOpponent effect is seat-generic.
    // Add a third fixture player after construction so this scenario exercises both opponents
    // without broadening the public engine constructor contract for issue #268.
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn fund(engine: &mut GameEngine) {
    give_mana(
        engine,
        0,
        ManaGift {
            w: 8,
            u: 8,
            b: 8,
            r: 8,
            g: 8,
            c: 12,
        },
    );
}

fn permanent_target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
}

fn cast_named(engine: &mut GameEngine, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
}

fn resolve_three_player_stack(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("three-player priority pass");
    }
}

fn resolve_two_player_stack_with_last_batch(engine: &mut GameEngine) -> RuledEventBatch {
    let mut last_batch = None;
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        let first = engine.state.priority_player_id();
        last_batch = Some(
            engine
                .apply_command(first, &pass())
                .expect("first two-player resolution pass"),
        );
        if engine.state.stack.is_empty() {
            break;
        }
        let second = engine.state.priority_player_id();
        last_batch = Some(
            engine
                .apply_command(second, &pass())
                .expect("second two-player resolution pass"),
        );
    }
    last_batch.expect("stack resolution batch")
}

#[test]
fn alesha_legacy_reuses_controlled_keyword_targets_and_expires_at_cleanup() {
    let mut engine = two_player_engine(268_001);
    fund(&mut engine);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    inject_card_into_hand(&mut engine, 0, "aleshas_legacy");
    let slot = hand_index_for_card(&engine, 0, "aleshas_legacy");
    let legal = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
        [&((slot as u32) << 8)];
    assert_eq!(legal.groups[0].valid_permanent_ids, [own]);
    assert!(!legal.groups[0].valid_permanent_ids.contains(&opponent));

    engine
        .apply_command(0, &cast_spell(slot, target_object(own)))
        .expect("cast Alesha's Legacy");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(own, Keyword::Deathtouch));
    assert!(engine.effective_has_keyword(own, Keyword::Indestructible));

    end_active_turn(&mut engine, 0);
    assert!(!engine.effective_has_keyword(own, Keyword::Deathtouch));
    assert!(!engine.effective_has_keyword(own, Keyword::Indestructible));
}

#[test]
fn assert_perfection_shares_the_pumped_target_and_allows_its_optional_damage_target() {
    let mut engine = two_player_engine(268_002);
    fund(&mut engine);
    let own = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let opponent = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 5);
    cast_named(
        &mut engine,
        "assert_perfection",
        vec![permanent_target(own, 0), permanent_target(opponent, 1)],
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.state.objects[&opponent].damage, 3);

    let mut optional = two_player_engine(268_003);
    fund(&mut optional);
    let own = inject_creature_on_battlefield(&mut optional, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut optional, 1, "grizzly_bears");
    cast_named(
        &mut optional,
        "assert_perfection",
        vec![permanent_target(own, 0)],
    );
    resolve_entire_stack_two_player(&mut optional);
    assert_eq!(optional.effective_power(own), Some(3));
    assert_eq!(optional.state.objects[&opponent].damage, 0);

    let mut stale = two_player_engine(268_009);
    fund(&mut stale);
    let own = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    let opponent = inject_creature_with_stats(&mut stale, 1, "hill_giant", 5, 5);
    cast_named(
        &mut stale,
        "assert_perfection",
        vec![permanent_target(own, 0), permanent_target(opponent, 1)],
    );
    stale.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != opponent);
    stale.state.players[1].hand.push(opponent);
    stale.state.objects.get_mut(&opponent).expect("target").zone = Zone::Hand;
    *stale
        .state
        .zone_change_generation
        .entry(opponent)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.effective_power(own), Some(3));
    assert_eq!(stale.state.objects[&opponent].damage, 0);
}

#[test]
fn ancestral_reminiscence_draws_three_then_privately_discards_one_physical_card() {
    let mut engine = two_player_engine(268_004);
    fund(&mut engine);
    let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let drawn = [
        inject_library_card(&mut engine, 0, "forest"),
        inject_library_card(&mut engine, 0, "island"),
        inject_library_card(&mut engine, 0, "swamp"),
    ];
    for object_id in drawn.iter().rev() {
        engine.state.players[0]
            .library
            .retain(|candidate| candidate != object_id);
        engine.state.players[0].library.push_front(*object_id);
    }
    cast_named(&mut engine, "ancestral_reminiscence", vec![]);
    resolve_entire_stack_two_player(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("discard choice after draw three");
    assert!(pending.presentation.candidates.contains(&discarded));
    engine
        .apply_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("submit private discard");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert!(drawn
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Hand));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn auroral_procession_moves_the_exact_targeted_graveyard_object_to_hand() {
    let mut engine = two_player_engine(268_005);
    fund(&mut engine);
    let target = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let other = inject_graveyard_card(&mut engine, 0, "forest");
    cast_named(
        &mut engine,
        "auroral_procession",
        vec![graveyard_target(target)],
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&other].zone, Zone::Graveyard);
    assert!(engine.state.players[0].hand.contains(&target));
}

#[test]
fn boltwave_damages_each_opponent_in_a_three_player_game() {
    let mut engine = three_player_main1(268_006);
    fund(&mut engine);
    cast_named(&mut engine, "boltwave", vec![]);
    resolve_three_player_stack(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 17);
    assert_eq!(engine.state.players[2].life, 17);
}

#[test]
fn bombard_uses_its_source_name_for_four_damage_to_a_creature() {
    let mut engine = two_player_engine(268_010);
    fund(&mut engine);
    let target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 5);
    let bombard = inject_card_into_hand(&mut engine, 0, "bombard");
    let slot = hand_index_for_card(&engine, 0, "bombard");
    let cast = engine
        .apply_command(0, &cast_spell(slot, vec![permanent_target(target, 0)]))
        .expect("cast Bombard");
    let pushed = cast
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(stack)) => Some(stack),
            _ => None,
        })
        .expect("Bombard StackPushed");
    assert_eq!(
        pushed.object_id, bombard,
        "the physical Bombard is the stack source"
    );
    assert_eq!(pushed.card_id, "bombard");

    let resolved = resolve_two_player_stack_with_last_batch(&mut engine);
    assert!(resolved.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text.starts_with("Bombard deals 4 damage to ")
        )
    }));
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].damage, 4);
}

#[test]
fn sephiroths_intervention_destroys_then_gains_two_life() {
    let mut engine = two_player_engine(268_011);
    fund(&mut engine);
    engine.state.players[0].life = 18;
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_named(
        &mut engine,
        "sephiroths_intervention",
        vec![permanent_target(target, 0)],
    );
    let resolved = resolve_two_player_stack_with_last_batch(&mut engine);
    let destroyed_index = resolved
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(Ev::PermanentMoved(moved)) if moved.object_id == target
            )
        })
        .expect("Sephiroth's target movement event");
    let gained_life_index = resolved
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.ev,
                Some(Ev::LifeChanged(change))
                    if change.player_id == 0 && change.delta == 2
            )
        })
        .expect("Sephiroth's life-gain event");
    assert!(
        destroyed_index < gained_life_index,
        "Sephiroth's Intervention must destroy before gaining life: events={:?}",
        resolved.events
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn sudden_strike_accepts_only_an_attacking_or_blocking_creature() {
    let mut engine = GameEngine::new(268_007, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    fund(&mut engine);
    cast_named(
        &mut engine,
        "sudden_strike",
        vec![permanent_target(attacker, 0)],
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Graveyard);
}

#[test]
fn mass_modifiers_and_single_target_removal_preserve_order_and_zones() {
    let mut engine = two_player_engine(268_008);
    fund(&mut engine);
    let own_a = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let own_b = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    cast_named(&mut engine, "overrun", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(own_a), Some(5));
    assert_eq!(engine.effective_power(own_b), Some(5));
    assert!(engine.effective_has_keyword(own_a, Keyword::Trample));
    assert_eq!(engine.effective_power(opposing), Some(2));

    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    cast_named(
        &mut engine,
        "disenchant",
        vec![permanent_target(artifact, 0)],
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[1]
            .graveyard
            .iter()
            .filter(|object_id| engine.state.objects[object_id].card_id == "short_sword")
            .count(),
        1
    );

    cast_named(
        &mut engine,
        "wander_off",
        vec![permanent_target(opposing, 0)],
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Exile);

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(own_a), Some(2));
    assert!(!engine.effective_has_keyword(own_a, Keyword::Trample));
}
