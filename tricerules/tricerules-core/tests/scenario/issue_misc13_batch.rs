//! Reviewed direct-RON Vehicle/Equipment/spell scenarios: Detention Chariot, Strixhaven Skycoach,
//! Ragged Short Spear, Pterafractyl, Auron's Inspiration, Dictate of Kruphix, Rowdy Research and
//! Eject.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 301.5/702.6
//! (Equipment and equip), CR 508.1k/611.2c/613.4c (combat pump), CR 601.2f/608.2i (cost
//! reduction), CR 603.6a (entry trigger), CR 610.3/608.2b (exile until source leaves), CR 614.1c/
//! 122.6 (enters with X counters), CR 702.8 (flash), CR 702.9 (flying), CR 702.29 (cycling),
//! CR 702.34 (flashback), and CR 702.122 (crew).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, AbilitySourceZone, ActivateAbility, CastMethod,
    CastSpell, ChoiceKind, ChooseTriggerTarget, CostObjectRef, CostObjectRefs, CostSelection,
    ResolutionChoiceDecision, ResolutionChoiceRequired, RuledCommand, TargetRef, TargetRefKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Casts and drives passes until the stack settles, a resolution choice appears, or a pending
/// trigger target remains.
fn cast(
    e: &mut GameEngine,
    player: i32,
    card_id: &str,
    targets: Vec<TargetRef>,
) -> Option<ResolutionChoiceRequired> {
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

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                decision: ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: index,
                ..Default::default()
            },
        )),
    }
}

fn battlefield_object(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    *e.state.players[player]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn generation(e: &GameEngine, object_id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn cost_selection(cost_index: u32, e: &GameEngine, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: generation(e, *object_id),
                })
                .collect(),
        })),
    }
}

fn activate_on(
    e: &GameEngine,
    object_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
    cost_selections: Vec<CostSelection>,
) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(object_id, ability_index, targets, cost_selections);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    command
}

fn cycle_from_hand(e: &GameEngine, object_id: u32, ability_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: object_id,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: generation(e, object_id),
            ability_index,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_misc13_detention_chariot_exiles_returns_crews_and_cycles() {
    let mut e = engine(813_001);
    let their_boots = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    cast(&mut e, 0, "detention_chariot", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(their_boots));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&their_boots].zone, Zone::Exile);

    // Crew 3: two 2/2 creatures supply 4 power.
    let chariot = battlefield_object(&e, 0, "detention_chariot");
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(!e.characteristics(chariot).unwrap().is_creature());
    let crew = activate_on(
        &e,
        chariot,
        0,
        vec![],
        vec![cost_selection(0, &e, &[first, second])],
    );
    semantic::accepted(&mut e, 0, &crew);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&first].tapped && e.state.objects[&second].tapped);
    let crewed = e.characteristics(chariot).unwrap();
    assert!(crewed.is_creature());
    assert_eq!((crewed.power, crewed.toughness), (Some(6), Some(6)));

    // Leaving the battlefield returns the exiled card to its owner.
    cast(&mut e, 0, "murder", target_object(chariot));
    assert_eq!(e.state.objects[&chariot].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&their_boots].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&their_boots].controller, 1);

    // Cycling {W} from hand.
    let cycling_card = inject_card_into_hand(&mut e, 0, "detention_chariot");
    grant_pool(&mut e, 0);
    let hand_before = e.state.players[0].hand.len();
    let cycling = cycle_from_hand(&e, cycling_card, 1);
    semantic::accepted(&mut e, 0, &cycling);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&cycling_card].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
}

#[test]
fn issue_misc13_strixhaven_skycoach_searches_and_crews() {
    let mut e = engine(813_002);
    let basic = inject_library_card(&mut e, 0, "forest");
    let choice = cast(&mut e, 0, "strixhaven_skycoach", vec![]).expect("optional search branch");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    semantic::accepted(&mut e, 0, &select_branch(0));
    let search = {
        let mut found = None;
        for _ in 0..20 {
            if let Some(c) = e.state.pending_resolution.as_ref() {
                if c.presentation.candidates.contains(&basic) {
                    found = Some(c.clone());
                    break;
                }
            }
            let priority = e.state.priority_player_id();
            semantic::accepted(&mut e, priority, &pass());
        }
        found.expect("basic-land search choice")
    };
    let _ = search;
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![basic]));
    assert_eq!(e.state.objects[&basic].zone, Zone::Hand);

    // Crew 2 with one 2-power creature.
    let skycoach = battlefield_object(&e, 0, "strixhaven_skycoach");
    let pilot = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let crew = activate_on(
        &e,
        skycoach,
        0,
        vec![],
        vec![cost_selection(0, &e, &[pilot])],
    );
    semantic::accepted(&mut e, 0, &crew);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&pilot].tapped);
    let crewed = e.characteristics(skycoach).unwrap();
    assert!(crewed.is_creature());
    assert_eq!((crewed.power, crewed.toughness), (Some(3), Some(2)));
}

#[test]
fn issue_misc13_ragged_short_spear_rummages_and_equips_for_two_power() {
    let mut e = engine(813_003);
    let pitch = inject_card_into_hand(&mut e, 0, "island");
    let choice = cast(&mut e, 0, "ragged_short_spear", vec![]).expect("optional rummage choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.min, 0, "the discard is optional");
    assert!(choice.candidate_object_ids.contains(&pitch));
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![pitch]));
    assert_eq!(e.state.objects[&pitch].zone, Zone::Graveyard);

    let spear = battlefield_object(&e, 0, "ragged_short_spear");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(bear).expect("power");
    let equip = activate_on(&e, spear, 0, target_object(bear), vec![]);
    semantic::accepted(&mut e, 0, &equip);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(base_power + 2));
    assert_eq!(e.effective_toughness(bear), Some(2), "no toughness bonus");
}

#[test]
fn issue_misc13_pterafractyl_enters_with_x_counters_and_gains_life() {
    let mut e = engine(813_004);
    let life_before = e.state.players[0].life;
    inject_card_into_hand(&mut e, 0, "pterafractyl");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "pterafractyl");
    semantic::accepted(&mut e, 0, &cast_spell_x(slot, vec![], 2));
    resolve_entire_stack_two_player(&mut e);
    let pterafractyl = battlefield_object(&e, 0, "pterafractyl");
    assert_eq!(
        e.state.objects[&pterafractyl].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2,
        "X +1/+1 counters"
    );
    assert_eq!(e.effective_power(pterafractyl), Some(3));
    assert_eq!(e.effective_toughness(pterafractyl), Some(2));
    assert_eq!(e.state.players[0].life, life_before + 2);
}

#[test]
fn issue_misc13_aurons_inspiration_pumps_attackers_and_flashbacks() {
    let mut e = GameEngine::new(813_005, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![first, second]))
        .expect("declare attackers");
    let (p1, p2) = (
        e.effective_power(first).unwrap(),
        e.effective_power(second).unwrap(),
    );
    let auron = {
        inject_card_into_hand(&mut e, 0, "aurons_inspiration");
        grant_pool(&mut e, 0);
        let slot = hand_index_for_card(&e, 0, "aurons_inspiration");
        let oid = e.state.players[0].hand[slot];
        semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
        resolve_entire_stack_two_player(&mut e);
        oid
    };
    assert_eq!(e.effective_power(first), Some(p1 + 2));
    assert_eq!(e.effective_power(second), Some(p2 + 2));

    // CR 702.34: cast from the graveyard for the flashback cost, then exile on resolution.
    let generation = e.state.zone_change_generation[&auron];
    semantic::accepted(
        &mut e,
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                source: Some(graveyard_cast_source(auron, generation)),
                cast_method: CastMethod::Flashback as i32,
                ..Default::default()
            })),
        },
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.players[0].exile.contains(&auron));
    assert!(!e.state.players[0].graveyard.contains(&auron));
}

#[test]
fn issue_misc13_dictate_of_kruphix_draws_for_each_player() {
    let mut e = engine(813_006);
    cast(&mut e, 0, "dictate_of_kruphix", vec![]);
    // End the first player's turn, then advance through the opponent's draw step.
    end_active_turn(&mut e, 0);
    let hand_before = e.state.players[1].hand.len();
    for _ in 0..30 {
        if e.state.turn_step == tricerules_core::TurnStep::Main1 && e.state.stack.is_empty() {
            break;
        }
        let priority = e.state.priority_player_id();
        semantic::accepted(&mut e, priority, &pass());
    }
    assert_eq!(
        e.state.players[1].hand.len(),
        hand_before + 2,
        "the draw step draws one card plus Dictate's additional card"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        7,
        "the controller does not draw"
    );
}

#[test]
fn issue_misc13_rowdy_research_reduces_per_attacker_and_draws_three() {
    let mut e = GameEngine::new(813_007, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let attackers: Vec<u32> = (0..3)
        .map(|_| inject_creature_on_battlefield(&mut e, 0, "grizzly_bears"))
        .collect();
    e.apply_command(0, &declare_attackers(attackers))
        .expect("declare attackers");
    inject_card_into_hand(&mut e, 0, "rowdy_research");
    // {6}{U} reduced by three declared attackers costs {3}{U}.
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "rowdy_research");
    assert!(
        e.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the spell is not castable without the reduction"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let hand_before = e.state.players[0].hand.len();
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3);
}

#[test]
fn issue_misc13_eject_cannot_be_countered_and_bounces_a_nonland_permanent() {
    let mut e = engine(813_008);
    let their_artifact = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    inject_card_into_hand(&mut e, 0, "eject");
    grant_pool(&mut e, 0);
    let eject_card = e.state.players[0].hand[hand_index_for_card(&e, 0, "eject")];
    let slot = hand_index_for_card(&e, 0, "eject");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(their_artifact)));

    // The opponent answers with a counterspell; "can't be countered" means Eject still resolves.
    inject_card_into_hand(&mut e, 1, "counterspell");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    semantic::accepted(&mut e, 0, &pass());
    let counter_slot = hand_index_for_card(&e, 1, "counterspell");
    semantic::accepted(
        &mut e,
        1,
        &cast_spell(counter_slot, target_object(eject_card)),
    );
    let hand_before = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&their_artifact].zone,
        Zone::Hand,
        "Eject resolved and returned the artifact"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "Eject's controller drew a card"
    );

    // A land is not a legal target for the bounce.
    let mut bad = engine(813_018);
    let forest = inject_permanent_on_battlefield(&mut bad, 1, "forest");
    inject_card_into_hand(&mut bad, 0, "eject");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "eject");
    assert!(
        bad.apply_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "a land is not a nonland permanent"
    );
}
