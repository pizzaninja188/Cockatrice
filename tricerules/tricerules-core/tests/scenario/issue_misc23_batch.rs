//! Reviewed direct-RON "another permanent enters/dies" watcher scenarios: Valley Mightcaller,
//! Serra Redeemer, Wartime Protestors, Machinesmith Automaton, Shocking Sharpshooter, Boggart
//! Cursecrafter and Snarling Gorehound.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-23 against the pinned snapshot.
//! Governance: CR 603.6a/603.6d (exclude-self permanent-entry watchers), CR 603.6c (dies watcher),
//! CR 115.4 (target opponent), CR 119/120 (damage), CR 122.1 (+1/+1 counters), CR 701.25 (surveil),
//! CR 702.2 (deathtouch), CR 702.10 (haste), CR 702.17 (reach), CR 702.19 (trample), and CR 702.110
//! (menace). Rulings confirm the enter-time power snapshot and the per-permanent trigger count.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, player: i32, card_id: &str) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
}

fn battlefield_object(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    *e.state.players[player]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn counter_count(e: &GameEngine, object_id: u32) -> u32 {
    e.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne)
}

fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    semantic::accepted(e, 0, &cast_spell(slot, target_object(source)));
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

#[test]
fn issue_misc23_valley_mightcaller_counts_a_qualifying_subtype() {
    let mut e = engine(823_001);
    cast(&mut e, 0, "valley_mightcaller");
    let caller = battlefield_object(&e, 0, "valley_mightcaller");
    assert!(e.effective_has_keyword(caller, Keyword::Trample));

    // A Frog qualifies; a Bear does not.
    cast(&mut e, 0, "grizzly_bears");
    assert_eq!(
        counter_count(&e, caller),
        0,
        "a Bear is not a listed subtype"
    );
    cast(&mut e, 0, "valley_mightcaller");
    assert_eq!(
        counter_count(&e, caller),
        1,
        "another Frog you control adds a counter"
    );
}

#[test]
fn issue_misc23_serra_redeemer_counts_the_small_enterer() {
    let mut e = engine(823_002);
    cast(&mut e, 0, "serra_redeemer");
    let redeemer = battlefield_object(&e, 0, "serra_redeemer");
    assert!(e.effective_has_keyword(redeemer, Keyword::Flying));

    // A power-1 creature entering qualifies; the counters land on the entering creature.
    cast(&mut e, 0, "grizzly_bears");
    let bear = battlefield_object(&e, 0, "grizzly_bears");
    assert_eq!(
        counter_count(&e, bear),
        2,
        "two counters on the 2-power enterer"
    );

    // A power-4 creature entering does not trigger.
    cast(&mut e, 0, "air_elemental");
    let big = battlefield_object(&e, 0, "air_elemental");
    assert_eq!(counter_count(&e, big), 0, "power 4 exceeds the threshold");
    assert_eq!(
        counter_count(&e, redeemer),
        0,
        "the Redeemer is not the subject"
    );
}

#[test]
fn issue_misc23_wartime_protestors_count_and_hastes_the_ally() {
    let mut e = engine(823_003);
    cast(&mut e, 0, "wartime_protestors");
    let protestors = battlefield_object(&e, 0, "wartime_protestors");
    assert!(e.effective_has_keyword(protestors, Keyword::Haste));

    // A non-Ally does not trigger; an Ally does, and gains haste.
    cast(&mut e, 0, "grizzly_bears");
    let bear = battlefield_object(&e, 0, "grizzly_bears");
    assert_eq!(counter_count(&e, bear), 0);
    cast(&mut e, 0, "avatar_enthusiasts");
    let ally = battlefield_object(&e, 0, "avatar_enthusiasts");
    assert_eq!(
        counter_count(&e, ally),
        1,
        "another Ally you control adds a counter to itself"
    );
    assert!(e.effective_has_keyword(ally, Keyword::Haste));
}

#[test]
fn issue_misc23_machinesmith_automaton_counts_another_artifact() {
    let mut e = engine(823_004);
    cast(&mut e, 0, "machinesmith_automaton");
    let automaton = battlefield_object(&e, 0, "machinesmith_automaton");
    assert!(e.effective_has_keyword(automaton, Keyword::Trample));

    // A nonartifact does not trigger; another artifact does.
    cast(&mut e, 0, "grizzly_bears");
    assert_eq!(counter_count(&e, automaton), 0);
    cast(&mut e, 0, "magitek_armor");
    assert_eq!(
        counter_count(&e, automaton),
        1,
        "another artifact you control adds a counter to the Automaton"
    );
}

#[test]
fn issue_misc23_shocking_sharpshooter_damages_a_target_opponent() {
    let mut e = engine(823_005);
    inject_permanent_on_battlefield(&mut e, 0, "shocking_sharpshooter");
    let their_life = e.state.players[1].life;
    cast(&mut e, 0, "grizzly_bears");
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry watcher waits");
    let choose = RuledCommand {
        cmd: Some(
            tricerules_proto::ruled::v1::ruled_command::Cmd::ChooseTriggerTarget(
                ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id: 1,
                        group_index: 0,
                        kind: TargetRefKind::Player as i32,
                        ..Default::default()
                    }],
                },
            ),
        ),
    };
    semantic::accepted(&mut e, 0, &choose);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, their_life - 1);
}

#[test]
fn issue_misc23_boggart_cursecrafter_punishes_a_goblin_death() {
    let mut e = engine(823_006);
    inject_permanent_on_battlefield(&mut e, 0, "boggart_cursecrafter");
    let their_life = e.state.players[1].life;
    let goblin = inject_creature_on_battlefield(&mut e, 0, "raging_goblin");
    kill_with_murder(&mut e, goblin);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        their_life - 1,
        "another Goblin dying drains each opponent"
    );

    // A non-Goblin death does not trigger.
    let mut other = engine(823_016);
    inject_permanent_on_battlefield(&mut other, 0, "boggart_cursecrafter");
    let before = other.state.players[1].life;
    let bear = inject_creature_on_battlefield(&mut other, 0, "grizzly_bears");
    kill_with_murder(&mut other, bear);
    resolve_entire_stack_two_player(&mut other);
    assert_eq!(other.state.players[1].life, before);
}

#[test]
fn issue_misc23_snarling_gorehound_surveils_on_a_small_enterer() {
    let mut e = engine(823_007);
    inject_permanent_on_battlefield(&mut e, 0, "snarling_gorehound");
    cast(&mut e, 0, "grizzly_bears");
    assert!(
        e.state.pending_resolution.is_some(),
        "a power-2-or-less enterer parks a surveil 1 choice"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    assert!(e.state.pending_resolution.is_none());

    // A power-4 enterer does not trigger.
    let mut big = engine(823_017);
    inject_permanent_on_battlefield(&mut big, 0, "snarling_gorehound");
    cast(&mut big, 0, "air_elemental");
    assert!(
        big.state.pending_resolution.is_none(),
        "power 4 exceeds the threshold"
    );
}
