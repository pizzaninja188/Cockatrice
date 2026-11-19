//! Actual-card scenarios for the queue-v42 activated self-pump creature cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activated ability and its
//! exact cost; CR 611.2a/613.4c apply the pump until end of turn. The printed keyword or
//! protection static is checked in derived characteristics.
use super::helpers::*;
use tricerules_cards::primitives::{Color, Keyword, ProtectionQuality};
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    activation: ManaGift,
    body: (u32, u32),
    pump: (i32, i32),
    keyword: Option<Keyword>,
    protection: Option<Color>,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "ember-eye_wolf",
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 2),
            pump: (2, 0),
            keyword: Some(Keyword::Haste),
            protection: None,
        },
        Case {
            id: "lionheart_maverick",
            activation: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            body: (1, 1),
            pump: (1, 2),
            keyword: Some(Keyword::Vigilance),
            protection: None,
        },
        Case {
            id: "rune-cervin_rider",
            activation: ManaGift {
                g: 2,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, 1),
            keyword: Some(Keyword::Flying),
            protection: None,
        },
        Case {
            id: "inkrise_infiltrator",
            activation: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            body: (1, 2),
            pump: (2, 2),
            keyword: Some(Keyword::Flying),
            protection: None,
        },
        Case {
            id: "wildfire_emissary",
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 4),
            pump: (1, 0),
            keyword: None,
            protection: Some(Color::White),
        },
        Case {
            id: "thirsting_shade",
            activation: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 1),
            pump: (1, 1),
            keyword: Some(Keyword::Lifelink),
            protection: None,
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

fn activate_pump(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate the self-pump");
    assert_eq!(e.state.stack.len(), 1, "{id}: one ability is on the stack");
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact activation cost");
}

#[test]
fn q42_activated_self_pump_canary_simplest_pumps_with_the_exact_cost() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "ember-eye_wolf")
        .expect("Ember-Eye Wolf case");
    let mut e = engine(42_070_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(creature, Keyword::Haste));
    activate_pump(&mut e, creature, case.activation, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.body.0 + 2), Some(case.body.1)),
        "the printed +2/+0 applies"
    );
}

#[test]
fn q42_activated_self_pump_canary_hardest_accepts_either_hybrid_half() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "rune-cervin_rider")
        .expect("Rune-Cervin Rider case");
    for (index, mana) in [
        ManaGift {
            g: 2,
            ..Default::default()
        },
        ManaGift {
            w: 2,
            ..Default::default()
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(42_070_100 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(e.effective_has_keyword(creature, Keyword::Flying));
        activate_pump(&mut e, creature, mana, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0 + 1), Some(case.body.1 + 1)),
            "either {{G/W}} half pays the hybrid cost"
        );
    }
}

#[test]
fn q42_activated_self_pump_cards_apply_their_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_071_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        match case.keyword {
            Some(keyword) => assert!(
                e.effective_has_keyword(creature, keyword),
                "{}: printed keyword",
                case.id
            ),
            None => assert!(
                e.characteristics(creature)
                    .expect("characteristics")
                    .keywords
                    .is_empty(),
                "{}: no printed keyword",
                case.id
            ),
        }
        if let Some(color) = case.protection {
            assert_eq!(
                e.characteristics(creature)
                    .expect("characteristics")
                    .protections,
                [ProtectionQuality::Color(color)],
                "{}",
                case.id
            );
        }
        activate_pump(&mut e, creature, case.activation, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((case.body.0 as i32 + case.pump.0) as u32),
                Some((case.body.1 as i32 + case.pump.1) as u32)
            ),
            "{}: printed pump",
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
fn q42_activated_self_pump_cards_stack_two_activations() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_072_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((case.body.0 as i32 + 2 * case.pump.0) as u32),
                Some((case.body.1 as i32 + 2 * case.pump.1) as u32)
            ),
            "{}: both activations apply",
            case.id
        );
    }
}

#[test]
fn q42_activated_self_pump_cards_reject_activation_without_the_printed_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_073_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the exact mana is required",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: a rejected activation spends no mana",
            case.id
        );
        assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
