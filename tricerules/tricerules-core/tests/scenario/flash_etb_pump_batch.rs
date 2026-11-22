//! Actual-card scenarios for the queue-v19 flash entry-pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.8a permits flash timing; CR 603.6a
//! stages the entry trigger; CR 115.1 makes the target chosen as the trigger is put on
//! the stack; CR 611.2c/514.2 fix and end the pump.
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
            id: "affa_guard_hound",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 3,
        },
        Case {
            id: "bogardan_rager",
            mana: ManaGift {
                r: 1,
                c: 5,
                ..Default::default()
            },
            power: 3,
            toughness: 4,
            pump_power: 4,
            pump_toughness: 0,
        },
        Case {
            id: "briarpack_alpha",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            pump_power: 2,
            pump_toughness: 2,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

#[test]
fn flash_etb_pump_batch_cards_cast_at_instant_speed_and_pump_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_980_000 + index as u64);
        let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[1].id,
            "{}: opponent has priority",
            case.id
        );
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes priority");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: flash cast: {error:?}", case.id));
        pass_both_players(&mut e);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(bears))
            .expect("choose the pump target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Battlefield,
            "{}: flash resolved on the opponent's turn",
            case.id
        );
        assert_eq!(
            (e.effective_power(slot), e.effective_toughness(slot)),
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
fn flash_etb_pump_batch_cards_target_any_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_981_000 + index as u64);
        let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .expect("cast");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(opponent_creature))
            .expect("choose an opponent's creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (
                e.effective_power(opponent_creature),
                e.effective_toughness(opponent_creature)
            ),
            (Some(2 + case.pump_power), Some(2 + case.pump_toughness)),
            "{}: the trigger may pump any creature",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    }
}

#[test]
fn flash_etb_pump_batch_cards_pump_expires_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_982_000 + index as u64);
        let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .expect("cast");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(bears))
            .expect("choose the pump target");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(bears),
            Some(2 + case.pump_power),
            "{}",
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
fn flash_etb_pump_batch_cards_can_target_the_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_983_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(source))
            .expect("choose the source itself");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power),
                Some(case.toughness + case.pump_toughness)
            ),
            "{}: the source may target itself",
            case.id
        );
    }
}

#[test]
fn flash_etb_pump_batch_cards_stale_target_fizzles_without_pumping_another() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_984_000 + index as u64);
        let friendly = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the departed target");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the source is not pumped when its target left",
            case.id
        );
        assert_eq!(
            (e.effective_power(friendly), e.effective_toughness(friendly)),
            (Some(2), Some(2)),
            "{}: no other creature is pumped",
            case.id
        );
    }
}
