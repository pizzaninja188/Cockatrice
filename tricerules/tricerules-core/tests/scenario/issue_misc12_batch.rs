//! Reviewed direct-RON Equipment/Vehicle/spell scenarios: Glimmerlight, Veloheart Bike,
//! Ripclaw Wrangler, Reach for the Sky, Skycrash and Locust Spray.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 111.10a (Glimmer
//! token), CR 115 (targets), CR 301.5/702.6 (Equipment and equip), CR 603.6a (entry trigger),
//! CR 603.6c/700.4 ("dies"), CR 605/106.1b (any-color mana), CR 611.2c/613.4c (P/T and keyword
//! changes), CR 701.9 (discard), CR 702.8 (flash), CR 702.19 (reach), CR 702.29 (cycling), and
//! CR 702.122 (crew).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChoiceKind,
    CostObjectRef, CostObjectRefs, CostSelection, ResolutionChoiceRequired, RuledCommand,
    TargetRef,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Casts a card and drives passes until the stack settles or a resolution choice appears.
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
        if e.state.stack.is_empty() {
            return None;
        }
        if e.state.blocking_choice().is_some() {
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

/// Activates a hand-zone ability (for example cycling), binding the source's current generation.
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
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
        command.cmd.as_mut()
    else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    command
}

#[test]
fn issue_misc12_glimmerlight_makes_a_glimmer_and_equips() {
    let mut e = engine(812_001);
    cast(&mut e, 0, "glimmerlight", vec![]);
    assert_eq!(
        battlefield_token_oids(&e, 0, "glimmer_w_1_1").len(),
        1,
        "the entry trigger creates a Glimmer"
    );

    let glimmerlight = battlefield_object(&e, 0, "glimmerlight");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(bear).expect("power");
    let base_toughness = e.effective_toughness(bear).expect("toughness");
    let equip = activate_on(&e, glimmerlight, 0, target_object(bear), vec![]);
    semantic::accepted(&mut e, 0, &equip);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(base_power + 1));
    assert_eq!(e.effective_toughness(bear), Some(base_toughness + 1));
}

#[test]
fn issue_misc12_veloheart_bike_gains_life_makes_mana_and_crews() {
    let mut e = engine(812_002);
    let life_before = e.state.players[0].life;
    cast(&mut e, 0, "veloheart_bike", vec![]);
    assert_eq!(e.state.players[0].life, life_before + 2, "ETB gain 2 life");

    let bike = battlefield_object(&e, 0, "veloheart_bike");
    let before = e.state.players[0].mana_pool;
    let mana = activate_on(&e, bike, 0, vec![], vec![]);
    semantic::accepted(&mut e, 0, &mana);
    assert!(
        e.state.objects[&bike].tapped,
        "the mana ability taps the Bike"
    );
    let after = e.state.players[0].mana_pool;
    let before_total = before.white + before.blue + before.black + before.red + before.green;
    let after_total = after.white + after.blue + after.black + after.red + after.green;
    assert_eq!(
        after_total,
        before_total + 1,
        "one mana of any color was added"
    );

    // Crew 2 animates the Vehicle for the turn.
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(!e.characteristics(bike).unwrap().is_creature());
    let crew = activate_on(
        &e,
        bike,
        1,
        vec![],
        vec![cost_selection(0, &e, &[first, second])],
    );
    semantic::accepted(&mut e, 0, &crew);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&first].tapped && e.state.objects[&second].tapped);
    let crewed = e.characteristics(bike).unwrap();
    assert!(crewed.is_creature());
    assert_eq!((crewed.power, crewed.toughness), (Some(4), Some(2)));
}

#[test]
fn issue_misc12_ripclaw_wrangler_makes_each_opponent_discard() {
    let mut e = engine(812_003);
    // Seed a known card in the opponent's hand so the discard is observable.
    let their_card = inject_card_into_hand(&mut e, 1, "grizzly_bears");
    let choice = cast(&mut e, 0, "ripclaw_wrangler", vec![]).expect("entry discard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&their_card));
    semantic::accepted(&mut e, 1, &submit_resolution_choice(vec![their_card]));
    assert_eq!(e.state.objects[&their_card].zone, Zone::Graveyard);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_misc12_reach_for_the_sky_pumps_and_draws_when_it_leaves() {
    let mut e = engine(812_004);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(bear).expect("power");
    let base_toughness = e.effective_toughness(bear).expect("toughness");
    cast(&mut e, 0, "reach_for_the_sky", target_object(bear));
    assert_eq!(e.effective_power(bear), Some(base_power + 3));
    assert_eq!(e.effective_toughness(bear), Some(base_toughness + 2));
    assert!(e.effective_has_keyword(bear, tricerules_cards::Keyword::Reach));

    // The Aura is put into the graveyard from the battlefield when the creature dies, so it draws.
    let hand_before = e.state.players[0].hand.len();
    cast(&mut e, 0, "murder", target_object(bear));
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the self-graveyard trigger draws a card"
    );
}

#[test]
fn issue_misc12_reach_for_the_sky_does_not_trigger_when_it_fizzles_from_the_stack() {
    // 2024-04-12 ruling: if the target is illegal on resolution the Aura is put into the graveyard
    // from the stack, not the battlefield, so its self-graveyard trigger must not fire.
    let mut e = engine(812_014);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "reach_for_the_sky");
    grant_pool(&mut e, 0);
    let reach_slot = hand_index_for_card(&e, 0, "reach_for_the_sky");
    semantic::accepted(&mut e, 0, &cast_spell(reach_slot, target_object(bear)));

    // Respond by destroying the only target while the Aura is still on the stack.
    inject_card_into_hand(&mut e, 0, "murder");
    grant_pool(&mut e, 0);
    let murder_slot = hand_index_for_card(&e, 0, "murder");
    semantic::accepted(&mut e, 0, &cast_spell(murder_slot, target_object(bear)));
    let hand_before = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    let aura = e.state.players[0]
        .graveyard
        .iter()
        .find(|oid| e.state.objects[oid].card_id == "reach_for_the_sky")
        .copied();
    assert!(
        aura.is_some(),
        "the fizzled Aura is put into the graveyard from the stack"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "a fizzled Aura does not draw a card"
    );
}

#[test]
fn issue_misc12_skycrash_destroys_an_artifact_and_cycles() {
    let mut e = engine(812_005);
    let their_artifact = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    cast(&mut e, 0, "skycrash", target_object(their_artifact));
    assert_eq!(e.state.objects[&their_artifact].zone, Zone::Graveyard);

    // Cycling {R}: discard this card, then draw a card.
    let skycrash = inject_card_into_hand(&mut e, 0, "skycrash");
    grant_pool(&mut e, 0);
    let hand_before = e.state.players[0].hand.len();
    let cycling = cycle_from_hand(&e, skycrash, 0);
    semantic::accepted(&mut e, 0, &cycling);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&skycrash].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "cycling discards one card and draws one"
    );

    // An untargeted cast is illegal: the destruction needs an artifact target.
    let mut bad = engine(812_015);
    inject_card_into_hand(&mut bad, 0, "skycrash");
    grant_pool(&mut bad, 0);
    let slot = hand_index_for_card(&bad, 0, "skycrash");
    assert!(
        bad.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the destruction requires an artifact target"
    );
}

#[test]
fn issue_misc12_locust_spray_weakens_a_creature_and_cycles() {
    let mut e = engine(812_006);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, 0, "locust_spray", target_object(bear));
    assert_eq!(e.effective_power(bear), Some(1));
    assert_eq!(e.effective_toughness(bear), Some(1));

    // Cycling {B} from hand.
    let locust = inject_card_into_hand(&mut e, 0, "locust_spray");
    grant_pool(&mut e, 0);
    let hand_before = e.state.players[0].hand.len();
    let cycling = cycle_from_hand(&e, locust, 0);
    semantic::accepted(&mut e, 0, &cycling);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&locust].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].hand.len(), hand_before);

    // At the next cleanup the toughness reduction expires.
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(bear), Some(2));
    assert_eq!(e.effective_toughness(bear), Some(2));
}
