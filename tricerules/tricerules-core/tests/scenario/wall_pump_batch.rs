//! Actual-card scenarios for the queue-v19 defender self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.3b forbids a defender from attacking;
//! CR 602.1a/602.2 activate the printed pump ability; CR 611.2a/514.2 end the pump
//! during cleanup.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};

struct Case {
    id: &'static str,
    cast: ManaGift,
    pump: ManaGift,
    power: u32,
    toughness: u32,
    delta_power: u32,
    delta_toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "wall_of_faith",
            cast: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 5,
            delta_power: 0,
            delta_toughness: 1,
        },
        Case {
            id: "wall_of_fire",
            cast: ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 5,
            delta_power: 1,
            delta_toughness: 0,
        },
        Case {
            id: "wall_of_lava",
            cast: ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 3,
            delta_power: 1,
            delta_toughness: 1,
        },
        Case {
            id: "wall_of_opposition",
            cast: ManaGift {
                r: 2,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 6,
            delta_power: 1,
            delta_toughness: 0,
        },
        Case {
            id: "wall_of_water",
            cast: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            pump: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 5,
            delta_power: 1,
            delta_toughness: 0,
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

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.cast));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact full cost", case.id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}: resolves onto the battlefield",
        case.id
    );
    slot
}

fn pump(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate pump");
    pass_both_players(e);
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{id}: exact pump cost consumed"
    );
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
fn wall_pump_batch_cards_cast_with_exact_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_970_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Defender),
            "{}: defender",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn wall_pump_batch_cards_pump_with_the_exact_cost_and_printed_delta() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_971_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        pump(&mut e, creature, gift(&case.pump), case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + case.delta_power),
            "{}: power pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness + case.delta_toughness),
            "{}: toughness pump",
            case.id
        );
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: pumping does not tap",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].damage, 0,
            "{}: pumping does not change damage",
            case.id
        );
    }
}

#[test]
fn wall_pump_batch_cards_pump_expires_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_972_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        pump(&mut e, creature, gift(&case.pump), case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + case.delta_power),
            "{}",
            case.id
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power),
            "{}: power pump expires",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}: toughness pump expires",
            case.id
        );
    }
}

#[test]
fn wall_pump_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_973_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the pump needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: a rejected activation changes no body",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: no stack object", case.id);
    }
}

#[test]
fn wall_pump_batch_cards_cannot_attack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_974_000 + index as u64);
        let wall = cast_creature(&mut e, &case);
        e.state.objects.get_mut(&wall).expect("wall").summoning_sick = false;
        let legal_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to begin combat");
        e.apply_scenario_command(0, &pass())
            .expect("ap pass begin combat");
        e.apply_scenario_command(1, &pass())
            .expect("nap pass begin combat");
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::DeclareAttackers
        );
        assert!(
            e.apply_scenario_command(0, &declare_attackers(vec![wall]))
                .is_err(),
            "{}: a defender cannot attack",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![legal_attacker]))
            .expect("non-defender attacks");
        assert_eq!(
            e.state.combat.as_ref().expect("combat").attacking,
            vec![legal_attacker],
            "{}: the rejected wall did not enter combat",
            case.id
        );
    }
}
