//! Actual-card scenarios for the queue-v43 enters-counter targeted cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the stack;
//! CR 115.1 and 603.3d choose one target creature you control; CR 122.1 places the printed
//! counter. A target you do not control is not legal.
use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    counter: CounterKind,
    delta: i32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "kujar_seedsculptor",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        },
        Case {
            id: "teyos_lightshield",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 3,
            counter: CounterKind::PlusOnePlusOne,
            delta: 1,
        },
        Case {
            id: "ornery_kudu",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 4,
            counter: CounterKind::MinusOneMinusOne,
            delta: -1,
        },
    ]
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

fn choose_trigger_object(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: target_object(object_id),
            },
        )),
    }
}

fn cast_to_pending_trigger(engine: &mut GameEngine, case: &Case) -> u32 {
    let source = inject_card_into_hand(engine, 0, case.id);
    let slot = hand_index_for_card(engine, 0, case.id);
    give_mana(engine, 0, gift(&case.mana));
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    for _ in 0..engine.state.players.len() {
        let player = engine.state.priority_player_id();
        semantic::accepted(engine, player, &pass());
    }
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Battlefield,
        "{} must enter before its trigger",
        case.id
    );
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "{} must park one entry-trigger target",
        case.id
    );
    source
}

fn resolve_etb_target(engine: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let source = cast_to_pending_trigger(engine, case);
    semantic::accepted(engine, 0, &choose_trigger_object(target));
    source
}

#[test]
fn q43_etb_counter_target_canary_places_the_printed_counter() {
    let case = &cases()[0];
    let mut e = semantic::main_phase(43_080_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let source = resolve_etb_target(&mut e, case, target);
    semantic::complete(&mut e, 24, |_| None).require_exercised();
    assert_eq!(
        e.state.objects[&target].counters.get(&case.counter),
        Some(&1),
        "one +1/+1 counter on the chosen creature"
    );
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3)),
        "the counter raises the printed body"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "the source is unchanged"
    );
}

#[test]
fn q43_etb_counter_target_cards_apply_their_printed_counter_to_a_controlled_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = semantic::main_phase(43_081_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 4, 4);
        resolve_etb_target(&mut e, case, target);
        semantic::complete(&mut e, 24, |_| None).require_exercised();
        assert_eq!(
            e.state.objects[&target].counters.get(&case.counter),
            Some(&1),
            "{}: one printed counter",
            case.id
        );
        assert_eq!(
            e.effective_power(target),
            Some((4 + case.delta) as u32),
            "{}: power delta",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some((4 + case.delta) as u32),
            "{}: toughness delta",
            case.id
        );
        assert_eq!(
            e.state.players[0].battlefield.len(),
            2,
            "{}: source and target stay",
            case.id
        );
    }
}

#[test]
fn q43_etb_counter_target_cards_reject_a_creature_you_do_not_control() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = semantic::main_phase(43_082_000 + index as u64);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let _source = cast_to_pending_trigger(&mut e, case);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_object(opponent))
                .is_err(),
            "{}: an opponent's creature is not a legal target",
            case.id
        );
    }
}

#[test]
fn q43_etb_counter_target_cards_may_target_themselves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = semantic::main_phase(43_084_000 + index as u64);
        let entered = cast_to_pending_trigger(&mut e, case);
        semantic::accepted(&mut e, 0, &choose_trigger_object(entered));
        semantic::complete(&mut e, 24, |_| None).require_exercised();
        assert_eq!(
            e.state.objects[&entered].counters.get(&case.counter),
            Some(&1),
            "{}: the source may target itself",
            case.id
        );
        assert_eq!(
            e.effective_power(entered),
            Some((case.power as i32 + case.delta) as u32),
            "{}: self counter changes power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(entered),
            Some((case.toughness as i32 + case.delta) as u32),
            "{}: self counter changes toughness",
            case.id
        );
    }
}
