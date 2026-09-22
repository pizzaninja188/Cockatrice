//! Reviewed direct-RON entry/value scenarios: Magitek Armor, Dragoon's Wyvern, Fire Nation Warship,
//! Knowledge Seeker, Disruptor of Currents, Battle-Rattle Shaman and Bespoke Bō.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-22 against the pinned snapshot.
//! Governance: CR 111.10 (Clue and Hero tokens), CR 115.3 (the "up to one other" bounce), CR 121.2/
//! 603.2 (the second-draw ordinal), CR 301.5/702.6 (Equipment and equip), CR 301.7 (Vehicle),
//! CR 508.1/603.2b (beginning-of-combat timing), CR 603.6a (entry trigger), CR 603.6c (dies
//! trigger), CR 613.4c (attached +2/+1 and vigilance), CR 702.9 (flying), CR 702.17 (reach),
//! CR 702.20 (vigilance), CR 702.8 (flash), CR 702.51 (convoke), and CR 702.122 (crew).

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind};

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

fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    let slot = hand_index_for_card(e, 0, "murder");
    semantic::accepted(e, 0, &cast_spell(slot, target_object(source)));
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("murder resolves");
}

#[test]
fn issue_misc18_magitek_armor_creates_a_hero_on_entry() {
    let mut e = engine(818_001);
    cast(&mut e, 0, "magitek_armor", vec![]);
    let heroes = battlefield_token_oids(&e, 0, "hero_c_1_1");
    assert_eq!(heroes.len(), 1, "one Hero token");
    assert_eq!(e.effective_power(heroes[0]), Some(1));
    assert_eq!(e.effective_toughness(heroes[0]), Some(1));
}

#[test]
fn issue_misc18_dragoons_wyvern_creates_a_hero_and_flies() {
    let mut e = engine(818_002);
    cast(&mut e, 0, "dragoons_wyvern", vec![]);
    let wyvern = battlefield_object(&e, 0, "dragoons_wyvern");
    assert!(e.effective_has_keyword(wyvern, Keyword::Flying));
    assert_eq!(battlefield_token_oids(&e, 0, "hero_c_1_1").len(), 1);
}

#[test]
fn issue_misc18_fire_nation_warship_makes_a_clue_when_it_dies() {
    let mut e = engine(818_003);
    let warship = inject_permanent_on_battlefield(&mut e, 0, "fire_nation_warship");
    assert!(e.effective_has_keyword(warship, Keyword::Reach));
    inject_card_into_hand(&mut e, 0, "skycrash");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "skycrash");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(warship)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&warship].zone, Zone::Graveyard);
    assert_eq!(
        battlefield_token_oids(&e, 0, "clue").len(),
        1,
        "the dies trigger creates one Clue token"
    );
}

#[test]
fn issue_misc18_knowledge_seeker_counts_the_second_draw_and_makes_a_clue_on_death() {
    let mut e = engine(818_004);
    cast(&mut e, 0, "knowledge_seeker", vec![]);
    let seeker = battlefield_object(&e, 0, "knowledge_seeker");
    assert!(e.effective_has_keyword(seeker, Keyword::Vigilance));

    // Divination draws two cards; the second draw adds a +1/+1 counter.
    cast(&mut e, 0, "divination", vec![]);
    assert_eq!(
        e.state.objects[&seeker].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the second draw adds a counter"
    );

    kill_with_murder(&mut e, seeker);
    assert_eq!(e.state.objects[&seeker].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "clue").len(),
        1,
        "the dies trigger creates one Clue token"
    );
}

#[test]
fn issue_misc18_disruptor_of_currents_bounces_an_other_nonland_permanent() {
    let mut e = engine(818_005);
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "disruptor_of_currents", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&theirs].zone, Zone::Hand);
    assert!(e.state.players[1].hand.contains(&theirs));

    // A land is not a legal target, and the source cannot target itself.
    let mut land = engine(818_015);
    let forest = inject_permanent_on_battlefield(&mut land, 1, "forest");
    cast(&mut land, 0, "disruptor_of_currents", vec![]);
    assert!(
        land.apply_command(0, &choose_trigger_target(forest))
            .is_err(),
        "a land is not a legal target"
    );

    let mut self_target = engine(818_025);
    cast(&mut self_target, 0, "disruptor_of_currents", vec![]);
    let source = battlefield_object(&self_target, 0, "disruptor_of_currents");
    assert!(
        self_target
            .apply_command(0, &choose_trigger_target(source))
            .is_err(),
        "the source cannot target itself"
    );
}

#[test]
fn issue_misc18_battle_rattle_shaman_pumps_at_beginning_of_combat() {
    let mut e = engine(818_006);
    inject_permanent_on_battlefield(&mut e, 0, "battle-rattle_shaman");
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    e.apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "the combat trigger waits"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(4), "+2/+0 until end of turn");
    assert_eq!(e.effective_toughness(bear), Some(2), "toughness unchanged");
}

#[test]
fn issue_misc18_bespoke_bo_bounces_and_pumps_the_equipped_creature() {
    let mut e = engine(818_007);
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, 0, "bespoke_bō", vec![]);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    semantic::accepted(&mut e, 0, &choose_trigger_target(theirs));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&theirs].zone, Zone::Hand);

    let bo = battlefield_object(&e, 0, "bespoke_bō");
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    grant_pool(&mut e, 0);
    let equip = activate_on(&e, bo, 0, target_object(bear));
    semantic::accepted(&mut e, 0, &equip);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(4), "+2/+1");
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert!(e.effective_has_keyword(bear, Keyword::Vigilance));
}
