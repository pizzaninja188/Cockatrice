//! Actual-card scenarios for the queue-v36 untap-and-reach cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target as the
//! spell is cast; CR 701.26 untaps it; CR 611.2a/514.2 end the pump and reach grant;
//! CR 613.1f/613.4c apply the keyword and power/toughness changes.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "aim_high",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "spidery_grasp",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 4,
        },
        Case {
            id: "steady_aim",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 4,
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

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    let card = e.state.players[0].hand[slot];
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
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
fn q36_untap_reach_pump_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(36_080_001);
    let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    e.state.objects.get_mut(&bear).expect("object").tapped = true;
    let spell = cast_at(&mut e, case, bear);
    resolve_entire(&mut e);
    assert!(
        !e.state.objects[&bear].tapped,
        "Steady Aim untaps the target"
    );
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(3), Some(6)),
        "Steady Aim applies +1/+4"
    );
    assert!(
        e.effective_has_keyword(bear, Keyword::Reach),
        "Steady Aim grants reach"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q36_untap_reach_pump_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(36_080_002);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.state.objects.get_mut(&bear).expect("object").tapped = true;
    cast_at(&mut e, case, bear);
    resolve_entire(&mut e);
    assert!(!e.state.objects[&bear].tapped);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(4), Some(6)),
        "Spidery Grasp applies +2/+4 to an opponent's creature"
    );
    assert!(e.effective_has_keyword(bear, Keyword::Reach));
}

#[test]
fn q36_untap_reach_pump_cards_apply_the_printed_changes_and_expire() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_081_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        e.state.objects.get_mut(&bear).expect("object").tapped = true;
        cast_at(&mut e, case, bear);
        resolve_entire(&mut e);
        assert!(!e.state.objects[&bear].tapped, "{}: untapped", case.id);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (
                Some((2 + case.power) as u32),
                Some((2 + case.toughness) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(bear, Keyword::Reach),
            "{}: reach",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Reach),
            "{}: only the chosen creature gains reach",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2), Some(2)),
            "{}: the pump ends with the turn",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bear, Keyword::Reach),
            "{}: reach ends with the turn",
            case.id
        );
    }
}

#[test]
fn q36_untap_reach_pump_cards_accept_an_already_untapped_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_082_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_at(&mut e, case, bear);
        resolve_entire(&mut e);
        assert!(
            !e.state.objects[&bear].tapped,
            "{}: stays untapped",
            case.id
        );
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (
                Some((2 + case.power) as u32),
                Some((2 + case.toughness) as u32)
            ),
            "{}: the pump still applies",
            case.id
        );
        assert!(e.effective_has_keyword(bear, Keyword::Reach));
    }
}

#[test]
fn q36_untap_reach_pump_cards_require_a_creature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_083_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: a player is not a legal target",
            case.id
        );
        assert_eq!(
            pool(&e),
            (
                case.mana.w,
                case.mana.u,
                case.mana.b,
                case.mana.r,
                case.mana.g,
                case.mana.c
            ),
            "{}: rejected casts do not pay mana",
            case.id
        );
    }
}

#[test]
fn q36_untap_reach_pump_cards_fizzle_when_the_target_is_gone() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_084_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let spell = cast_at(&mut e, case, bear);
        move_to_graveyard(&mut e, 0, bear);
        resolve_entire(&mut e);
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(
            e.state.objects.get(&bear).map(|object| object.zone),
            Some(Zone::Graveyard),
            "{}: the target stays gone",
            case.id
        );
    }
}
