//! Actual-card scenarios for the queue-v81 01-etb-target-pump cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger, CR 115.1 makes the
//! target chosen as the trigger is put on the stack, and CR 611.2c/514.2 fix and end the pump.
//! Flash timing follows CR 702.8a; menace, flying and haste are the printed keywords.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "bone_pit_brute",
            mana: ManaGift {
                r: 2,
                c: 4,
                ..Default::default()
            },
            power: 4,
            toughness: 5,
            pump_power: 4,
            pump_toughness: 0,
        },
        Case {
            id: "daybreak_combatants",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 0,
        },
        Case {
            id: "soldier_of_the_grey_host",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 0,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn choose_trigger_target(target: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(target),
        })),
    }
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("target").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn && e.state.turn_step == TurnStep::Upkeep {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

fn cast_and_pump(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    pass_both_players(e);
    assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
    e.apply_scenario_command(0, &choose_trigger_target(target))
        .expect("choose the pump target");
    resolve_entire_stack_two_player(e);
    e.state
        .objects
        .iter()
        .find_map(|(id, object)| {
            (object.card_id == case.id && object.zone == Zone::Battlefield).then_some(*id)
        })
        .unwrap_or_else(|| panic!("{} did not stay on the battlefield", case.id))
}

#[test]
fn q81_etb_pump_canary_simplest_daybreak_combatants_pumps_two() {
    let case = &cases()[1];
    let mut e = engine(81_100_001);
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    let source = cast_and_pump(&mut e, case, bears);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "printed body"
    );
    assert_eq!(
        (e.effective_power(bears), e.effective_toughness(bears)),
        (Some(4), Some(2)),
        "the entry trigger gives +2/+0"
    );
}

#[test]
fn q81_etb_pump_canary_hardest_soldier_is_flash_flying_and_pumps() {
    let case = &cases()[2];
    let mut e = engine(81_100_002);
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    end_active_turn(&mut e, 0);
    pass_both_players(&mut e);
    pass_both_players(&mut e);
    assert_eq!(
        e.state.priority_player_id(),
        e.state.players[1].id,
        "opponent has priority"
    );
    e.apply_scenario_command(e.state.players[1].id, &pass())
        .expect("opponent passes priority");
    let source = cast_and_pump(&mut e, case, bears);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "flash resolved on the opponent's turn"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "printed body"
    );
    assert!(
        e.effective_has_keyword(source, tricerules_cards::Keyword::Flying),
        "the printed flying keyword applies"
    );
    assert_eq!(
        (e.effective_power(bears), e.effective_toughness(bears)),
        (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
        "printed pump"
    );
}

#[test]
fn q81_etb_pump_cards_apply_printed_pump_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_101_000 + index as u64);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let source = cast_and_pump(&mut e, &case, bears);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
            "{}: printed pump",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    }
}

#[test]
fn q81_etb_pump_cards_target_any_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_102_000 + index as u64);
        let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_and_pump(&mut e, &case, opponent_creature);
        assert_eq!(
            (
                e.effective_power(opponent_creature),
                e.effective_toughness(opponent_creature)
            ),
            (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
            "{}: the trigger may pump any creature",
            case.id
        );
    }
}

#[test]
fn q81_etb_pump_cards_pump_expires_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_103_000 + index as u64);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_and_pump(&mut e, &case, bears);
        assert_eq!(
            e.effective_power(bears),
            Some(2 + case.pump_power),
            "{}: pumped before cleanup",
            case.id
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(
            e.effective_power(bears),
            Some(2),
            "{}: power expires",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bears),
            Some(2),
            "{}: toughness expires",
            case.id
        );
    }
}

#[test]
fn q81_etb_pump_cards_stale_target_fizzles_without_retargeting() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_104_000 + index as u64);
        let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        pass_both_players(&mut e);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(bears))
            .expect("choose the doomed target");
        move_to_graveyard(&mut e, 0, bears);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(other), e.effective_toughness(other)),
            (Some(2), Some(2)),
            "{}: the trigger does not retarget another creature",
            case.id
        );
    }
}
