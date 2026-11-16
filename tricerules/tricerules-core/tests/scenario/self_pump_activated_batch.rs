//! Actual-card scenarios for the queue-v20 activated self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activated
//! ability and exact cost; CR 611.2a/613.4c apply the pump until end of turn and
//! CR 514.2 ends it during cleanup.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    activation: ManaGift,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "dross_ripper",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "ebony_treefolk",
            cast: ManaGift {
                b: 1,
                g: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                g: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "folk_of_the_pines",
            cast: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 5,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "kranioceros",
            cast: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 5,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 3,
        },
        Case {
            id: "zof_shade",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
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

fn activate_pump(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate the self-pump");
    assert_eq!(e.state.stack.len(), 1, "{id}: one ability is on the stack");
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact activation cost");
}

#[test]
fn self_pump_activated_batch_cards_cast_with_exact_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_970_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn self_pump_activated_batch_cards_activate_for_the_exact_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_971_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(case.power + case.pump_power),
                Some(case.toughness + case.pump_toughness)
            ),
            "{}: printed pump applied",
            case.id
        );
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: a mana-only activation does not tap",
            case.id
        );
    }
}

#[test]
fn self_pump_activated_batch_cards_stack_two_activations() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_972_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(case.power + 2 * case.pump_power),
                Some(case.toughness + 2 * case.pump_toughness)
            ),
            "{}: both activations apply",
            case.id
        );
    }
}

#[test]
fn self_pump_activated_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
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
            "{}: a rejected activation does not pump",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn self_pump_activated_batch_cards_expire_the_pump_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_974_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(case.power + case.pump_power),
                Some(case.toughness + case.pump_toughness)
            ),
            "{}",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: CR 514.2 ends the pump",
            case.id
        );
    }
}
