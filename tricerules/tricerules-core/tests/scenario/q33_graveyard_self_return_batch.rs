//! Actual-card scenarios for the queue-v33 graveyard self-return cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 govern the graveyard
//! activation, CR 118.3 requires the printed mana, and the returned object is a
//! new object in hand (CR 400.7). Clay Revenant's entry replacement is separate.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

fn plus_one_counters(e: &GameEngine, id: u32) -> u32 {
    e.state.objects[&id].counter_count(CounterKind::PlusOnePlusOne)
}

struct Case {
    id: &'static str,
    cost: ManaGift,
}

const CASES: [Case; 4] = [
    Case {
        id: "clay_revenant",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 1,
            r: 0,
            g: 0,
            c: 2,
        },
    },
    Case {
        id: "durable_coilbug",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 1,
            r: 0,
            g: 0,
            c: 4,
        },
    },
    Case {
        id: "jungle_creeper",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 1,
            r: 0,
            g: 1,
            c: 3,
        },
    },
    Case {
        id: "sanitarium_skeleton",
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 1,
            r: 0,
            g: 0,
            c: 2,
        },
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[card]), deck_with("forest", &[])]);
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

fn generation(e: &GameEngine, id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&id)
        .copied()
        .unwrap_or(0)
}

fn activate_graveyard(e: &GameEngine, id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: id,
            source_zone: AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: generation(e, id),
            ability_index: 0,
            ..Default::default()
        })),
    }
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

#[test]
fn q33_graveyard_self_return_canary_simplest() {
    let case = &CASES[3];
    let mut e = engine(33_050_001, case.id);
    let source = inject_graveyard_card(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.cost));
    let command = activate_graveyard(&e, source);
    e.apply_scenario_command(0, &command)
        .expect("activate the graveyard ability");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&source));
    assert!(!e.state.players[0].graveyard.contains(&source));
}

#[test]
fn q33_graveyard_self_return_canary_hardest() {
    let case = &CASES[2];
    let mut e = engine(33_050_002, case.id);
    let source = inject_graveyard_card(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.cost));
    let command = activate_graveyard(&e, source);
    e.apply_scenario_command(0, &command)
        .expect("activate the graveyard ability");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Hand);
    assert_eq!(plus_one_counters(&e, source), 0);
}

#[test]
fn q33_graveyard_self_return_cards_require_the_printed_mana_and_return_only_themselves() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_051_000 + index as u64, case.id);
        let source = inject_graveyard_card(&mut e, 0, case.id);
        let bystander = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let command = activate_graveyard(&e, source);
        assert!(
            e.apply_scenario_command(0, &command).is_err(),
            "{}: the printed mana is required",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        give_mana(&mut e, 0, gift(&case.cost));
        let command = activate_graveyard(&e, source);
        e.apply_scenario_command(0, &command)
            .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&source].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Graveyard,
            "{}: only the source returns",
            case.id
        );
    }
}

#[test]
fn q33_graveyard_self_return_cards_reject_foreign_controllers() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_052_000 + index as u64, case.id);
        let source = inject_graveyard_card(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.cost));
        give_mana(&mut e, 1, gift(&case.cost));
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        let command = activate_graveyard(&e, source);
        assert!(
            e.apply_scenario_command(1, &command).is_err(),
            "{}: another player cannot activate it even with the exact mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn q33_graveyard_self_return_clay_revenant_enters_tapped() {
    let mut e = engine(33_053_000, "clay_revenant");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 0,
            c: 1,
        },
    );
    ensure_in_hand(&mut e, 0, "clay_revenant");
    let slot = hand_index_for_card(&e, 0, "clay_revenant");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Clay Revenant");
    resolve_entire_stack_two_player(&mut e);
    let source = battlefield_object_for_card(&e, 0, "clay_revenant");
    assert!(e.state.objects[&source].tapped);
}
