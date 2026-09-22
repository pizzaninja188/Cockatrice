//! Reviewed direct-RON entry-value scenarios: Rampaging Spiketail, Dinotomaton, Fang Guardian,
//! Lotusguard Disciple, Pileated Provisioner, Cloudblazer and Wood Elves.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: the rules Glossary term "another", CR 119.3 (life gain), CR 121.1
//! (draw), CR 122.1 (+1/+1 counters), CR 603.6a (entry trigger), CR 611.2c (until-end-of-turn P/T
//! and keyword grants), CR 701.23 (search, including fail-to-find), CR 702.8 (flash), CR 702.9
//! (flying), CR 702.12 (indestructible), CR 702.15 (lifelink), CR 702.29 (Swampcycling), and
//! CR 702.111 (menace).

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, player: i32, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
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

fn pass_to_search(e: &mut GameEngine) -> Vec<u32> {
    for _ in 0..40 {
        if let Some(pending) = e.state.pending_resolution.as_ref() {
            return pending.presentation.candidates.clone();
        }
        if e.state.stack.is_empty() {
            break;
        }
        let priority = e.state.priority_player_id();
        semantic::accepted(e, priority, &pass());
    }
    panic!("search never parked");
}

#[test]
fn issue_misc20_rampaging_spiketail_pumps_and_cycles() {
    let mut e = engine(820_001);
    let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast(&mut e, 0, "rampaging_spiketail", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(mine));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(mine), Some(4), "+2/+0");
    assert_eq!(e.effective_toughness(mine), Some(2));
    assert!(e.effective_has_keyword(mine, Keyword::Indestructible));

    // Swampcycling {2}: discard this card, then search up a Swamp.
    let mut cyc = engine(820_011);
    let swamp = inject_library_card(&mut cyc, 0, "swamp");
    let spiketail = inject_card_into_hand(&mut cyc, 0, "rampaging_spiketail");
    grant_pool(&mut cyc, 0);
    let cycle = activate_from_hand(&cyc, spiketail, 0);
    semantic::accepted(&mut cyc, 0, &cycle);
    let candidates = pass_to_search(&mut cyc);
    assert!(candidates.contains(&swamp), "a Swamp qualifies");
    semantic::accepted(&mut cyc, 0, &submit_resolution_choice(vec![swamp]));
    assert_eq!(cyc.state.objects[&spiketail].zone, Zone::Graveyard);
    assert_eq!(cyc.state.objects[&swamp].zone, Zone::Hand);
}

#[test]
fn issue_misc20_dinotomaton_grants_menace() {
    let mut e = engine(820_002);
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, 0, "dinotomaton", vec![]);
    let dinotomaton = battlefield_object(&e, 0, "dinotomaton");
    assert!(e.effective_has_keyword(dinotomaton, Keyword::Menace));
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(mine));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(mine, Keyword::Menace));
}

#[test]
fn issue_misc20_fang_guardian_pumps_another_permanent() {
    let mut e = engine(820_003);
    let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast(&mut e, 0, "fang_guardian", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(mine));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(mine), Some(4), "+2/+2");
    assert_eq!(e.effective_toughness(mine), Some(4));

    // "Another" excludes the Guardian itself.
    let mut own = engine(820_013);
    cast(&mut own, 0, "fang_guardian", vec![]);
    let source = battlefield_object(&own, 0, "fang_guardian");
    assert!(
        own.apply_command(0, &choose_trigger_target(source))
            .is_err(),
        "the source is not a legal 'another' target"
    );
}

#[test]
fn issue_misc20_lotusguard_disciple_grants_lifelink_and_indestructible() {
    let mut e = engine(820_004);
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "lotusguard_disciple", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(theirs, Keyword::Lifelink));
    assert!(e.effective_has_keyword(theirs, Keyword::Indestructible));
}

#[test]
fn issue_misc20_pileated_provisioner_counts_a_nonflyer() {
    let mut e = engine(820_005);
    let ground = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast(&mut e, 0, "pileated_provisioner", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(ground));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&ground].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    // A flyer is not a legal target.
    let mut flying = engine(820_015);
    let flier = inject_creature_on_battlefield(&mut flying, 0, "faerie_miscreant");
    cast(&mut flying, 0, "pileated_provisioner", vec![]);
    assert!(
        flying
            .apply_command(0, &choose_trigger_target(flier))
            .is_err(),
        "a creature with flying is not a legal target"
    );
}

#[test]
fn issue_misc20_cloudblazer_gains_life_and_draws() {
    let mut e = engine(820_006);
    let hand_before = e.state.players[0].hand.len();
    let life_before = e.state.players[0].life;
    cast(&mut e, 0, "cloudblazer", vec![]);
    assert_eq!(e.state.players[0].life, life_before + 2);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 2);
}

#[test]
fn issue_misc20_wood_elves_ramps_a_forest() {
    let mut e = engine(820_007);
    let forest = inject_library_card(&mut e, 0, "forest");
    let island = inject_library_card(&mut e, 0, "island");
    cast(&mut e, 0, "wood_elves", vec![]);
    let candidates = pass_to_search(&mut e);
    assert!(candidates.contains(&forest), "a Forest qualifies");
    assert!(!candidates.contains(&island), "an Island does not");
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![forest]));
    assert_eq!(e.state.objects[&forest].zone, Zone::Battlefield);
}
