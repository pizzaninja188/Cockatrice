//! Actual-card scenarios for the queue-v36 pump-and-fight cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/115.3 require two separate target
//! instances; CR 611.2a/514.2 end the pump; CR 701.12a-b make the creatures deal damage
//! equal to their power to each other.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "epic_confrontation",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
        },
        Case {
            id: "ruthless_predation",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
        },
        Case {
            id: "savage_smash",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
        },
        Case {
            id: "swift_kick",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            power: 1,
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn group_one(oid: u32) -> TargetRef {
    let mut target = target_object(oid).remove(0);
    target.group_index = 1;
    target
}

fn pair(first: u32, second: u32) -> Vec<TargetRef> {
    vec![target_object(first)[0], group_one(second)]
}

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    let card = e.state.players[0].hand[slot];
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    card
}

fn resolve_entire(e: &mut GameEngine) {
    loop {
        if e.state.stack.is_empty() {
            break;
        }
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_scenario_command(first, &pass())
            .expect("first pass");
        e.apply_scenario_command(second, &pass())
            .expect("second pass");
    }
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn q36_pump_fight_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(36_070_001);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 3, 8);
    let their = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    let spell = cast_at(&mut e, case, pair(own, their));
    resolve_entire(&mut e);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        e.effective_power(own),
        Some(4),
        "Swift Kick pumps before the fight"
    );
    assert_eq!(
        e.state.objects[&their].damage, 4,
        "the pumped creature deals its power to the opposing creature"
    );
    assert_eq!(
        e.state.objects[&own].damage, 6,
        "the opposing creature deals its power back"
    );
}

#[test]
fn q36_pump_fight_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(36_070_002);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 3, 8);
    let their = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    cast_at(&mut e, case, pair(own, their));
    resolve_entire(&mut e);
    assert_eq!(
        e.state.objects[&their].damage, 5,
        "Savage Smash's +2/+2 pump is dealt during the fight"
    );
    assert_eq!(e.state.objects[&own].damage, 6);
    end_active_turn(&mut e, 0);
}

#[test]
fn q36_pump_fight_cards_apply_the_printed_pump_then_fight() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_071_000 + index as u64);
        let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 3, 8);
        let their = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
        cast_at(&mut e, case, pair(own, their));
        resolve_entire(&mut e);
        assert_eq!(
            e.state.objects[&their].damage,
            (3 + case.power) as u32,
            "{}: the pumped power is dealt to the opposing creature",
            case.id
        );
        assert_eq!(
            e.state.objects[&own].damage, 6,
            "{}: the opposing 6 power is dealt back",
            case.id
        );
        assert_eq!(
            e.effective_power(own),
            Some((3 + case.power) as u32),
            "{}: printed pump",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(own),
            Some(3),
            "{}: the pump ends with the turn",
            case.id
        );
    }
}

#[test]
fn q36_pump_fight_cards_require_one_creature_on_each_side() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_072_000 + index as u64);
        let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let own_other = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        let their = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let their_other = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, pair(their, their_other)))
                .is_err(),
            "{}: the first target must be a creature you control",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, pair(own, own_other)))
                .is_err(),
            "{}: the second target must be a creature you don't control",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, pair(own, their)))
                .is_ok(),
            "{}: one target from each side is legal",
            case.id
        );
    }
}

#[test]
fn q36_pump_fight_cards_keep_the_pump_when_the_opposing_target_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_073_000 + index as u64);
        let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let their = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let spell = cast_at(&mut e, case, pair(own, their));
        move_to_graveyard(&mut e, 1, their);
        resolve_entire(&mut e);
        assert_eq!(
            e.effective_power(own),
            Some((2 + case.power) as u32),
            "{}: the remaining legal target still receives the pump",
            case.id
        );
        assert_eq!(
            e.state.objects[&own].damage, 0,
            "{}: no fight happens without both creatures",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    }
}
