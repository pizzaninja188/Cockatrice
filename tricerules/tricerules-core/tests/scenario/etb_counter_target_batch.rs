//! Actual-card scenarios for the reviewed enters-counter creature family.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on
//! the stack; CR 115.1 and 603.3d choose one target creature; CR 122.1 places
//! one +1/+1 counter; CR 608.2b drops the trigger when its target is stale.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "bond_beetle",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 1,
        },
        Case {
            id: "backup_agent",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "dauntless_survivor",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "ironshell_beetle",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "satyr_grovedancer",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "timberland_guide",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
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

fn engine(seed: u64) -> GameEngine {
    semantic::main_phase(seed)
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
fn etb_counter_target_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(4_030_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = resolve_etb_target(&mut e, &case, target);
        semantic::complete(&mut e, 24, |_| None).require_exercised();
        assert_eq!(
            e.state.objects[&target]
                .counters
                .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
            Some(&1),
            "{}: one +1/+1 counter on the chosen creature",
            case.id
        );
        assert_eq!(
            e.effective_power(target),
            Some(3),
            "{}: the counter raises power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(3),
            "{}: the counter raises toughness",
            case.id
        );
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: the source is unchanged",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}: the source is unchanged",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let mut self_e = engine(4_030_500 + index as u64);
        let self_source = cast_to_pending_trigger(&mut self_e, &case);
        semantic::accepted(&mut self_e, 0, &choose_trigger_object(self_source));
        semantic::complete(&mut self_e, 24, |_| None).require_exercised();
        assert_eq!(
            self_e.state.objects[&self_source]
                .counters
                .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
            Some(&1),
            "{}: the source may target itself",
            case.id
        );
        assert_eq!(
            self_e.effective_power(self_source),
            Some(case.power + 1),
            "{}",
            case.id
        );
        assert_eq!(
            self_e.effective_toughness(self_source),
            Some(case.toughness + 1),
            "{}",
            case.id
        );
    }
}

#[test]
fn etb_counter_target_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(4_031_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        let legal = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = cast_to_pending_trigger(&mut e, &case);
        assert!(
            e.apply_scenario_command(0, &choose_trigger_object(artifact))
                .is_err(),
            "{}: an artifact is not a creature target",
            case.id
        );
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: the rejected choice keeps the trigger pending",
            case.id
        );
        semantic::accepted(&mut e, 0, &choose_trigger_object(legal));
        semantic::complete(&mut e, 24, |_| None).require_exercised();
        assert!(
            e.state
                .objects
                .get(&artifact)
                .expect("artifact")
                .counters
                .is_empty(),
            "{}: an illegal target receives no counter",
            case.id
        );
        assert_eq!(
            e.state.objects[&legal]
                .counters
                .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
            Some(&1),
            "{}: the legal target receives the counter",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn etb_counter_target_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(4_032_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = resolve_etb_target(&mut e, &case, target);
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        semantic::complete(&mut e, 24, |_| None).require_exercised();
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert!(
            e.state
                .objects
                .get(&target)
                .expect("target")
                .counters
                .is_empty(),
            "{}: a stale target receives no counter",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(
            e.state
                .objects
                .get(&source)
                .expect("source")
                .counters
                .is_empty(),
            "{}: the counter did not move to the source",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
