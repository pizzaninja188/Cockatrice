//! Reviewed direct-RON conditional/modal scenarios: Bristlepack Sentry, Child of the Volcano,
//! Shipwreck Sentry, Stormcatch Mentor, Lassoed by the Law, Live or Die and Kavaron Skywarden.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 400.7/608.2b (graveyard return), CR 601.2f (cost reduction), CR 603.4
//! (intervening-if), CR 603.6a (entry trigger), CR 610.3 (exile until source leaves), CR 611.2c/
//! 613.4c (P/T and keywords), CR 700.2a/c (modal choice), CR 701.8 (destroy), CR 701.9 (discard),
//! CR 702.3b (defender exception), CR 702.10 (haste), CR 702.17 (reach), and CR 702.19 (trample).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    ChooseTriggerTarget, DevCommand, DevPutCardInZone, DevZone, RuledCommand, TargetRef,
    TargetRefKind,
};

fn dev_put_ready(e: &mut GameEngine, player: i32, card_name: &str) {
    e.apply_command(
        player,
        &RuledCommand {
            cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::DevCommand(
                DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: card_name.to_string(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                },
            )),
        },
    )
    .unwrap_or_else(|error| panic!("dev put {card_name}: {error:?}"));
}

fn roll_into_declare_attackers(e: &mut GameEngine, active: i32) {
    e.apply_command(active, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn clear_pool(e: &mut GameEngine, player: usize) {
    e.state.players[player].mana_pool = Default::default();
}

fn cast(
    e: &mut GameEngine,
    player: i32,
    card_id: &str,
    targets: Vec<TargetRef>,
) -> Option<tricerules_proto::ruled::v1::ResolutionChoiceRequired> {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    let batch = semantic::accepted(e, player, &cast_spell(slot, targets));
    if let Some(choice) = find_resolution_choice(&batch) {
        return Some(choice);
    }
    for _ in 0..40 {
        if e.state.stack.is_empty() || e.state.blocking_choice().is_some() {
            return None;
        }
        let priority = e.state.priority_player_id();
        let batch = semantic::accepted(e, priority, &pass());
        if let Some(choice) = find_resolution_choice(&batch) {
            return Some(choice);
        }
    }
    panic!("cast never settled");
}

fn cast_modal(e: &mut GameEngine, player: i32, card_id: &str, modes: Vec<(u32, Vec<TargetRef>)>) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_modal_spell(slot, modes));
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        group_index: 0,
                        kind: TargetRefKind::Permanent as i32,
                        ..Default::default()
                    }],
                },
            ),
        ),
    }
}

fn battlefield_object(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    *e.state.players[player]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn advance_to_end_step(engine: &mut GameEngine, active: i32) {
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
}

fn resolve_stack_collecting(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        pass_both_players(engine);
    }
}

#[test]
fn issue_misc14_bristlepack_sentry_needs_a_power_four_creature_to_attack() {
    // With an Air Elemental (4/4) controlled, the condition holds.
    let mut allowed = engine(814_001);
    allowed.enable_dev_commands();
    dev_put_ready(&mut allowed, 0, "Bristlepack Sentry");
    let sentry = battlefield_object(&allowed, 0, "bristlepack_sentry");
    dev_put_ready(&mut allowed, 0, "Air Elemental");
    roll_into_declare_attackers(&mut allowed, 0);
    allowed
        .apply_command(0, &declare_attackers(vec![sentry]))
        .expect("a power-4 creature unlocks the attack");
    assert!(allowed.state.objects[&sentry].tapped, "the Sentry attacked");

    // Without a power-4 creature it keeps defender.
    let mut blocked = engine(814_011);
    blocked.enable_dev_commands();
    dev_put_ready(&mut blocked, 0, "Bristlepack Sentry");
    let sentry = battlefield_object(&blocked, 0, "bristlepack_sentry");
    dev_put_ready(&mut blocked, 0, "Grizzly Bears");
    roll_into_declare_attackers(&mut blocked, 0);
    assert!(
        blocked
            .apply_command(0, &declare_attackers(vec![sentry]))
            .is_err(),
        "defender stops the attack without a power-4 creature"
    );
}

#[test]
fn issue_misc14_child_of_the_volcano_counts_on_descend_at_end_step() {
    let mut e = engine(814_002);
    let child = inject_creature_on_battlefield(&mut e, 0, "child_of_the_volcano");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, 0, "murder", target_object(fodder));
    assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard);
    advance_to_end_step(&mut e, 0);
    resolve_stack_collecting(&mut e);
    assert_eq!(
        e.state.objects[&child].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1,
        "descending this turn adds a +1/+1 counter"
    );

    // Without a descent, the intervening-if fails.
    let mut none = engine(814_012);
    let child = inject_creature_on_battlefield(&mut none, 0, "child_of_the_volcano");
    advance_to_end_step(&mut none, 0);
    resolve_stack_collecting(&mut none);
    assert_eq!(
        none.state.objects[&child].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn issue_misc14_shipwreck_sentry_needs_an_artifact_entry_to_attack() {
    // A real artifact enters this turn, satisfying the condition.
    let mut allowed = engine(814_003);
    allowed.enable_dev_commands();
    dev_put_ready(&mut allowed, 0, "Swiftfoot Boots");
    dev_put_ready(&mut allowed, 0, "Shipwreck Sentry");
    let sentry = battlefield_object(&allowed, 0, "shipwreck_sentry");
    dev_put_ready(&mut allowed, 0, "Grizzly Bears");
    roll_into_declare_attackers(&mut allowed, 0);
    allowed
        .apply_command(0, &declare_attackers(vec![sentry]))
        .expect("an artifact entered this turn unlocks the attack");
    assert!(allowed.state.objects[&sentry].tapped);

    // No artifact entry keeps defender.
    let mut blocked = engine(814_013);
    blocked.enable_dev_commands();
    dev_put_ready(&mut blocked, 0, "Shipwreck Sentry");
    let sentry = battlefield_object(&blocked, 0, "shipwreck_sentry");
    dev_put_ready(&mut blocked, 0, "Grizzly Bears");
    roll_into_declare_attackers(&mut blocked, 0);
    assert!(
        blocked
            .apply_command(0, &declare_attackers(vec![sentry]))
            .is_err(),
        "defender stops the attack without an artifact entry"
    );
}

#[test]
fn issue_misc14_stormcatch_mentor_has_prowess_and_reduces_instants() {
    let mut e = engine(814_004);
    inject_creature_on_battlefield(&mut e, 0, "stormcatch_mentor");
    let mentor = battlefield_object(&e, 0, "stormcatch_mentor");
    let power = e.effective_power(mentor).expect("power");
    cast(&mut e, 0, "divination", vec![]);
    assert_eq!(e.effective_power(mentor), Some(power + 1), "prowess");

    // The reduction turns Divination's {2}{U} into {1}{U}.
    let mut reduced = engine(814_014);
    inject_creature_on_battlefield(&mut reduced, 0, "stormcatch_mentor");
    clear_pool(&mut reduced, 0);
    inject_card_into_hand(&mut reduced, 0, "divination");
    give_mana(
        &mut reduced,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&reduced, 0, "divination");
    semantic::accepted(&mut reduced, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut reduced);

    // Without the Mentor, {1}{U} is not enough for {2}{U}.
    let mut full = engine(814_024);
    clear_pool(&mut full, 0);
    inject_card_into_hand(&mut full, 0, "divination");
    give_mana(
        &mut full,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&full, 0, "divination");
    assert!(
        full.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the unreduced {{2}}{{U}} is not castable with {{1}}{{U}}"
    );
}

#[test]
fn issue_misc14_lassoed_by_the_law_exiles_and_makes_a_mercenary() {
    let mut e = engine(814_005);
    let their_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "lassoed_by_the_law", vec![]);
    answer_trigger_order_in_engine_order(&mut e);
    semantic::accepted(&mut e, 0, &choose_trigger_target(their_creature));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&their_creature].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&e, 0, "mercenary_r_1_1").len(), 1);

    // Leaving the battlefield returns the exiled permanent.
    let lassoed = battlefield_object(&e, 0, "lassoed_by_the_law");
    cast(&mut e, 0, "disenchant", target_object(lassoed));
    assert_eq!(e.state.objects[&their_creature].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&their_creature].controller, 1);
}

#[test]
fn issue_misc14_live_or_die_modes_reanimate_or_destroy() {
    // Mode index 1: destroy target creature.
    let mut destroy = engine(814_006);
    let their_creature = inject_creature_on_battlefield(&mut destroy, 1, "grizzly_bears");
    cast_modal(
        &mut destroy,
        0,
        "live_or_die",
        vec![(1, target_object(their_creature))],
    );
    resolve_entire_stack_two_player(&mut destroy);
    assert_eq!(destroy.state.objects[&their_creature].zone, Zone::Graveyard);

    // Mode index 0: return a creature card from the graveyard.
    let mut reanimate = engine(814_016);
    let bear = inject_graveyard_card(&mut reanimate, 0, "grizzly_bears");
    inject_card_into_hand(&mut reanimate, 0, "live_or_die");
    grant_pool(&mut reanimate, 0);
    let slot = hand_index_for_card(&reanimate, 0, "live_or_die");
    semantic::accepted(
        &mut reanimate,
        0,
        &cast_modal_spell(slot, vec![(0, target_object(bear))]),
    );
    for _ in 0..20 {
        if let Some(choice) = reanimate.state.pending_resolution.as_ref() {
            if choice.presentation.candidates.contains(&bear) {
                break;
            }
        }
        if reanimate.state.stack.is_empty() && reanimate.state.pending_resolution.is_none() {
            break;
        }
        let priority = reanimate.state.priority_player_id();
        semantic::accepted(&mut reanimate, priority, &pass());
    }
    if reanimate.state.pending_resolution.is_some() {
        semantic::accepted(&mut reanimate, 0, &submit_resolution_choice(vec![bear]));
    }
    assert_eq!(reanimate.state.objects[&bear].zone, Zone::Battlefield);
}

#[test]
fn issue_misc14_kavaron_skywarden_counts_void_at_end_step() {
    let mut e = engine(814_008);
    let kavaron = inject_creature_on_battlefield(&mut e, 0, "kavaron_skywarden");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // A nonland permanent leaving the battlefield this turn satisfies Void.
    cast(&mut e, 0, "murder", target_object(fodder));
    assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard);
    advance_to_end_step(&mut e, 0);
    resolve_stack_collecting(&mut e);
    assert_eq!(
        e.state.objects[&kavaron].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );

    // With nothing leaving and no warp, Void fails.
    let mut none = engine(814_018);
    let kavaron = inject_creature_on_battlefield(&mut none, 0, "kavaron_skywarden");
    advance_to_end_step(&mut none, 0);
    resolve_stack_collecting(&mut none);
    assert_eq!(
        none.state.objects[&kavaron].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        0
    );
}
