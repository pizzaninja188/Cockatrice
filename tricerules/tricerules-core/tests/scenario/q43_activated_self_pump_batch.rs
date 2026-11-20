//! Actual-card scenarios for the queue-v43 activated self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activation; CR 118.3 pays
//! the printed mana; CR 611.2a/613.4c apply the temporary pump and CR 514.2 ends it at cleanup.
use super::helpers::*;

struct Case {
    id: &'static str,
    activation: ManaGift,
    body: (u32, u32),
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "tyrranax",
            activation: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            body: (5, 4),
            pump: (-1, 1),
        },
        Case {
            id: "greater_forgeling",
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 4),
            pump: (3, -3),
        },
        Case {
            id: "shambling_strider",
            activation: ManaGift {
                r: 1,
                g: 1,
                ..Default::default()
            },
            body: (5, 5),
            pump: (1, -1),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn activate(e: &mut GameEngine, creature: u32, mana: &ManaGift, id: &str) {
    give_mana(e, 0, gift(mana));
    apply_ability(e, 0, creature, 0, vec![])
        .unwrap_or_else(|error| panic!("{id}: activate: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost");
    assert!(!e.state.objects[&creature].tapped, "{id}: no tap cost");
}

#[test]
fn q43_activated_self_pump_canary_applies_the_printed_pump() {
    let case = &cases()[0];
    let mut e = engine(43_060_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    activate(&mut e, creature, &case.activation, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (
            Some((case.body.0 as i32 + case.pump.0) as u32),
            Some((case.body.1 as i32 + case.pump.1) as u32)
        ),
        "the printed pump applies"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact cost");
}

#[test]
fn q43_activated_self_pump_cards_apply_their_printed_values_and_expire() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_061_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        activate(&mut e, creature, &case.activation, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((case.body.0 as i32 + case.pump.0) as u32),
                Some((case.body.1 as i32 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn q43_activated_self_pump_cards_require_the_printed_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_062_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: activation requires its printed mana",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: no pump without mana",
            case.id
        );
    }
}
