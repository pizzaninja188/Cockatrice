//! Actual-card scenarios for the queue-v76 02-subtype-lords cohort.
//!
//! Expectations are printed Oracle values. CR 604.2/611.3b keep the anthem active while
//! the source remains on the battlefield, CR 613.4c applies the layer-7c modification,
//! CR 109.5 resolves "you" to the source's controller, and the printed "other" excludes
//! only the source itself. Pinned 2018-2024 rulings confirm marked damage can
//! become lethal when the lord leaves.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    subtype: &'static str,
    buddy: &'static str,
    power: u32,
    toughness: u32,
    delta_power: u32,
    delta_toughness: u32,
    human_bystander: bool,
}

const CASES: [Case; 10] = [
    Case {
        id: "merfolk_mistbinder",
        subtype: "Merfolk",
        buddy: "coral_merfolk",
        power: 2,
        toughness: 2,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: false,
    },
    Case {
        id: "legion_lieutenant",
        subtype: "Vampire",
        buddy: "vampire_noble",
        power: 2,
        toughness: 2,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: false,
    },
    Case {
        id: "thirsting_bloodlord",
        subtype: "Vampire",
        buddy: "vampire_noble",
        power: 3,
        toughness: 3,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: false,
    },
    Case {
        id: "kargan_warleader",
        subtype: "Warrior",
        buddy: "crazed_goblin",
        power: 3,
        toughness: 3,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: false,
    },
    Case {
        id: "yotian_tactician",
        subtype: "Soldier",
        buddy: "honor_guard",
        power: 3,
        toughness: 4,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: true,
    },
    Case {
        id: "regal_imperiosaur",
        subtype: "Dinosaur",
        buddy: "charging_monstrosaur",
        power: 5,
        toughness: 4,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: false,
    },
    Case {
        id: "squirrel_sovereign",
        subtype: "Squirrel",
        buddy: "squirrelanoids",
        power: 2,
        toughness: 2,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: false,
    },
    Case {
        id: "inspiring_veteran",
        subtype: "Knight",
        buddy: "white_knight",
        power: 2,
        toughness: 2,
        delta_power: 1,
        delta_toughness: 1,
        human_bystander: true,
    },
    Case {
        id: "bladestitched_skaab",
        subtype: "Zombie",
        buddy: "walking_corpse",
        power: 2,
        toughness: 3,
        delta_power: 1,
        delta_toughness: 0,
        human_bystander: false,
    },
    Case {
        id: "king_of_the_pride",
        subtype: "Cat",
        buddy: "savannah_lions",
        power: 2,
        toughness: 1,
        delta_power: 2,
        delta_toughness: 1,
        human_bystander: false,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, card]),
        deck_with("island", &[]),
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

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|oid| *oid != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("source").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn pumped(case: &Case, power: u32, toughness: u32) -> (Option<u32>, Option<u32>) {
    (
        Some(power + case.delta_power),
        Some(toughness + case.delta_toughness),
    )
}

#[test]
fn q76_subtype_lords_canary_simplest_merfolk_mistbinder_pumps_only_another_merfolk() {
    let case = &CASES[0];
    let mut e = engine(76_200_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let buddy = inject_creature_with_stats(&mut e, 0, case.buddy, 2, 2);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let opposing = inject_creature_with_stats(&mut e, 1, case.buddy, 2, 2);

    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "the printed \"other\" excludes the source"
    );
    assert_eq!(
        (e.effective_power(buddy), e.effective_toughness(buddy)),
        pumped(case, 2, 2),
        "another Merfolk you control is pumped"
    );
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "a non-Merfolk is unaffected"
    );
    assert_eq!(
        (e.effective_power(opposing), e.effective_toughness(opposing)),
        (Some(2), Some(2)),
        "an opponent's Merfolk is unaffected"
    );
}

#[test]
fn q76_subtype_lords_canary_hardest_king_of_the_pride_uses_its_printed_values() {
    let case = &CASES[9];
    let mut e = engine(76_200_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let buddy = inject_creature_with_stats(&mut e, 0, case.buddy, 2, 2);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);

    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(1)),
        "King of the Pride keeps its printed 2/1 body"
    );
    assert_eq!(
        (e.effective_power(buddy), e.effective_toughness(buddy)),
        (Some(4), Some(3)),
        "other Cats get +2/+1"
    );
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2))
    );
}

#[test]
fn q76_subtype_lords_all_cards_pump_only_other_controlled_creatures_of_the_subtype() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(76_201_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let buddy = inject_creature_with_stats(&mut e, 0, case.buddy, 2, 2);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let human = inject_creature_with_stats(&mut e, 0, "archivist", 2, 2);
        let opposing = inject_creature_with_stats(&mut e, 1, case.buddy, 2, 2);

        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: source body is unchanged",
            case.id
        );
        assert_eq!(
            (e.effective_power(buddy), e.effective_toughness(buddy)),
            pumped(case, 2, 2),
            "{}: another {} you control is pumped",
            case.id,
            case.subtype
        );
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2), Some(2)),
            "{}: a non-{} is unaffected",
            case.id,
            case.subtype
        );
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (Some(2), Some(2)),
            "{}: an opponent's {} is unaffected",
            case.id,
            case.subtype
        );
        if case.human_bystander {
            assert_eq!(
                (e.effective_power(human), e.effective_toughness(human)),
                (Some(2), Some(2)),
                "{}: a Human without the printed subtype is unaffected",
                case.id
            );
        }
    }
}

#[test]
fn q76_subtype_lords_apply_to_later_entries_and_end_when_the_source_leaves() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(76_202_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let late = inject_creature_with_stats(&mut e, 0, case.buddy, 2, 2);
        assert_eq!(
            (e.effective_power(late), e.effective_toughness(late)),
            pumped(case, 2, 2),
            "{}: a later same-subtype creature is pumped",
            case.id
        );
        move_to_graveyard(&mut e, 0, source);
        assert_eq!(
            (e.effective_power(late), e.effective_toughness(late)),
            (Some(2), Some(2)),
            "{}: the pump drains when the source leaves",
            case.id
        );
    }
}

#[test]
fn q76_subtype_lords_two_copies_pump_each_other() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(76_203_000 + index as u64, case.id);
        let first = move_ready_to_battlefield(&mut e, 0, case.id);
        let second = move_ready_to_battlefield(&mut e, 0, case.id);
        for (source, other) in [(first, second), (second, first)] {
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                pumped(case, case.power, case.toughness),
                "{}: each copy pumps the other",
                case.id
            );
            assert_ne!(source, other);
        }
    }
}
