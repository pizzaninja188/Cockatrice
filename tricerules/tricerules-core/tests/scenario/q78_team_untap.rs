//! Actual-card scenarios for the queue-v78 03-team-untap cohort.
//!
//! Expectations are printed Oracle values. CR 701.26 untaps each creature the spell's controller
//! controls at resolution; CR 613.4c applies the pump in layer 7c. The pinned 2017-09-29 and
//! 2019-01-25 rulings require already-untapped creatures to receive the pump and later creatures
//! to be unaffected.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: Option<(u32, u32)>,
    draw: bool,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "mobilize",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            pump: None,
            draw: false,
        },
        Case {
            id: "to_arms!",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump: None,
            draw: true,
        },
        Case {
            id: "rallying_roar",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            pump: Some((1, 1)),
            draw: false,
        },
        Case {
            id: "rally_to_battle",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            pump: Some((1, 3)),
            draw: false,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

fn cast_case(e: &mut GameEngine, case: &Case) -> Result<u32, String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .map(|_| e.state.stack.last().expect("spell").id)
        .map_err(|error| format!("{error:?}"))
}

fn is_tapped(e: &GameEngine, id: u32) -> bool {
    e.state.objects[&id].tapped
}

#[test]
fn q78_team_untap_canary_simplest_mobilize_untaps_only_your_creatures() {
    let case = &cases()[0];
    let mut e = engine(78_300_001);
    let untapped = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let tapped_creature = inject_creature_with_stats(&mut e, 0, "juggernaut", 5, 5);
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state
        .objects
        .get_mut(&tapped_creature)
        .expect("mine")
        .tapped = true;
    e.state.objects.get_mut(&opponent).expect("theirs").tapped = true;
    cast_case(&mut e, case).expect("cast Mobilize");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed {{G}} is spent");
    assert!(
        is_tapped(&e, tapped_creature),
        "the creature is tapped before resolution"
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(
        !is_tapped(&e, untapped),
        "an untapped creature stays untapped"
    );
    assert!(
        !is_tapped(&e, tapped_creature),
        "a tapped creature is untapped"
    );
    assert!(
        is_tapped(&e, opponent),
        "the opponent's creature is not untapped"
    );
    assert_eq!(
        (e.effective_power(untapped), e.effective_toughness(untapped)),
        (Some(2), Some(2)),
        "Mobilize does not change power or toughness"
    );
}

#[test]
fn q78_team_untap_canary_hardest_rallying_roar_pumps_and_untaps_even_untapped_creatures() {
    let case = &cases()[2];
    let mut e = engine(78_300_002);
    let untapped = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let tapped_creature = inject_creature_with_stats(&mut e, 0, "juggernaut", 5, 5);
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state
        .objects
        .get_mut(&tapped_creature)
        .expect("mine")
        .tapped = true;
    e.state.objects.get_mut(&opponent).expect("theirs").tapped = true;
    cast_case(&mut e, case).expect("cast Rallying Roar");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        !is_tapped(&e, tapped_creature),
        "the tapped creature is untapped"
    );
    assert_eq!(
        (e.effective_power(untapped), e.effective_toughness(untapped)),
        (Some(3), Some(3)),
        "an already-untapped creature still gets +1/+1"
    );
    assert_eq!(
        (
            e.effective_power(tapped_creature),
            e.effective_toughness(tapped_creature)
        ),
        (Some(6), Some(6)),
        "the tapped creature gets +1/+1 and untaps"
    );
    assert_eq!(
        (e.effective_power(opponent), e.effective_toughness(opponent)),
        (Some(2), Some(2)),
        "the opponent's creature is unaffected"
    );
    assert!(
        is_tapped(&e, opponent),
        "the opponent's creature stays tapped"
    );
}

#[test]
fn q78_team_untap_cards_apply_only_their_printed_riders() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_301_000 + index as u64);
        let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let other = inject_creature_on_battlefield(&mut e, 0, "juggernaut");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&other).expect("mine").tapped = true;
        e.state.objects.get_mut(&opponent).expect("theirs").tapped = true;
        let hand_before = e.state.players[0].hand.len();
        cast_case(&mut e, &case).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert!(!is_tapped(&e, mine), "{}: untapped", case.id);
        assert!(!is_tapped(&e, other), "{}: untapped", case.id);
        assert!(
            is_tapped(&e, opponent),
            "{}: the opponent's creature stays tapped",
            case.id
        );
        let (power, toughness) = case.pump.unwrap_or((0, 0));
        assert_eq!(
            (e.effective_power(mine), e.effective_toughness(mine)),
            (Some(2 + power), Some(2 + toughness)),
            "{}: printed pump",
            case.id
        );
        if case.draw {
            assert_eq!(
                e.state.players[0].hand.len(),
                hand_before + 1,
                "{}: draws one card",
                case.id
            );
        } else {
            assert_eq!(
                e.state.players[0].hand.len(),
                hand_before,
                "{}: no draw",
                case.id
            );
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q78_team_untap_cards_do_not_affect_creatures_that_enter_later() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.pump.is_none() {
            continue;
        }
        let mut e = engine(78_302_000 + index as u64);
        let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_case(&mut e, &case).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let (power, toughness) = case.pump.expect("pump case");
        assert_eq!(
            (e.effective_power(mine), e.effective_toughness(mine)),
            (Some(2 + power), Some(2 + toughness)),
            "{}: the resolving case pumps the existing creature",
            case.id
        );
        let later = inject_creature_with_stats(&mut e, 0, "juggernaut", 5, 5);
        assert_eq!(
            (e.effective_power(later), e.effective_toughness(later)),
            (Some(5), Some(5)),
            "{}: a creature that enters later is not pumped",
            case.id
        );
    }
}

#[test]
fn q78_team_untap_mobilize_keeps_sorcery_timing() {
    let case = &cases()[0];
    let mut e = engine(78_303_000);
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("active pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass begin combat");
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let before = pool(&e);
    let slot = hand_index_for_card(&e, 0, case.id);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .is_err(),
        "Mobilize is a sorcery and cannot be cast during combat"
    );
    assert_eq!(pool(&e), before, "the rejected cast spends no mana");
    assert!(e.state.stack.is_empty());
}

#[test]
fn q78_team_untap_pump_does_not_apply_to_a_creature_controlled_later() {
    let case = &cases()[2];
    let mut e = engine(78_304_000);
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_case(&mut e, case).expect("cast Rallying Roar");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(mine), e.effective_toughness(mine)),
        (Some(3), Some(3)),
        "the creature controlled at resolution is pumped"
    );
    assert_eq!(
        (e.effective_power(theirs), e.effective_toughness(theirs)),
        (Some(2), Some(2)),
        "the opponent's creature is not pumped"
    );
    let player0 = e.state.players[0].id;
    let changed = e.state.objects.get_mut(&theirs).expect("theirs");
    changed.controller = player0;
    changed.base_controller = player0;
    assert_eq!(
        (e.effective_power(theirs), e.effective_toughness(theirs)),
        (Some(2), Some(2)),
        "a creature controlled later in the turn is not retroactively pumped"
    );
}
