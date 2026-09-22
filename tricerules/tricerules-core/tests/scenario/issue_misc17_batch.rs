//! Reviewed direct-RON entry/value scenarios: Runaway Boulder, Hulldrifter, Meteor Sword, Holy Cow,
//! Terror of Mount Velus, Plumecreed Escort, Youthful Valkyrie, Obyra, Dreaming Duelist and
//! Long-Bodied Grey Dog.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 115.1a/120.3 (targeted damage), CR 301.5/702.6 (Equipment and equip),
//! CR 603.6a (entry trigger), CR 603.6d (self-excluding permanent-entry watchers), CR 611.2c
//! (until-end-of-turn keyword grants), CR 701.22 (scry), CR 702.4 (double strike), CR 702.9
//! (flying), CR 702.11 (hexproof), CR 702.17 (reach), CR 702.29 (cycling), CR 702.122 (crew),
//! and CR 111.10 (Treasure token).

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ChoiceKind, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
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

fn activate_on(
    e: &GameEngine,
    object_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    let mut command = activate_ability_with_costs(object_id, ability_index, targets, vec![]);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
        command.cmd.as_mut()
    else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    command
}

fn activate_from_hand(e: &GameEngine, object_id: u32, ability_index: u32) -> RuledCommand {
    let mut command = activate_ability_with_costs(object_id, ability_index, vec![], vec![]);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
        command.cmd.as_mut()
    else {
        unreachable!()
    };
    activation.source_zone = AbilitySourceZone::Hand as i32;
    activation.expected_zone_change_generation = generation(e, object_id);
    command
}

#[test]
fn issue_misc17_runaway_boulder_deals_six_to_an_opponent_creature() {
    let mut e = engine(817_001);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 7, 7);
    cast(&mut e, 0, "runaway_boulder", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&theirs].damage, 6,
        "the trigger deals damage rather than destroying"
    );
    assert_eq!(
        e.state.objects[&theirs].zone,
        Zone::Battlefield,
        "a 7/7 survives 6 damage"
    );

    // The entry trigger cannot target the controller's own creature.
    let mut own = engine(817_011);
    let mine = inject_creature_on_battlefield(&mut own, 0, "grizzly_bears");
    cast(&mut own, 0, "runaway_boulder", vec![]);
    assert!(
        own.apply_command(0, &choose_trigger_target(mine)).is_err(),
        "only an opponent's creature is a legal target"
    );

    // The Cycling {2} hand ability discards the card to draw.
    let mut cyc = engine(817_021);
    let hand_before = cyc.state.players[0].hand.len();
    let boulder = inject_card_into_hand(&mut cyc, 0, "runaway_boulder");
    grant_pool(&mut cyc, 0);
    let cycle = activate_from_hand(&cyc, boulder, 0);
    semantic::accepted(&mut cyc, 0, &cycle);
    resolve_entire_stack_two_player(&mut cyc);
    assert_eq!(
        cyc.state.objects[&boulder].zone,
        Zone::Graveyard,
        "cycling discards the card"
    );
    assert_eq!(
        cyc.state.players[0].hand.len(),
        hand_before + 1,
        "cycling draws a card after discarding"
    );
}

#[test]
fn issue_misc17_hulldrifter_draws_two_on_entry() {
    let mut e = engine(817_002);
    let hand_before = e.state.players[0].hand.len();
    cast(&mut e, 0, "hulldrifter", vec![]);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 2,
        "cast loses one card, then draws two"
    );
    let hulldrifter = battlefield_object(&e, 0, "hulldrifter");
    assert!(e.effective_has_keyword(hulldrifter, Keyword::Flying));
}

#[test]
fn issue_misc17_meteor_sword_destroys_on_entry_and_pumps_equipped() {
    let mut e = engine(817_003);
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "meteor_sword", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&theirs].zone, Zone::Graveyard);

    let sword = battlefield_object(&e, 0, "meteor_sword");
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    grant_pool(&mut e, 0);
    let equip = activate_on(&e, sword, 0, target_object(bear));
    semantic::accepted(&mut e, 0, &equip);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(5), "+3/+3");
    assert_eq!(e.effective_toughness(bear), Some(5));
}

#[test]
fn issue_misc17_holy_cow_gains_life_and_scries() {
    let mut e = engine(817_004);
    let life_before = e.state.players[0].life;
    let choice = cast(&mut e, 0, "holy_cow", vec![]);
    assert!(choice.is_some(), "the trailing scry 1 parks a choice");
    assert_eq!(e.state.players[0].life, life_before + 2);
    let pending = e.state.pending_resolution.as_ref().expect("scry choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryTop);
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_misc17_terror_of_mount_velus_grants_the_team_double_strike() {
    let mut e = engine(817_005);
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "terror_of_mount_velus", vec![]);
    let dragon = battlefield_object(&e, 0, "terror_of_mount_velus");
    assert!(e.effective_has_keyword(dragon, Keyword::DoubleStrike));
    assert!(
        e.effective_has_keyword(mine, Keyword::DoubleStrike),
        "each other creature you control also gains double strike"
    );
    assert!(
        !e.effective_has_keyword(theirs, Keyword::DoubleStrike),
        "an opponent's creature is unaffected"
    );

    // A creature that enters after the trigger resolves does not gain double strike.
    cast(&mut e, 0, "faerie_miscreant", vec![]);
    let late = battlefield_object(&e, 0, "faerie_miscreant");
    assert!(
        !e.effective_has_keyword(late, Keyword::DoubleStrike),
        "the grant is a resolution-time snapshot"
    );
}

#[test]
fn issue_misc17_plumecreed_escort_grants_hexproof_to_a_creature_you_control() {
    let mut e = engine(817_006);
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "plumecreed_escort", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(mine));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(mine, Keyword::Hexproof));
    assert!(!e.effective_has_keyword(theirs, Keyword::Hexproof));

    // Only a creature you control is a legal target.
    let mut own = engine(817_016);
    let enemy = inject_creature_on_battlefield(&mut own, 1, "grizzly_bears");
    cast(&mut own, 0, "plumecreed_escort", vec![]);
    assert!(
        own.apply_command(0, &choose_trigger_target(enemy)).is_err(),
        "an opponent's creature is not a legal target"
    );
}

#[test]
fn issue_misc17_youthful_valkyrie_watches_other_angels() {
    let mut e = engine(817_007);
    cast(&mut e, 0, "youthful_valkyrie", vec![]);
    let first = battlefield_object(&e, 0, "youthful_valkyrie");
    assert_eq!(
        e.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "it does not trigger on its own entry"
    );

    // Another Angel entering adds a +1/+1 counter.
    cast(&mut e, 0, "youthful_valkyrie", vec![]);
    assert_eq!(
        e.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "another Angel adds a counter"
    );

    // A non-Angel entering does not trigger.
    cast(&mut e, 0, "grizzly_bears", vec![]);
    assert_eq!(
        e.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "a Bear is not an Angel"
    );
}

#[test]
fn issue_misc17_obyra_drains_each_opponent_for_other_faeries() {
    let mut e = engine(817_008);
    cast(&mut e, 0, "obyra,_dreaming_duelist", vec![]);
    let their_life = e.state.players[1].life;

    // A non-Faerie entering does not drain.
    cast(&mut e, 0, "grizzly_bears", vec![]);
    assert_eq!(
        e.state.players[1].life, their_life,
        "a Bear is not a Faerie"
    );

    // A Faerie entering drains each opponent.
    cast(&mut e, 0, "faerie_miscreant", vec![]);
    assert_eq!(
        e.state.players[1].life,
        their_life - 1,
        "another Faerie drains each opponent"
    );
}

#[test]
fn issue_misc17_long_bodied_grey_dog_creates_a_tapped_treasure() {
    let mut e = engine(817_009);
    cast(&mut e, 0, "long-bodied_grey_dog", vec![]);
    let treasures = battlefield_token_oids(&e, 0, "treasure");
    assert_eq!(treasures.len(), 1, "one Treasure token");
    assert!(
        e.state.objects[&treasures[0]].tapped,
        "the Treasure enters tapped"
    );
}
